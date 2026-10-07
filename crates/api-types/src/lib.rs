//! Client <-> control-plane contract (HTTP JSON).
//!
//! The control plane (`crates/control`, spike S6) and the desktop client both depend on this
//! crate; the client's TypeScript side is generated with `cargo xtask gen-types`. Changes are
//! additive only, so an N-1 client keeps working (planning doc, "Kompatibilität").

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Bumped only on breaking changes; the server answers N-1 compatibly.
pub const API_VERSION: u32 = 1;

/// Every endpoint answers with this envelope.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiError {
    /// Stable machine-readable code, e.g. `invite_invalid`, `version_too_old`.
    pub code: String,
    pub message: String,
}

/// First login: an invite link carries the code.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RedeemInviteRequest {
    pub code: String,
    pub display_name: String,
    pub client: ClientInfo,
}

/// Sent on login so the control plane can enforce the minimum version.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientInfo {
    pub app_version: String,
    pub protocol_version: u32,
    pub platform: String,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Session {
    pub user: User,
    /// Bearer token for later requests; kept in the OS keychain by the client.
    pub session_token: String,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Role {
    Member,
    Admin,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    pub id: String,
    pub display_name: String,
    pub role: Role,
}

/// A voice channel. Its id is the LiveKit room name.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub position: u32,
    /// Media server the channel is currently assigned to (changes on failover).
    pub server_id: String,
}

#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VoiceTokenRequest {
    pub channel_id: String,
}

/// Long-lived (≈ one gaming evening) so reconnects survive a control-plane outage.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VoiceToken {
    pub ws_url: String,
    pub token: String,
    /// RFC 3339.
    pub expires_at: String,
}

/// Who sits where; pushed to clients, built from LiveKit webhooks + heartbeats.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Presence {
    pub channel_id: String,
    pub user_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_on_the_wire() {
        let json = serde_json::to_value(VoiceToken {
            ws_url: "wss://voice.example".into(),
            token: "t".into(),
            expires_at: "2026-10-07T23:00:00Z".into(),
        })
        .unwrap();
        assert_eq!(json["wsUrl"], "wss://voice.example");
        assert_eq!(json["expiresAt"], "2026-10-07T23:00:00Z");
    }

    #[test]
    fn role_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Role::Admin).unwrap(), "\"admin\"");
    }
}
