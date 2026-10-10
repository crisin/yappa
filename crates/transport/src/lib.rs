//! The seam between the audio engine and the media plane.
//!
//! The engine hands over its voice as 10 ms blocks of 48 kHz mono PCM and gets every remote
//! speaker back the same way; everything else is plain events. Nothing in this module knows
//! LiveKit — the implementation lives in [`livekit`] behind the cargo feature of the same
//! name, so another SFU (or a fake in tests) only has to implement [`Transport`].
//!
//! Threads: none of this is for the real-time audio callback. [`VoiceOut::send`] is called
//! from the engine's pump thread, [`FrameSink::frame`] from a transport thread; both sit
//! behind lock-free rings on the engine side.

#[cfg(feature = "livekit")]
pub mod livekit;

use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 48_000;
/// Samples in one 10 ms mono block — the unit on both directions of the seam.
pub const BLOCK: usize = 480;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishOptions {
    /// Upper limit for the voice track; Opus stays variable below it.
    pub bitrate_bps: u32,
    /// Redundant audio (each packet carries the previous frame again).
    pub red: bool,
    /// Stop sending during silence. Off by default: quality over thrift.
    pub dtx: bool,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            bitrate_bps: 64_000,
            red: true,
            dtx: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Disconnected,
    Connecting,
    Connected,
    /// The link dropped and the transport is trying to get it back by itself.
    Reconnecting,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransportEvent {
    State(LinkState),
    /// The join went through. Room and identity are what the token said.
    Joined {
        room: String,
        identity: String,
    },
    /// A join did not work, or an established session ended without [`Transport::leave`].
    /// Always followed by `State(Disconnected)`. `retryable` is false when joining again
    /// with the same token cannot help or would do harm: thrown out, the same identity
    /// joined elsewhere, the room is gone, the token was refused.
    Failed {
        message: String,
        retryable: bool,
    },
    ParticipantJoined {
        identity: String,
        name: String,
    },
    ParticipantLeft {
        identity: String,
    },
    /// The participant muted or unmuted their own microphone.
    ParticipantMuted {
        identity: String,
        muted: bool,
    },
    /// Audio from this participant starts or stops arriving at the [`AudioReceiver`].
    VoiceStarted {
        identity: String,
    },
    VoiceEnded {
        identity: String,
    },
    Stats(NetStats),
}

/// Counters for our audio streams, totals since the join. Rates are the engine's job.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetStats {
    /// Round trip of the connectivity checks to the media server.
    pub rtt_ms: f64,
    pub send_codec: String,
    pub packets_sent: u64,
    pub bytes_sent: u64,
    /// Share of our packets the server reported missing in its last report, 0..1.
    pub uplink_fraction_lost: f64,
    pub packets_received: u64,
    pub packets_lost: u64,
    pub jitter_ms: f64,
    /// Sum of the time samples spent in the jitter buffer, and how many samples that was.
    pub jitter_buffer_delay_s: f64,
    pub jitter_buffer_emitted: u64,
    pub samples_received: u64,
    /// Samples the decoder invented to cover losses.
    pub concealed_samples: u64,
}

/// Where the transport delivers remote audio. Implemented by the engine.
pub trait AudioReceiver: Send + Sync {
    /// A remote participant's voice starts. The returned sink gets every decoded 10 ms
    /// frame and is dropped when that voice ends — its lifetime is the track's lifetime.
    fn speaker_started(&self, identity: &str) -> Box<dyn FrameSink>;
}

pub trait FrameSink: Send {
    /// 48 kHz mono.
    fn frame(&mut self, samples: &[i16]);
}

/// Where the engine delivers its own voice.
pub trait VoiceOut: Send + Sync {
    /// One [`BLOCK`] of 48 kHz mono. Silently does nothing while not connected.
    fn send(&self, block: &[i16]);
}

pub type EventFn = Box<dyn Fn(TransportEvent) + Send + Sync>;

/// One voice session at a time. Every call returns at once; outcomes arrive as events.
pub trait Transport: Send {
    /// Called once before anything else: where events and remote audio go.
    fn attach(&mut self, events: EventFn, receiver: Arc<dyn AudioReceiver>);
    /// Leaves the current session first, if there is one.
    fn join(&mut self, url: &str, token: &str, options: PublishOptions);
    fn leave(&mut self);
    /// Tells the others we are muted. The engine also sends silence, so this is courtesy
    /// towards their UI, not the mute itself.
    fn set_muted(&mut self, muted: bool);
    fn voice_out(&self) -> Arc<dyn VoiceOut>;
}
