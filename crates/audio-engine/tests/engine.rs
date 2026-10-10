//! The engine with fakes on both sides: no sound hardware, no network.

use audio_engine::device::{AudioIo, DeviceCounters, DeviceList, OpenDevice};
use audio_engine::mixer::MixerRt;
use audio_engine::{spawn, EngineConfig, EngineHandle};
use engine_protocol::{AudioDevice, Command, ConnectionState, Event, Settings, TrackKind};
use rtrb::Producer;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use transport::{
    AudioReceiver, EventFn, LinkState, PublishOptions, Transport, TransportEvent, VoiceOut, BLOCK,
};

// ── fake transport ──────────────────────────────────────────────────────────────────────

/// What the engine attached: where events go and where remote audio goes.
type Hooks = (Arc<EventFn>, Arc<dyn AudioReceiver>);

#[derive(Default)]
struct Wire {
    calls: Mutex<Vec<String>>,
    hooks: Mutex<Option<Hooks>>,
    sent: Mutex<Vec<Vec<i16>>>,
}

impl Wire {
    fn emit(&self, event: TransportEvent) {
        let events = self.hooks.lock().unwrap().as_ref().unwrap().0.clone();
        events(event);
    }

    fn receiver(&self) -> Arc<dyn AudioReceiver> {
        self.hooks.lock().unwrap().as_ref().unwrap().1.clone()
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// What a real transport reports when a join goes through.
    fn establish(&self) {
        self.emit(TransportEvent::State(LinkState::Connecting));
        self.emit(TransportEvent::Joined {
            room: "gang".into(),
            identity: "me".into(),
        });
        self.emit(TransportEvent::State(LinkState::Connected));
    }
}

impl VoiceOut for Wire {
    fn send(&self, block: &[i16]) {
        self.sent.lock().unwrap().push(block.to_vec());
    }
}

struct FakeTransport(Arc<Wire>);

impl Transport for FakeTransport {
    fn attach(&mut self, events: EventFn, receiver: Arc<dyn AudioReceiver>) {
        *self.0.hooks.lock().unwrap() = Some((Arc::new(events), receiver));
    }
    fn join(&mut self, url: &str, token: &str, options: PublishOptions) {
        let call = format!("join {url} {token} {}", options.bitrate_bps);
        self.0.calls.lock().unwrap().push(call);
    }
    fn leave(&mut self) {
        self.0.calls.lock().unwrap().push("leave".into());
    }
    fn set_muted(&mut self, muted: bool) {
        self.0.calls.lock().unwrap().push(format!("muted {muted}"));
    }
    fn voice_out(&self) -> Arc<dyn VoiceOut> {
        self.0.clone()
    }
}

// ── fake sound hardware ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Hardware {
    opened: Mutex<Vec<String>>,
    microphone: Mutex<Option<Producer<i16>>>,
    counters: Mutex<Option<Arc<DeviceCounters>>>,
    speaker: Arc<Mutex<Option<MixerRt>>>,
}

struct FakeIo(Arc<Hardware>);

/// Like a real stream: closing it hands the mixer back.
struct FakeStream {
    speaker: Arc<Mutex<Option<MixerRt>>>,
    back: mpsc::Sender<MixerRt>,
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        if let Some(mixer) = self.speaker.lock().unwrap().take() {
            let _ = self.back.send(mixer);
        }
    }
}

fn open(name: &str, keep: Box<dyn std::any::Any + Send>) -> OpenDevice {
    OpenDevice {
        name: name.into(),
        sample_rate: 48_000,
        channels: 1,
        counters: Arc::new(DeviceCounters::default()),
        keep,
    }
}

impl AudioIo for FakeIo {
    fn devices(&self) -> DeviceList {
        let device = |id: &str| AudioDevice {
            id: id.into(),
            name: id.to_uppercase(),
            is_default: id.ends_with('1'),
        };
        DeviceList {
            inputs: vec![device("mic1"), device("mic2")],
            outputs: vec![device("out1")],
        }
    }

    fn open_capture(
        &mut self,
        id: Option<&str>,
        ring: Producer<i16>,
    ) -> Result<OpenDevice, String> {
        let name = id.unwrap_or("mic1");
        self.0
            .opened
            .lock()
            .unwrap()
            .push(format!("capture {name}"));
        if name == "gone" {
            return Err("the selected device is not connected".into());
        }
        *self.0.microphone.lock().unwrap() = Some(ring);
        let device = open(name, Box::new(()));
        *self.0.counters.lock().unwrap() = Some(device.counters.clone());
        Ok(device)
    }

    fn open_playout(
        &mut self,
        id: Option<&str>,
        mixer: MixerRt,
        back: mpsc::Sender<MixerRt>,
    ) -> Result<OpenDevice, String> {
        let name = id.unwrap_or("out1");
        self.0
            .opened
            .lock()
            .unwrap()
            .push(format!("playout {name}"));
        if name == "gone" {
            let _ = back.send(mixer);
            return Err("the selected device is not connected".into());
        }
        *self.0.speaker.lock().unwrap() = Some(mixer);
        let stream = FakeStream {
            speaker: self.0.speaker.clone(),
            back,
        };
        Ok(open(name, Box::new(stream)))
    }
}

// ── harness ─────────────────────────────────────────────────────────────────────────────

struct Rig {
    engine: EngineHandle,
    events: mpsc::Receiver<Event>,
    wire: Arc<Wire>,
    hardware: Arc<Hardware>,
}

fn rig(settings: Settings) -> Rig {
    let wire = Arc::new(Wire::default());
    let hardware = Arc::new(Hardware::default());
    let (tx, events) = mpsc::channel();
    let config = EngineConfig {
        rejoin_delay: Duration::from_millis(50),
    };
    let engine = spawn(
        Box::new(FakeTransport(wire.clone())),
        Box::new(FakeIo(hardware.clone())),
        settings,
        config,
        move |event| {
            let _ = tx.send(event);
        },
    );
    Rig {
        engine,
        events,
        wire,
        hardware,
    }
}

impl Rig {
    /// The first event that satisfies `wanted`, skipping levels and stats in between.
    fn wait<T>(&self, what: &str, wanted: impl Fn(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        while let Ok(event) = self
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            if let Some(found) = wanted(&event) {
                return found;
            }
        }
        panic!("no event within 5 s: {what}");
    }

    fn wait_state(&self, state: ConnectionState) {
        self.wait(&format!("{state:?}"), |e| {
            matches!(e, Event::Connection { state: s } if *s == state).then_some(())
        });
    }

    fn wait_until(&self, what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "not within 5 s: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn join(&self) {
        self.engine.send(Command::Join {
            url: "ws://test".into(),
            token: "tok".into(),
        });
        self.wait_until("join reaches the transport", || {
            self.wire.calls().iter().any(|c| c.starts_with("join"))
        });
        self.wire.establish();
        self.wait_state(ConnectionState::Connected);
    }
}

// ── tests ───────────────────────────────────────────────────────────────────────────────

#[test]
fn starts_ready_lists_devices_and_opens_the_defaults() {
    let rig = rig(Settings::default());
    rig.wait("ready", |e| matches!(e, Event::Ready { .. }).then_some(()));
    let inputs = rig.wait("devices", |e| match e {
        Event::Devices { inputs, .. } => Some(inputs.clone()),
        _ => None,
    });
    assert_eq!(inputs.len(), 2);
    assert!(inputs[0].is_default);
    rig.wait("stats", |e| match e {
        Event::Stats { stats } => Some(stats.clone()),
        _ => None,
    });
    assert_eq!(
        *rig.hardware.opened.lock().unwrap(),
        ["capture mic1", "playout out1"]
    );
}

#[test]
fn a_missing_device_falls_back_to_the_default_and_says_so() {
    let rig = rig(Settings {
        input_device: Some("gone".into()),
        output_device: Some("gone".into()),
        ..Settings::default()
    });
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("not connected"), "{message}");
    rig.wait_until("fallbacks opened", || {
        *rig.hardware.opened.lock().unwrap()
            == [
                "capture gone",
                "capture mic1",
                "playout gone",
                "playout out1",
            ]
    });
    // The mixer survived the failed attempt: remote audio still reaches the output.
    assert!(rig.hardware.speaker.lock().unwrap().is_some());
}

#[test]
fn join_uses_the_bitrate_from_the_settings_and_reports_the_room() {
    let rig = rig(Settings {
        voice_bitrate_kbps: 96,
        ..Settings::default()
    });
    rig.engine.send(Command::Join {
        url: "ws://test".into(),
        token: "tok".into(),
    });
    rig.wait_until("join", || rig.wire.calls() == ["join ws://test tok 96000"]);
    rig.wire.emit(TransportEvent::State(LinkState::Connecting));
    rig.wait_state(ConnectionState::Connecting);
    rig.wire.emit(TransportEvent::Joined {
        room: "gang".into(),
        identity: "me".into(),
    });
    let room = rig.wait("joined", |e| match e {
        Event::Joined { room, .. } => Some(room.clone()),
        _ => None,
    });
    assert_eq!(room, "gang");
    rig.wire.emit(TransportEvent::State(LinkState::Connected));
    rig.wait_state(ConnectionState::Connected);
}

#[test]
fn participants_are_listed_sorted_with_voice_and_mute() {
    let rig = rig(Settings::default());
    rig.join();
    for (identity, name) in [("u2", "ben"), ("u1", "Anna")] {
        rig.wire.emit(TransportEvent::ParticipantJoined {
            identity: identity.into(),
            name: name.into(),
        });
    }
    rig.wire.emit(TransportEvent::VoiceStarted {
        identity: "u2".into(),
    });
    rig.wire.emit(TransportEvent::ParticipantMuted {
        identity: "u1".into(),
        muted: true,
    });
    let list = rig.wait("both listed, with state", |e| match e {
        Event::Participants { participants }
            if participants.len() == 2 && participants[0].muted && participants[1].has_voice =>
        {
            Some(participants.clone())
        }
        _ => None,
    });
    assert_eq!(list[0].name, "Anna");
    assert_eq!(list[1].name, "ben");

    rig.wire.emit(TransportEvent::ParticipantLeft {
        identity: "u1".into(),
    });
    rig.wait("anna gone", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });
}

#[test]
fn a_lost_session_is_joined_again_until_the_user_leaves() {
    let rig = rig(Settings::default());
    rig.join();
    rig.wire.emit(TransportEvent::ParticipantJoined {
        identity: "u1".into(),
        name: "Anna".into(),
    });

    rig.wire.emit(TransportEvent::Failed {
        message: "disconnected by the server".into(),
        retryable: true,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.wait("the others are gone", |e| match e {
        Event::Participants { participants } if participants.is_empty() => Some(()),
        _ => None,
    });
    rig.wait_state(ConnectionState::Reconnecting);
    rig.wait_until("second join", || {
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count()
            == 2
    });

    // The second attempt fails as well: keep trying.
    rig.wire.emit(TransportEvent::Failed {
        message: "could not connect".into(),
        retryable: true,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.wait_until("third join", || {
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count()
            == 3
    });

    // Leave in the pause before the next attempt: no session exists that could report
    // its end, the engine has to say "disconnected" by itself.
    rig.wire.emit(TransportEvent::Failed {
        message: "could not connect".into(),
        retryable: true,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.engine.send(Command::Leave);
    rig.wait_state(ConnectionState::Disconnected);
    rig.wait_until("leave", || rig.wire.calls().last().unwrap() == "leave");
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count(),
        3,
        "no rejoin after leave"
    );
}

#[test]
fn a_join_that_never_worked_is_not_repeated() {
    let rig = rig(Settings::default());
    rig.engine.send(Command::Join {
        url: "ws://test".into(),
        token: "expired".into(),
    });
    rig.wait_until("join", || rig.wire.calls().len() == 1);
    rig.wire.emit(TransportEvent::State(LinkState::Connecting));
    rig.wire.emit(TransportEvent::Failed {
        message: "401 Unauthorized".into(),
        retryable: false,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("401"));
    rig.wait_state(ConnectionState::Disconnected);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(rig.wire.calls().len(), 1);
}

#[test]
fn the_microphone_reaches_the_transport_in_blocks_and_mute_sends_silence() {
    let rig = rig(Settings::default());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    let speak = |blocks: usize| {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        for _ in 0..blocks * BLOCK {
            mic.as_mut().unwrap().push(8000).unwrap();
        }
    };

    speak(2);
    rig.wait_until("two blocks sent", || {
        rig.wire.sent.lock().unwrap().len() == 2
    });
    assert!(rig.wire.sent.lock().unwrap()[1] == vec![8000; BLOCK]);
    let level = rig.wait("input level", |e| match e {
        Event::Levels {
            input_db,
            gate_open: true,
            ..
        } if *input_db > -20.0 => Some(*input_db),
        _ => None,
    });
    assert!(
        (level - -12.25).abs() < 0.1,
        "8000/32767 is -12.25 dBFS, got {level}"
    );

    rig.engine.send(Command::SetMuted { muted: true });
    rig.wait_until("mute reaches the transport", || {
        rig.wire.calls().contains(&"muted true".to_string())
    });
    speak(1);
    rig.wait_until("third block sent", || {
        rig.wire.sent.lock().unwrap().len() == 3
    });
    assert!(rig.wire.sent.lock().unwrap()[2] == vec![0; BLOCK]);
    // The meter still shows the microphone works, the gate shows it is not sent.
    rig.wait("muted level", |e| match e {
        Event::Levels {
            input_db,
            gate_open: false,
            ..
        } if *input_db > -20.0 => Some(()),
        _ => None,
    });
}

#[test]
fn a_capture_backlog_is_dropped_not_sent_as_a_burst() {
    let rig = rig(Settings::default());
    rig.wait_until("microphone open", || {
        rig.hardware.microphone.lock().unwrap().is_some()
    });
    {
        let mut mic = rig.hardware.microphone.lock().unwrap();
        let mic = mic.as_mut().unwrap();
        // Eight blocks at once, numbered — the pump never gets a chance in between.
        let backlog: Vec<i16> = (0..8).flat_map(|n| [n; BLOCK]).collect();
        mic.push_entire_slice(&backlog).unwrap();
    }
    rig.wait_until("something sent", || {
        !rig.wire.sent.lock().unwrap().is_empty()
    });
    std::thread::sleep(Duration::from_millis(50));
    let sent = rig.wire.sent.lock().unwrap();
    assert!(sent.len() <= 3, "{} blocks went out", sent.len());
    assert_eq!(sent.last().unwrap()[0], 7, "the newest block is kept");
    drop(sent);
    let dropped = rig.wait("dropped blocks in the stats", |e| match e {
        Event::Stats { stats } if stats.send_blocks_dropped > 0 => Some(stats.send_blocks_dropped),
        _ => None,
    });
    assert!(dropped >= 5);
}

#[test]
fn remote_voices_reach_the_output_with_level_volume_and_a_freed_slot() {
    let rig = rig(Settings::default());
    rig.join();
    rig.wait_until("output open", || {
        rig.hardware.speaker.lock().unwrap().is_some()
    });
    let render = |frames: usize| {
        let mut out = vec![0f32; frames];
        rig.hardware
            .speaker
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .render(&mut out);
        out
    };

    let mut anna = rig.wire.receiver().speaker_started("u1");
    render(BLOCK); // a fresh slot is flushed first
    anna.frame(&[16384; BLOCK]);
    anna.frame(&[16384; BLOCK]);
    assert!(render(BLOCK).iter().all(|s| (*s - 0.5).abs() < 1e-3));

    let peer = rig.wait("anna speaking", |e| match e {
        Event::Levels { peers, .. } if peers.len() == 1 && peers[0].speaking => {
            Some(peers[0].clone())
        }
        _ => None,
    });
    assert_eq!(peer.identity, "u1");
    assert!((peer.level_db - -6.02).abs() < 0.1);

    rig.engine.send(Command::SetPeerVolume {
        identity: "u1".into(),
        track: TrackKind::Voice,
        gain_db: -6.0206,
    });
    rig.wait_until("volume applied", || {
        anna.frame(&[16384; BLOCK]);
        anna.frame(&[16384; BLOCK]);
        let out = render(2 * BLOCK);
        (out[0] - 0.25).abs() < 1e-3
    });

    drop(anna);
    rig.wait("nobody speaking", |e| match e {
        Event::Levels { peers, .. } if peers.is_empty() => Some(()),
        _ => None,
    });
}

#[test]
fn changing_the_output_keeps_the_speakers() {
    let rig = rig(Settings::default());
    rig.wait_until("output open", || {
        rig.hardware.speaker.lock().unwrap().is_some()
    });
    let mut anna = rig.wire.receiver().speaker_started("u1");

    rig.engine.send(Command::SetOutputDevice {
        id: Some("out2".into()),
    });
    rig.wait_until("second output opened", || {
        rig.hardware.opened.lock().unwrap().last().unwrap() == "playout out2"
            && rig.hardware.speaker.lock().unwrap().is_some()
    });
    assert_eq!(rig.engine.settings().output_device.as_deref(), Some("out2"));

    let mut out = vec![0f32; BLOCK];
    let mut speaker = rig.hardware.speaker.lock().unwrap();
    speaker.as_mut().unwrap().render(&mut out);
    for _ in 0..2 {
        anna.frame(&[16384; BLOCK]);
    }
    speaker.as_mut().unwrap().render(&mut out);
    assert!(
        out.iter().all(|s| (*s - 0.5).abs() < 1e-3),
        "same mixer, same slot"
    );
}

#[test]
fn a_failed_microphone_is_reopened_on_the_default() {
    let rig = rig(Settings {
        input_device: Some("mic2".into()),
        ..Settings::default()
    });
    rig.wait_until("microphone open", || {
        rig.hardware.counters.lock().unwrap().is_some()
    });
    rig.hardware
        .counters
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .failed
        .store(true, Relaxed);
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("microphone stopped working"));
    rig.wait_until("default reopened", || {
        *rig.hardware.opened.lock().unwrap() == ["capture mic2", "playout out1", "capture mic1"]
    });
}

#[test]
fn invalid_settings_are_rejected_and_the_old_ones_stay() {
    let rig = rig(Settings::default());
    rig.engine.send(Command::ApplySettings {
        settings: Settings {
            voice_bitrate_kbps: 4,
            ..Settings::default()
        },
    });
    let message = rig.wait("error", |e| match e {
        Event::Error { message } => Some(message.clone()),
        _ => None,
    });
    assert!(message.contains("settings rejected"));
    assert_eq!(rig.engine.settings().voice_bitrate_kbps, 80);
}

#[test]
fn a_ui_that_attaches_late_gets_the_current_state_again() {
    let rig = rig(Settings::default());
    rig.join();
    rig.wire.emit(TransportEvent::ParticipantJoined {
        identity: "u1".into(),
        name: "Anna".into(),
    });
    rig.wait("anna listed", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });

    rig.engine.send(Command::Resync);
    rig.wait("ready", |e| matches!(e, Event::Ready { .. }).then_some(()));
    rig.wait("devices", |e| {
        matches!(e, Event::Devices { .. }).then_some(())
    });
    rig.wait_state(ConnectionState::Connected);
    let room = rig.wait("room", |e| match e {
        Event::Joined { room, identity } => Some(format!("{room}/{identity}")),
        _ => None,
    });
    assert_eq!(room, "gang/me");
    rig.wait("anna again", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });
}

#[test]
fn being_thrown_out_is_not_answered_with_a_rejoin() {
    let rig = rig(Settings::default());
    rig.join();
    // The same invite was opened on a second machine: joining again would only throw
    // that one out in turn, forever.
    rig.wire.emit(TransportEvent::Failed {
        message: "disconnected by the server or the network: DuplicateIdentity".into(),
        retryable: false,
    });
    rig.wire
        .emit(TransportEvent::State(LinkState::Disconnected));
    rig.wait_state(ConnectionState::Disconnected);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(rig.wire.calls().len(), 1, "one join, no second");
}

#[test]
fn the_pause_between_rejoin_attempts_grows() {
    let rig = rig(Settings::default());
    rig.join();
    let joins = || {
        rig.wire
            .calls()
            .iter()
            .filter(|c| c.starts_with("join"))
            .count()
    };
    let fail = || {
        rig.wire.emit(TransportEvent::Failed {
            message: "could not connect".into(),
            retryable: true,
        });
        rig.wire
            .emit(TransportEvent::State(LinkState::Disconnected));
    };
    // Base delay in this rig: 50 ms. Attempts come after 50, 100, 150 ms.
    let started = Instant::now();
    fail();
    rig.wait_until("second join", || joins() == 2);
    fail();
    rig.wait_until("third join", || joins() == 3);
    fail();
    rig.wait_until("fourth join", || joins() == 4);
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn joining_another_room_forgets_the_people_of_the_first() {
    let rig = rig(Settings::default());
    rig.join();
    rig.wire.emit(TransportEvent::ParticipantJoined {
        identity: "u1".into(),
        name: "Anna".into(),
    });
    rig.wait("anna listed", |e| match e {
        Event::Participants { participants } if participants.len() == 1 => Some(()),
        _ => None,
    });
    // A real transport says nothing more about the session it replaces.
    rig.engine.send(Command::Join {
        url: "ws://other".into(),
        token: "tok2".into(),
    });
    rig.wait("the first room is empty for us", |e| match e {
        Event::Participants { participants } if participants.is_empty() => Some(()),
        _ => None,
    });
    rig.wait_until("second join", || rig.wire.calls().len() == 2);
}
