//! UI <-> audio engine contract.
//!
//! Commands go into the engine, events and levels come out. Everything is plain data
//! (serde + ts-rs), so the engine can live in-process (Tauri) today and in its own process
//! later without the UI noticing. The TypeScript side is generated: `cargo xtask gen-types`.
//!
//! Bump [`PROTOCOL_VERSION`] on every breaking change; additive changes keep it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Version of this contract. The UI refuses to talk to an engine with a different major.
pub const PROTOCOL_VERSION: u32 = 1;

/// Version of the [`Settings`] schema, stored with the settings file for migrations.
pub const SETTINGS_VERSION: u32 = 1;

/// UI -> engine.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Command {
    /// `None` = system default device.
    SetInputDevice {
        id: Option<String>,
    },
    SetOutputDevice {
        id: Option<String>,
    },
    SetTransmitMode {
        mode: TransmitMode,
    },
    SetMuted {
        muted: bool,
    },
    SetDeafened {
        deafened: bool,
    },
    /// Receive side: per remote user and track.
    SetPeerVolume {
        identity: String,
        track: TrackKind,
        gain_db: f32,
    },
    SetPeerMuted {
        identity: String,
        track: TrackKind,
        muted: bool,
    },
    /// Replaces the whole settings object (simple and pro view write the same schema).
    ApplySettings {
        settings: Settings,
    },
    /// Connect to a LiveKit room with a token from the control plane.
    Join {
        url: String,
        token: String,
    },
    Leave,
    /// Ask for a fresh [`Event::Devices`] (a headset was plugged in).
    RefreshDevices,
    /// The UI (re)attached and missed what happened so far: send the current state again —
    /// ready, devices, connection, room and participants.
    Resync,
}

/// Engine -> UI.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Event {
    Ready {
        protocol_version: u32,
    },
    Devices {
        inputs: Vec<AudioDevice>,
        outputs: Vec<AudioDevice>,
    },
    /// Sent at a fixed rate (~30 Hz) while audio runs; never from the audio callback itself.
    Levels {
        input_db: f32,
        gate_open: bool,
        peers: Vec<PeerLevel>,
    },
    Connection {
        state: ConnectionState,
    },
    Error {
        message: String,
    },
    /// We are in a room. Room and identity come from the token.
    Joined {
        room: String,
        identity: String,
    },
    /// The complete list of the others in the room, sent on every change.
    Participants {
        participants: Vec<Participant>,
    },
    /// About once per second while audio runs — for the debug panel and the log.
    Stats {
        stats: Stats,
    },
    /// What the engine and the transport log at info level and above, as it happens.
    Log {
        entry: LogEntry,
    },
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransmitMode {
    VoiceActivation,
    PushToTalk,
    AlwaysOn,
}

/// The three published tracks per participant (see planning doc, "Quellen und Tracks").
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrackKind {
    Voice,
    Music,
    Soundboard,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DenoiserKind {
    Off,
    /// RNNoise class (nnnoiseless): little CPU.
    Rnnoise,
    /// DeepFilterNet: best quality, default on gaming PCs.
    DeepFilter,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PeerLevel {
    pub identity: String,
    pub level_db: f32,
    pub speaking: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Participant {
    pub identity: String,
    pub name: String,
    /// We receive a voice track from them (otherwise they are in the room but silent to us).
    pub has_voice: bool,
    /// They muted their own microphone.
    pub muted: bool,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogEntry {
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub time_ms: u64,
    pub level: LogLevel,
    /// Where it came from, e.g. `transport::livekit`.
    pub target: String,
    pub message: String,
}

/// One snapshot of everything worth watching while a call runs. `None` = not known (yet).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Stats {
    pub network: Option<NetworkStats>,
    pub capture: Option<DeviceStats>,
    pub playout: Option<DeviceStats>,
    /// Longest gap between two 10 ms blocks handed to the transport since the last
    /// snapshot — far above 10 means the send side stalled.
    pub send_interval_max_ms: f32,
    /// Capture blocks thrown away because a backlog had built up (total).
    pub send_blocks_dropped: u32,
    /// Received frames thrown away because a speaker's playout buffer was full (total).
    pub playout_frames_dropped: u32,
    /// Times playout jumped forward because a speaker's buffer ran over — their clock is
    /// faster than our output device, or playout stalled (total).
    pub playout_skips: u32,
}

/// From the transport's WebRTC statistics. "Recent" values cover the time since the
/// previous snapshot, the others are totals for this connection.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkStats {
    /// Round trip to the media server (ICE connectivity checks).
    pub rtt_ms: f32,
    pub send_codec: String,
    pub send_kbps_recent: f32,
    pub packets_sent: u32,
    /// Share of our packets the server reported missing, 0..100.
    pub uplink_loss_percent: f32,
    pub packets_received: u32,
    pub packets_lost: u32,
    pub loss_percent_recent: f32,
    pub jitter_ms: f32,
    /// Mean time a sample waited in the jitter buffer.
    pub jitter_buffer_ms: f32,
    /// Share of played samples the decoder had to invent — what loss sounds like.
    pub concealed_percent_recent: f32,
    pub concealed_percent_total: f32,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceStats {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Largest callback buffer seen, in frames — the effective device buffer.
    pub callback_frames: u32,
    /// Capture: samples lost because the engine did not keep up. Playout: times a
    /// speaker's buffer ran dry.
    pub xruns: u32,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqBand {
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompressorSettings {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}

/// Everything the user can set. Simple and pro view both read and write this.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    pub version: u32,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub transmit_mode: TransmitMode,
    /// Voice activation threshold, dBFS.
    pub vad_threshold_db: f32,
    pub vad_hold_ms: u32,
    pub denoiser: DenoiserKind,
    pub high_pass_hz: f32,
    pub eq: Vec<EqBand>,
    pub compressor: CompressorSettings,
    /// Voice track bitrate; the baseline asks for 64–96 kbps with RED, no DTX.
    pub voice_bitrate_kbps: u32,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            input_device: None,
            output_device: None,
            transmit_mode: TransmitMode::VoiceActivation,
            vad_threshold_db: -50.0,
            vad_hold_ms: 300,
            denoiser: DenoiserKind::DeepFilter,
            high_pass_hz: 80.0,
            eq: Vec::new(),
            compressor: CompressorSettings {
                enabled: true,
                threshold_db: -18.0,
                ratio: 3.0,
                attack_ms: 5.0,
                release_ms: 120.0,
                makeup_db: 3.0,
            },
            voice_bitrate_kbps: 80,
            theme: "dark".into(),
        }
    }
}

impl Settings {
    /// Sanity limits the engine relies on. Returns the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SETTINGS_VERSION {
            return Err(format!(
                "settings version {} != {SETTINGS_VERSION}",
                self.version
            ));
        }
        if !(16..=510).contains(&self.voice_bitrate_kbps) {
            return Err(format!(
                "voice bitrate {} kbps out of 16..=510",
                self.voice_bitrate_kbps
            ));
        }
        if !(-90.0..=0.0).contains(&self.vad_threshold_db) {
            return Err(format!(
                "vad threshold {} dBFS out of -90..=0",
                self.vad_threshold_db
            ));
        }
        if self.eq.len() > 5 {
            return Err(format!("{} EQ bands, at most 5", self.eq.len()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_internally_tagged_camel_case() {
        let json = serde_json::to_value(Command::SetPeerVolume {
            identity: "u1".into(),
            track: TrackKind::Soundboard,
            gain_db: -6.0,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "setPeerVolume", "identity": "u1", "track": "soundboard", "gainDb": -6.0
            })
        );
    }

    #[test]
    fn events_round_trip() {
        let ev = Event::Levels {
            input_db: -23.5,
            gate_open: true,
            peers: vec![PeerLevel {
                identity: "u2".into(),
                level_db: -40.0,
                speaking: false,
            }],
        };
        let back: Event = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(ev, back);
    }

    #[test]
    fn stats_use_null_for_unknown_and_camel_case() {
        let json = serde_json::to_value(Event::Stats {
            stats: Stats {
                send_interval_max_ms: 10.5,
                ..Stats::default()
            },
        })
        .unwrap();
        assert_eq!(json["type"], "stats");
        assert_eq!(json["stats"]["network"], serde_json::Value::Null);
        assert_eq!(json["stats"]["sendIntervalMaxMs"], 10.5);
    }

    #[test]
    fn default_settings_are_valid_and_round_trip() {
        let s = Settings::default();
        s.validate().unwrap();
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }

    /// Writes `Settings::default()` next to the generated TS types, so the UI uses the real
    /// defaults instead of a hand-copied mock (runs with `cargo xtask gen-types` / tests).
    #[test]
    fn export_default_settings_json() {
        let Some(dir) = std::env::var_os("TS_RS_EXPORT_DIR") else {
            return;
        };
        let json = serde_json::to_string_pretty(&Settings::default()).unwrap()
            + "
";
        let path = std::path::Path::new(&dir).join("defaultSettings.json");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(path, json).unwrap();
        }
    }

    #[test]
    fn validate_rejects_out_of_range_bitrate() {
        let s = Settings {
            voice_bitrate_kbps: 4,
            ..Settings::default()
        };
        assert!(s.validate().is_err());
    }
}
