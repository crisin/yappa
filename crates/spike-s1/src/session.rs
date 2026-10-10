//! One LiveKit participant: connect with a dev token, publish one audio track fed by a pump
//! thread, hand every subscribed audio track to a receive task, sample WebRTC stats.

use crate::click::{ClickDetector, ClickGen, BLOCK, SAMPLE_RATE};
use futures_util::StreamExt;
use livekit::options::{AudioEncoding, TrackPublishOptions};
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::stats::RtcStats;
use livekit_api::access_token::{AccessToken, VideoGrants};
use rtrb::{Consumer, Producer};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, String>;

/// One clock and one event log for every participant in this process.
pub struct Log {
    start: Instant,
    /// When a click block was handed on (seconds since start).
    pub sent: Mutex<Vec<f64>>,
    /// When a click onset came out of a subscribed track.
    pub detected: Mutex<Vec<f64>>,
    /// Connection events, for the reconnect timeline.
    pub events: Mutex<Vec<(f64, String)>>,
}

impl Log {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            start: Instant::now(),
            sent: Mutex::default(),
            detected: Mutex::default(),
            events: Mutex::default(),
        })
    }

    pub fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    pub fn event(&self, who: &str, what: impl std::fmt::Display) {
        let t = self.now();
        let line = format!("{who}: {what}");
        println!("[{t:8.3} s] {line}");
        self.events.lock().unwrap().push((t, line));
    }
}

#[derive(Clone)]
pub struct ClientConfig {
    pub url: String,
    pub room: String,
    pub identity: String,
    pub bitrate: u64,
    pub red: bool,
    pub dtx: bool,
}

/// What this participant publishes.
pub enum Source {
    Nothing,
    Silence,
    Click { period_ms: u32, noise: f32 },
    Mic(Consumer<i16>),
}

/// What happens to the audio this participant receives.
#[derive(Default)]
pub struct Sink {
    /// Detect click onsets with this threshold (share of full scale).
    pub detect: Option<f32>,
    /// Rings into the playout mix; each subscribed track takes one while it lasts.
    pub playout: Arc<Mutex<Vec<Producer<i16>>>>,
}

#[derive(Default)]
pub struct PumpStats {
    pub blocks: AtomicU64,
    /// Time spent in `capture_frame` per 10 ms block.
    pub handoff_ns_sum: AtomicU64,
    pub handoff_ns_max: AtomicU64,
    /// Longest distance between two blocks — pacing jitter of the source.
    pub interval_ns_max: AtomicU64,
}

#[derive(Default)]
pub struct ReceiveStats {
    pub frames: AtomicU64,
    /// Frames thrown away because the playout ring was full (clock drift, stalls).
    pub dropped_frames: AtomicU64,
}

pub struct Client {
    pub room: Arc<Room>,
    pub pump: Arc<PumpStats>,
    pub receive: Arc<ReceiveStats>,
    stop: Arc<AtomicBool>,
    pump_thread: Option<std::thread::JoinHandle<()>>,
}

impl Client {
    pub async fn start(
        cfg: ClientConfig,
        source: Source,
        sink: Sink,
        log: Arc<Log>,
    ) -> Result<Self> {
        let token = dev_token(&cfg)?;
        let (room, events) = Room::connect(&cfg.url, &token, RoomOptions::default())
            .await
            .map_err(|e| format!("{}: connect to {} failed: {e}", cfg.identity, cfg.url))?;
        let room = Arc::new(room);
        log.event(&cfg.identity, format!("joined room '{}'", room.name()));

        let receive = Arc::new(ReceiveStats::default());
        tokio::spawn(event_loop(
            cfg.identity.clone(),
            events,
            sink,
            log.clone(),
            receive.clone(),
        ));

        let stop = Arc::new(AtomicBool::new(false));
        let pump = Arc::new(PumpStats::default());
        let pump_thread = match source {
            Source::Nothing => None,
            source => {
                let native = publish(&room, &cfg).await?;
                log.event(
                    &cfg.identity,
                    format!(
                        "published voice track: {} bit/s, red={}, dtx={}",
                        cfg.bitrate, cfg.red, cfg.dtx
                    ),
                );
                let ctx = Pump {
                    runtime: tokio::runtime::Handle::current(),
                    native,
                    log: log.clone(),
                    stop: stop.clone(),
                    stats: pump.clone(),
                };
                Some(
                    std::thread::Builder::new()
                        .name(format!("pump-{}", cfg.identity))
                        .spawn(move || ctx.run(source))
                        .map_err(|e| e.to_string())?,
                )
            }
        };
        Ok(Self {
            room,
            pump,
            receive,
            stop,
            pump_thread,
        })
    }

    pub async fn close(mut self) {
        self.stop.store(true, Relaxed);
        if let Some(t) = self.pump_thread.take() {
            let _ = tokio::task::spawn_blocking(move || t.join()).await;
        }
        let _ = self.room.close().await;
    }
}

/// A token signed with the dev keys — what the control plane will issue later.
fn dev_token(cfg: &ClientConfig) -> Result<String> {
    let key = std::env::var("LIVEKIT_API_KEY").unwrap_or_else(|_| "devkey".into());
    let secret = std::env::var("LIVEKIT_API_SECRET").unwrap_or_else(|_| "secret".into());
    AccessToken::with_api_key(&key, &secret)
        .with_identity(&cfg.identity)
        .with_name(&cfg.identity)
        .with_ttl(Duration::from_secs(12 * 3600))
        .with_grants(VideoGrants {
            room_join: true,
            room: cfg.room.clone(),
            ..Default::default()
        })
        .to_jwt()
        .map_err(|e| format!("token: {e}"))
}

async fn publish(room: &Room, cfg: &ClientConfig) -> Result<NativeAudioSource> {
    // queue_size_ms = 0: every 10 ms block goes straight to the encoder, no extra buffer.
    let native = NativeAudioSource::new(AudioSourceOptions::default(), SAMPLE_RATE, 1, 0);
    let track =
        LocalAudioTrack::create_audio_track("voice", RtcAudioSource::Native(native.clone()));
    let options = TrackPublishOptions {
        audio_encoding: Some(AudioEncoding {
            max_bitrate: cfg.bitrate,
        }),
        red: cfg.red,
        dtx: cfg.dtx,
        source: TrackSource::Microphone,
        ..Default::default()
    };
    room.local_participant()
        .publish_track(LocalTrack::Audio(track), options)
        .await
        .map_err(|e| format!("{}: publish failed: {e}", cfg.identity))?;
    Ok(native)
}

struct Pump {
    runtime: tokio::runtime::Handle,
    native: NativeAudioSource,
    log: Arc<Log>,
    stop: Arc<AtomicBool>,
    stats: Arc<PumpStats>,
}

impl Pump {
    /// Feeds the track 10 ms at a time: paced by the capture ring for the microphone, by
    /// the clock for synthetic sources.
    fn run(self, mut source: Source) {
        const TICK: Duration = Duration::from_millis(10);
        let mut click = match source {
            Source::Click { period_ms, noise } => Some(ClickGen::new(period_ms).with_noise(noise)),
            _ => None,
        };
        let mut block = [0i16; BLOCK];
        let mut deadline = Instant::now();
        let mut last = None::<Instant>;
        while !self.stop.load(Relaxed) {
            let mut is_click = false;
            match &mut source {
                Source::Mic(ring) => {
                    if ring.slots() < BLOCK {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    // A backlog (capture started before we published, or the pump stalled)
                    // must not be sent as a burst: it would sit in the receiver's jitter
                    // buffer as standing delay. Keep the newest block only.
                    while ring.slots() > 3 * BLOCK {
                        let _ = ring.pop();
                    }
                    block.fill_with(|| ring.pop().unwrap_or(0));
                }
                _ => {
                    deadline += TICK;
                    let now = Instant::now();
                    if deadline > now {
                        std::thread::sleep(deadline - now);
                    } else if now - deadline > 10 * TICK {
                        deadline = now; // stalled (suspend, debugger): do not burst
                    }
                    match &mut click {
                        Some(gen) => is_click = gen.fill(&mut block),
                        None => block.fill(0),
                    }
                }
            }
            let frame = AudioFrame {
                data: Cow::Borrowed(&block[..]),
                sample_rate: SAMPLE_RATE,
                num_channels: 1,
                samples_per_channel: BLOCK as u32,
            };
            let start = Instant::now();
            if is_click {
                self.log.sent.lock().unwrap().push(self.log.now());
            }
            if let Err(e) = self.runtime.block_on(self.native.capture_frame(&frame)) {
                eprintln!("capture_frame: {e}");
            }
            let handoff = start.elapsed().as_nanos() as u64;
            self.stats.blocks.fetch_add(1, Relaxed);
            self.stats.handoff_ns_sum.fetch_add(handoff, Relaxed);
            self.stats.handoff_ns_max.fetch_max(handoff, Relaxed);
            if let Some(last) = last {
                let interval = (start - last).as_nanos() as u64;
                self.stats.interval_ns_max.fetch_max(interval, Relaxed);
            }
            last = Some(start);
        }
    }
}

async fn event_loop(
    who: String,
    mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
    sink: Sink,
    log: Arc<Log>,
    stats: Arc<ReceiveStats>,
) {
    let sink = Arc::new(sink);
    while let Some(event) = events.recv().await {
        match event {
            RoomEvent::TrackSubscribed {
                track: RemoteTrack::Audio(track),
                participant,
                ..
            } => {
                log.event(&who, format!("hears {}", participant.identity()));
                tokio::spawn(receive(track, sink.clone(), log.clone(), stats.clone()));
            }
            RoomEvent::TrackUnsubscribed { participant, .. } => {
                log.event(&who, format!("lost track of {}", participant.identity()));
            }
            RoomEvent::ParticipantConnected(p) => {
                log.event(&who, format!("{} joined", p.identity()));
            }
            RoomEvent::ParticipantDisconnected(p) => {
                log.event(&who, format!("{} left", p.identity()));
            }
            RoomEvent::ConnectionStateChanged(state) => {
                log.event(&who, format!("connection state {state:?}"));
            }
            RoomEvent::Reconnecting => log.event(&who, "RECONNECTING"),
            RoomEvent::Reconnected => log.event(&who, "RECONNECTED"),
            RoomEvent::Disconnected { reason } => {
                log.event(&who, format!("DISCONNECTED ({reason:?})"));
            }
            _ => {}
        }
    }
}

/// Decoded 48 kHz mono frames of one remote speaker: detect clicks, queue for playout.
async fn receive(
    track: RemoteAudioTrack,
    sink: Arc<Sink>,
    log: Arc<Log>,
    stats: Arc<ReceiveStats>,
) {
    let mut stream = NativeAudioStream::new(track.rtc_track(), SAMPLE_RATE as i32, 1);
    let mut detector = sink.detect.map(|t| ClickDetector::new(t, 200));
    let mut ring = sink.playout.lock().unwrap().pop();
    while let Some(frame) = stream.next().await {
        let arrived = log.now();
        stats.frames.fetch_add(1, Relaxed);
        if let Some(i) = detector.as_mut().and_then(|d| d.scan(&frame.data)) {
            // Sample i of this frame would leave a zero-latency output i samples from now.
            let onset = arrived + i as f64 / f64::from(SAMPLE_RATE);
            log.detected.lock().unwrap().push(onset);
        }
        if let Some(ring) = &mut ring {
            if ring.slots() < frame.data.len() {
                stats.dropped_frames.fetch_add(1, Relaxed);
            } else {
                for s in frame.data.iter() {
                    let _ = ring.push(*s);
                }
            }
        }
    }
    if let Some(ring) = ring {
        sink.playout.lock().unwrap().push(ring);
    }
}

/// The WebRTC counters the spike reports, summed over this participant's audio streams.
#[derive(Clone, Debug, Default)]
pub struct NetStats {
    pub codec_sent: String,
    pub codec_received: String,
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub packets_received: u64,
    pub packets_lost: i64,
    pub jitter_ms: f64,
    pub jitter_buffer_delay_s: f64,
    pub jitter_buffer_emitted: u64,
    pub samples_received: u64,
    pub concealed_samples: u64,
    pub concealment_events: u64,
    pub nack_count: u32,
    pub retransmitted_received: u64,
    pub fec_packets_received: u64,
    /// As the SFU reports our uplink (RTCP receiver reports).
    pub uplink_rtt_ms: f64,
    pub uplink_fraction_lost: f64,
    /// Round trip of the ICE connectivity checks on the active candidate pair.
    pub ice_rtt_ms: f64,
}

impl NetStats {
    pub async fn sample(room: &Room) -> Result<Self> {
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
        let codec = |id: &str| codecs.get(id).copied().unwrap_or("?").to_string();

        let mut out = Self::default();
        // With a single peer connection both lists carry the same reports.
        let mut seen = std::collections::HashSet::new();
        for stat in all {
            match stat {
                RtcStats::InboundRtp(s) if s.stream.kind == "audio" && seen.insert(&s.rtc.id) => {
                    out.codec_received = codec(&s.stream.codec_id);
                    out.packets_received += s.received.packets_received;
                    out.packets_lost += s.received.packets_lost;
                    out.jitter_ms = out.jitter_ms.max(s.received.jitter * 1000.0);
                    out.jitter_buffer_delay_s += s.inbound.jitter_buffer_delay;
                    out.jitter_buffer_emitted += s.inbound.jitter_buffer_emitted_count;
                    out.samples_received += s.inbound.total_samples_received;
                    out.concealed_samples += s.inbound.concealed_samples;
                    out.concealment_events += s.inbound.concealment_events;
                    out.nack_count += s.inbound.nack_count;
                    out.retransmitted_received += s.inbound.retransmitted_packets_received;
                    out.fec_packets_received += s.inbound.fec_packets_received;
                }
                RtcStats::OutboundRtp(s) if s.stream.kind == "audio" && seen.insert(&s.rtc.id) => {
                    out.codec_sent = codec(&s.stream.codec_id);
                    out.packets_sent += s.sent.packets_sent;
                    out.bytes_sent += s.sent.bytes_sent;
                }
                RtcStats::RemoteInboundRtp(s)
                    if s.stream.kind == "audio" && seen.insert(&s.rtc.id) =>
                {
                    out.uplink_rtt_ms = s.remote_inbound.round_trip_time * 1000.0;
                    out.uplink_fraction_lost = s.remote_inbound.fraction_lost;
                }
                RtcStats::CandidatePair(s) if s.candidate_pair.nominated => {
                    out.ice_rtt_ms = s.candidate_pair.current_round_trip_time * 1000.0;
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// Mean time a sample spent in the jitter buffer, in ms.
    pub fn jitter_buffer_ms(&self) -> Option<f64> {
        (self.jitter_buffer_emitted > 0)
            .then(|| self.jitter_buffer_delay_s / self.jitter_buffer_emitted as f64 * 1000.0)
    }

    /// Share of played samples that were invented by loss concealment, in percent.
    pub fn concealed_percent(&self) -> Option<f64> {
        (self.samples_received > 0)
            .then(|| self.concealed_samples as f64 / self.samples_received as f64 * 100.0)
    }
}
