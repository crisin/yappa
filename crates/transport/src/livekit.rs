//! [`Transport`] on the LiveKit Rust SDK.
//!
//! Owns a small tokio runtime; one task per session drives the room, one task per subscribed
//! voice track feeds the engine's sink. Publish settings follow spike S1: custom source with
//! `queue_size_ms = 0` (each 10 ms block goes straight to the encoder), RED on, DTX off.

use crate::{
    AudioReceiver, EventFn, LinkState, NetStats, PublishOptions, Transport, TransportEvent,
    VoiceOut, BLOCK, SAMPLE_RATE,
};
use futures_util::StreamExt;
use livekit::options::{AudioEncoding, TrackPublishOptions};
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::webrtc::audio_stream::native::{NativeAudioStream, NativeAudioStreamOptions};
use livekit::webrtc::stats::RtcStats;
use livekit::DisconnectReason;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// Decoded frames the SDK may queue per speaker before it drops the oldest: 100 ms. The
/// engine's own ring behind it decides what is actually played.
const RECEIVE_QUEUE_FRAMES: usize = 10;
const STATS_INTERVAL: Duration = Duration::from_secs(1);
/// How long shutdown waits for the server to hear our goodbye.
const LEAVE_TIMEOUT: Duration = Duration::from_secs(2);

/// How a session ended without being asked to.
struct Lost {
    message: String,
    retryable: bool,
}

pub struct LiveKitTransport {
    runtime: tokio::runtime::Runtime,
    voice: Arc<Voice>,
    hooks: Option<Hooks>,
    session: Option<mpsc::UnboundedSender<Control>>,
    /// The session's task, to wait for a clean goodbye at shutdown.
    task: Option<tokio::task::JoinHandle<()>>,
    /// Number of the session whose events count; older ones are talking to nobody.
    current: Arc<AtomicU64>,
    muted: bool,
}

#[derive(Clone)]
struct Hooks {
    events: Arc<EventFn>,
    receiver: Arc<dyn AudioReceiver>,
    session: u64,
    current: Arc<AtomicU64>,
}

impl Hooks {
    /// A session replaced by a newer join keeps quiet: its late "disconnected" must not
    /// overwrite what the new one reports.
    fn emit(&self, event: TransportEvent) {
        if self.current.load(Relaxed) == self.session {
            (self.events)(event);
        }
    }
}

enum Control {
    Leave,
    Mute(bool),
}

impl LiveKitTransport {
    pub fn new() -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("yappa-transport")
            .enable_all()
            .build()?;
        // The source outlives sessions: every join publishes a new track from it.
        let source = {
            let _in_runtime = runtime.enter();
            NativeAudioSource::new(AudioSourceOptions::default(), SAMPLE_RATE, 1, 0)
        };
        let voice = Arc::new(Voice {
            source,
            runtime: runtime.handle().clone(),
            live_session: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        });
        Ok(Self {
            runtime,
            voice,
            hooks: None,
            session: None,
            task: None,
            current: Arc::new(AtomicU64::new(0)),
            muted: false,
        })
    }
}

impl Transport for LiveKitTransport {
    fn attach(&mut self, events: EventFn, receiver: Arc<dyn AudioReceiver>) {
        self.hooks = Some(Hooks {
            events: Arc::new(events),
            receiver,
            session: 0,
            current: self.current.clone(),
        });
    }

    fn join(&mut self, url: &str, token: &str, options: PublishOptions) {
        self.leave();
        let Some(mut hooks) = self.hooks.clone() else {
            tracing::error!("join before attach — ignored");
            return;
        };
        hooks.session = self.current.fetch_add(1, Relaxed) + 1;
        let (control, control_rx) = mpsc::unbounded_channel();
        self.session = Some(control);
        let session = Session {
            url: url.to_string(),
            token: token.to_string(),
            options,
            muted: self.muted,
            voice: self.voice.clone(),
            hooks,
        };
        self.task = Some(self.runtime.spawn(session.run(control_rx)));
    }

    fn leave(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = session.send(Control::Leave);
        }
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if let Some(session) = &self.session {
            let _ = session.send(Control::Mute(muted));
        }
    }

    fn voice_out(&self) -> Arc<dyn VoiceOut> {
        self.voice.clone()
    }
}

/// Tells the server goodbye instead of leaving a ghost in the room until it times out.
impl Drop for LiveKitTransport {
    fn drop(&mut self) {
        self.leave();
        if let Some(task) = self.task.take() {
            // The timer must be created inside the runtime.
            let _ = self
                .runtime
                .block_on(async { tokio::time::timeout(LEAVE_TIMEOUT, task).await });
        }
    }
}

struct Voice {
    source: NativeAudioSource,
    runtime: tokio::runtime::Handle,
    /// Number of the session that has a track published, 0 for none; blocks are dropped
    /// here otherwise. A number, not a flag, so an old session that ends late cannot
    /// switch off the voice of the current one.
    live_session: AtomicU64,
    errors: AtomicU64,
}

impl VoiceOut for Voice {
    fn send(&self, block: &[i16]) {
        if self.live_session.load(Relaxed) == 0 || block.len() != BLOCK {
            return;
        }
        let frame = AudioFrame {
            data: Cow::Borrowed(block),
            sample_rate: SAMPLE_RATE,
            num_channels: 1,
            samples_per_channel: BLOCK as u32,
        };
        // Without a queue `capture_frame` completes synchronously; block_on only drives it.
        if let Err(e) = self.runtime.block_on(self.source.capture_frame(&frame)) {
            // A dead link fails 100 times per second: log the first and then every 10 s.
            let n = self.errors.fetch_add(1, Relaxed);
            if n.is_multiple_of(1000) {
                tracing::warn!(error = %e, failures = n + 1, "handing a voice block to LiveKit failed");
            }
        }
    }
}

struct Session {
    url: String,
    token: String,
    options: PublishOptions,
    muted: bool,
    voice: Arc<Voice>,
    hooks: Hooks,
}

impl Session {
    async fn run(self, mut control: mpsc::UnboundedReceiver<Control>) {
        let hooks = self.hooks.clone();
        hooks.emit(TransportEvent::State(LinkState::Connecting));
        tracing::info!(url = %self.url, "joining");
        if let Err(Lost { message, retryable }) = self.session(&mut control).await {
            tracing::error!(%message, retryable, "voice session failed");
            hooks.emit(TransportEvent::Failed { message, retryable });
        }
        self.silence();
        hooks.emit(TransportEvent::State(LinkState::Disconnected));
    }

    /// Stops our voice, unless a newer session has taken over in the meantime.
    fn silence(&self) {
        let _ = self
            .voice
            .live_session
            .compare_exchange(self.hooks.session, 0, Relaxed, Relaxed);
    }

    /// `Ok` = left on request, `Err` = could not join or lost the session.
    async fn session(&self, control: &mut mpsc::UnboundedReceiver<Control>) -> Result<(), Lost> {
        let hooks = &self.hooks;
        let mut muted = self.muted;
        // A leave must not wait for a connect that may hang for seconds.
        let connect = Room::connect(&self.url, &self.token, RoomOptions::default());
        tokio::pin!(connect);
        let connected = loop {
            tokio::select! {
                result = &mut connect => break result,
                message = control.recv() => match message {
                    Some(Control::Mute(now)) => muted = now,
                    Some(Control::Leave) | None => {
                        tracing::info!("left while connecting");
                        return Ok(());
                    }
                },
            }
        };
        let (room, mut events) = connected.map_err(|e| {
            let message = format!("could not connect to {}: {e}", self.url);
            // The server refusing the token will not change its mind.
            let refused = ["401", "403", "Unauthorized", "Forbidden"];
            Lost {
                retryable: !refused.iter().any(|sign| message.contains(sign)),
                message,
            }
        })?;
        let identity = room.local_participant().identity().to_string();
        tracing::info!(room = %room.name(), %identity, "joined");
        hooks.emit(TransportEvent::Joined {
            room: room.name(),
            identity,
        });
        for participant in room.remote_participants().values() {
            hooks.emit(joined(participant));
        }

        let track = LocalAudioTrack::create_audio_track(
            "voice",
            RtcAudioSource::Native(self.voice.source.clone()),
        );
        let publish = TrackPublishOptions {
            audio_encoding: Some(AudioEncoding {
                max_bitrate: u64::from(self.options.bitrate_bps),
            }),
            red: self.options.red,
            dtx: self.options.dtx,
            source: TrackSource::Microphone,
            ..Default::default()
        };
        if let Err(e) = room
            .local_participant()
            .publish_track(LocalTrack::Audio(track.clone()), publish)
            .await
        {
            let _ = room.close().await;
            return Err(Lost {
                message: format!("could not publish the voice track: {e}"),
                retryable: true,
            });
        }
        if muted {
            track.mute();
        }
        tracing::info!(
            bitrate_bps = self.options.bitrate_bps,
            red = self.options.red,
            dtx = self.options.dtx,
            "voice track published"
        );
        self.voice.live_session.store(hooks.session, Relaxed);
        hooks.emit(TransportEvent::State(LinkState::Connected));

        // One receive task per subscribed voice track, ended by the room's events — the
        // SDK's stream does not end by itself when a track goes away.
        let mut speakers: HashMap<TrackSid, (String, AbortHandle)> = HashMap::new();
        let mut stats = tokio::time::interval(STATS_INTERVAL);
        stats.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let result = loop {
            tokio::select! {
                message = control.recv() => match message {
                    Some(Control::Mute(true)) => track.mute(),
                    Some(Control::Mute(false)) => track.unmute(),
                    Some(Control::Leave) | None => {
                        tracing::info!("leaving");
                        break Ok(());
                    }
                },
                _ = stats.tick() => match sample_stats(&room).await {
                    Ok(stats) => hooks.emit(TransportEvent::Stats(stats)),
                    Err(e) => tracing::debug!(error = %e, "no WebRTC stats this round"),
                },
                event = events.recv() => match event {
                    Some(RoomEvent::TrackSubscribed { track: RemoteTrack::Audio(track), participant, publication }) => {
                        let identity = participant.identity().to_string();
                        // Mute changes arrive as events; who was muted before we came does not.
                        if publication.is_muted() {
                            hooks.emit(TransportEvent::ParticipantMuted { identity: identity.clone(), muted: true });
                        }
                        tracing::info!(%identity, track = %track.sid(), "voice track subscribed");
                        let task = tokio::spawn(receive(track.clone(), identity.clone(), hooks.receiver.clone()));
                        if let Some((_, old)) = speakers.insert(track.sid(), (identity.clone(), task.abort_handle())) {
                            old.abort();
                        }
                        hooks.emit(TransportEvent::VoiceStarted { identity });
                    }
                    Some(RoomEvent::TrackUnsubscribed { track, participant, .. }) => {
                        if let Some((identity, task)) = speakers.remove(&track.sid()) {
                            task.abort();
                            tracing::info!(%identity, "voice track ended");
                            hooks.emit(TransportEvent::VoiceEnded { identity });
                        } else {
                            tracing::debug!(identity = %participant.identity(), "untracked track unsubscribed");
                        }
                    }
                    Some(RoomEvent::TrackMuted { participant, .. }) => {
                        if let Some(event) = mute_change(&participant, true, &room) {
                            hooks.emit(event);
                        }
                    }
                    Some(RoomEvent::TrackUnmuted { participant, .. }) => {
                        if let Some(event) = mute_change(&participant, false, &room) {
                            hooks.emit(event);
                        }
                    }
                    Some(RoomEvent::ParticipantConnected(participant)) => {
                        tracing::info!(identity = %participant.identity(), "participant joined");
                        hooks.emit(joined(&participant));
                    }
                    Some(RoomEvent::ParticipantDisconnected(participant)) => {
                        tracing::info!(identity = %participant.identity(), "participant left");
                        hooks.emit(TransportEvent::ParticipantLeft {
                            identity: participant.identity().to_string(),
                        });
                    }
                    Some(RoomEvent::Reconnecting) => {
                        tracing::warn!("link lost — reconnecting");
                        hooks.emit(TransportEvent::State(LinkState::Reconnecting));
                    }
                    Some(RoomEvent::Reconnected) => {
                        tracing::info!("reconnected");
                        hooks.emit(TransportEvent::State(LinkState::Connected));
                    }
                    Some(RoomEvent::ConnectionQualityChanged { quality, participant }) => {
                        tracing::debug!(identity = %participant.identity(), ?quality, "connection quality");
                    }
                    Some(RoomEvent::Disconnected { reason }) => {
                        break match reason {
                            DisconnectReason::ClientInitiated => Ok(()),
                            reason => Err(Lost {
                                message: format!("disconnected by the server or the network: {reason:?}"),
                                // Joining again would throw the other session out in turn,
                                // or knock on a door that was closed on purpose.
                                retryable: !matches!(
                                    reason,
                                    DisconnectReason::DuplicateIdentity
                                        | DisconnectReason::ParticipantRemoved
                                        | DisconnectReason::RoomDeleted
                                ),
                            }),
                        };
                    }
                    Some(_) => {}
                    None => break Err(Lost { message: "the room's event stream ended".into(), retryable: true }),
                },
            }
        };

        self.silence();
        for (_, (identity, task)) in speakers.drain() {
            task.abort();
            hooks.emit(TransportEvent::VoiceEnded { identity });
        }
        let _ = room.close().await;
        result
    }
}

fn joined(participant: &RemoteParticipant) -> TransportEvent {
    let identity = participant.identity().to_string();
    let name = match participant.name() {
        name if name.is_empty() => identity.clone(),
        name => name,
    };
    TransportEvent::ParticipantJoined { identity, name }
}

/// Mute events also arrive for our own track; those say nothing new.
fn mute_change(participant: &Participant, muted: bool, room: &Room) -> Option<TransportEvent> {
    let identity = participant.identity().to_string();
    if identity == room.local_participant().identity().as_str() {
        return None;
    }
    tracing::info!(%identity, muted, "participant changed mute");
    Some(TransportEvent::ParticipantMuted { identity, muted })
}

/// Decoded frames of one remote speaker, until the task is aborted.
async fn receive(track: RemoteAudioTrack, identity: String, receiver: Arc<dyn AudioReceiver>) {
    let mut sink = receiver.speaker_started(&identity);
    let options = NativeAudioStreamOptions {
        queue_size_frames: Some(RECEIVE_QUEUE_FRAMES),
    };
    let mut stream =
        NativeAudioStream::with_options(track.rtc_track(), SAMPLE_RATE as i32, 1, options);
    while let Some(frame) = stream.next().await {
        sink.frame(&frame.data);
    }
}

async fn sample_stats(room: &Room) -> Result<NetStats, String> {
    let session = room.get_stats().await.map_err(|e| e.to_string())?;
    let all: Vec<&RtcStats> = session
        .publisher_stats
        .iter()
        .chain(&session.subscriber_stats)
        .collect();
    let codecs: HashMap<&str, &str> = all
        .iter()
        .filter_map(|s| match s {
            RtcStats::Codec(c) => Some((c.rtc.id.as_str(), c.codec.mime_type.as_str())),
            _ => None,
        })
        .collect();

    let mut out = NetStats::default();
    // With a single peer connection both lists carry the same reports.
    let mut seen = HashSet::new();
    for stat in all {
        match stat {
            RtcStats::InboundRtp(s) if s.stream.kind == "audio" && seen.insert(&s.rtc.id) => {
                out.packets_received += s.received.packets_received;
                out.packets_lost += u64::try_from(s.received.packets_lost).unwrap_or(0);
                out.jitter_ms = out.jitter_ms.max(s.received.jitter * 1000.0);
                out.jitter_buffer_delay_s += s.inbound.jitter_buffer_delay;
                out.jitter_buffer_emitted += s.inbound.jitter_buffer_emitted_count;
                out.samples_received += s.inbound.total_samples_received;
                out.concealed_samples += s.inbound.concealed_samples;
            }
            RtcStats::OutboundRtp(s) if s.stream.kind == "audio" && seen.insert(&s.rtc.id) => {
                out.send_codec = codecs
                    .get(s.stream.codec_id.as_str())
                    .copied()
                    .unwrap_or("?")
                    .to_string();
                out.packets_sent += s.sent.packets_sent;
                out.bytes_sent += s.sent.bytes_sent;
            }
            RtcStats::RemoteInboundRtp(s) if s.stream.kind == "audio" && seen.insert(&s.rtc.id) => {
                out.uplink_fraction_lost = s.remote_inbound.fraction_lost;
            }
            // The RTCP round trip is useless against LiveKit (spike S1); ICE's is real.
            RtcStats::CandidatePair(s) if s.candidate_pair.nominated => {
                out.rtt_ms = s.candidate_pair.current_round_trip_time * 1000.0;
            }
            _ => {}
        }
    }
    Ok(out)
}
