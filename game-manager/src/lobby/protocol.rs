use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A parsed lobby protocol message: `CommandName JSON\n`
#[derive(Debug, Clone)]
pub struct LobbyMessage {
    pub command: String,
    pub data: serde_json::Value,
}

impl LobbyMessage {
    pub fn new(command: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            command: command.into(),
            data,
        }
    }

    /// Serialize to wire format: `CommandName JSON\n`
    pub fn to_wire(&self) -> String {
        format!("{} {}\n", self.command, self.data)
    }

    /// Parse from a single line (without trailing newline).
    pub fn from_line(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        if let Some(space_idx) = line.find(' ') {
            let command = line[..space_idx].to_string();
            let json_str = &line[space_idx + 1..];
            let data = serde_json::from_str(json_str).unwrap_or(serde_json::Value::String(json_str.to_string()));
            Some(LobbyMessage { command, data })
        } else {
            Some(LobbyMessage {
                command: line.to_string(),
                data: serde_json::json!({}),
            })
        }
    }
}

// ── Client → Server commands ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginCommand {
    pub name: String,
    pub password_hash: String,
    #[serde(default)]
    pub user_id: i64,
    #[serde(default, rename = "InstallID")]
    pub install_id: i64,
    #[serde(default)]
    pub lobby_version: i64,
    #[serde(default)]
    pub steam_auth_token: String,
    #[serde(default)]
    pub dlc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SayCommand {
    pub place: i32,
    pub target: String,
    pub text: String,
    pub is_emote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JoinChannelCommand {
    pub channel_name: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LeaveChannelCommand {
    pub channel_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JoinBattleCommand {
    #[serde(rename = "BattleID")]
    pub battle_id: i64,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JoinBattleSuccessData {
    #[serde(rename = "BattleID")]
    pub battle_id: i64,
    #[serde(default)]
    pub players: Vec<serde_json::Value>,
    #[serde(default)]
    pub bots: Vec<serde_json::Value>,
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LeaveBattleCommand {
    #[serde(rename = "BattleID", skip_serializing_if = "Option::is_none")]
    pub battle_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterCommand {
    pub name: String,
    pub password_hash: String,
    pub email: String,
    #[serde(rename = "UserID", default)]
    pub user_id: i64,
    #[serde(rename = "InstallID", default)]
    pub install_id: String,
    #[serde(default)]
    pub steam_auth_token: String,
    #[serde(default)]
    pub dlc: String,
}

// ── Server → Client responses ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WelcomeData {
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub user_count: i32,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoginResponseData {
    pub result_code: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub ban_reason: Option<String>,
    #[serde(default)]
    pub session_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserData {
    #[serde(rename = "AccountID", default)]
    pub account_id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub clan: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub level: i32,
    #[serde(default)]
    pub effective_elo: f64,
    #[serde(rename = "BattleID", default)]
    pub battle_id: Option<i64>,
    #[serde(default)]
    pub ban_mute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserDisconnectedData {
    pub name: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SayData {
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub place: i32,
    #[serde(default)]
    pub is_emote: bool,
    #[serde(default)]
    pub ring: Option<bool>,
}

/// Chat places
pub const PLACE_CHANNEL: i32 = 0;
pub const PLACE_BATTLE: i32 = 1;
pub const PLACE_BATTLE_PRIVATE: i32 = 2;
pub const PLACE_MESSAGE_BOX: i32 = 3;
pub const PLACE_USER: i32 = 4;
pub const PLACE_SERVER: i32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BattleAddedData {
    pub header: BattleHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BattleUpdateData {
    pub header: BattleHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BattleRemovedData {
    #[serde(rename = "BattleID")]
    pub battle_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BattleHeader {
    #[serde(rename = "BattleID", default)]
    pub battle_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub founder: String,
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub max_players: i32,
    #[serde(default)]
    pub player_count: i32,
    #[serde(default)]
    pub spectator_count: i32,
    #[serde(default)]
    pub is_running: bool,
    #[serde(default)]
    pub is_password_protected: bool,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JoinChannelResponseData {
    pub channel_name: String,
    pub success: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub channel: Option<ChannelData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChannelData {
    #[serde(default)]
    pub topic: Option<TopicData>,
    #[serde(default)]
    pub users: Vec<String>,
    #[serde(default)]
    pub is_deluge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TopicData {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub set_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChannelUserAddedData {
    pub channel_name: String,
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChannelUserRemovedData {
    pub channel_name: String,
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ConnectSpringData {
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: i32,
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub script_password: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub is_spectator: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterResponseData {
    pub result_code: i32,
    #[serde(default)]
    pub ban_reason: Option<String>,
}

pub const REGISTER_OK: i32 = 0;

/// Login result codes
pub const LOGIN_OK: i32 = 0;
pub const LOGIN_INVALID_NAME: i32 = 1;
pub const LOGIN_INVALID_PASSWORD: i32 = 2;
pub const LOGIN_BANNED: i32 = 4;

// ── Matchmaker messages ──

/// Server → Client: sent on login, lists available matchmaker queues.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MatchMakerSetupData {
    #[serde(default)]
    pub possible_queues: Vec<QueueInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QueueInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub maps: Vec<String>,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub max_party_size: i32,
}

/// Client → Server: request to join/leave matchmaker queues.
/// Send empty Queues list to leave all queues.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MatchMakerQueueRequestCommand {
    pub queues: Vec<String>,
}

/// Server → Client: matchmaker status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MatchMakerStatusData {
    #[serde(default)]
    pub joined_queues: Vec<String>,
    #[serde(default)]
    pub queue_counts: HashMap<String, i32>,
    #[serde(default)]
    pub current_elo_width: Option<i32>,
    #[serde(default)]
    pub joined_time: Option<String>,
    #[serde(default)]
    pub banned_seconds: Option<i32>,
    #[serde(default)]
    pub instant_start_queues: Vec<String>,
    #[serde(default)]
    pub ingame_counts: HashMap<String, i32>,
    #[serde(default)]
    pub user_count: i32,
    #[serde(default)]
    pub user_count_discord: i32,
}

/// Server → Client: match found, asking player to accept.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AreYouReadyData {
    #[serde(default)]
    pub minimum_win_chance: f64,
    #[serde(default)]
    pub quick_play: bool,
    #[serde(default)]
    pub seconds_remaining: i32,
}

/// Client → Server: accept/decline match.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AreYouReadyResponseCommand {
    pub ready: bool,
}

/// Server → Client: live status while waiting for all players.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AreYouReadyUpdateData {
    #[serde(default)]
    pub ready_accepted: bool,
    #[serde(default)]
    pub likely_to_play: bool,
    #[serde(default)]
    pub queue_ready_counts: HashMap<String, i32>,
    #[serde(default)]
    pub your_battle_size: Option<i32>,
    #[serde(default)]
    pub your_battle_ready: Option<i32>,
}

/// Server → Client: final match result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AreYouReadyResultData {
    #[serde(default)]
    pub is_battle_starting: bool,
    #[serde(default)]
    pub are_you_banned: bool,
}

/// Create MD5 password hash for login.
pub fn hash_password(password: &str) -> String {
    use base64::Engine;
    use md5::Digest;

    let mut hasher = md5::Md5::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_parsing() {
        let msg = LobbyMessage::from_line(r#"Say {"User":"test","Text":"hello","Place":0}"#).unwrap();
        assert_eq!(msg.command, "Say");
        let data: SayData = serde_json::from_value(msg.data).unwrap();
        assert_eq!(data.user, "test");
        assert_eq!(data.text, "hello");
    }

    #[test]
    fn test_message_no_data() {
        let msg = LobbyMessage::from_line("Ping").unwrap();
        assert_eq!(msg.command, "Ping");
        assert_eq!(msg.data, serde_json::json!({}));
    }

    #[test]
    fn test_wire_format() {
        let msg = LobbyMessage::new("Ping", serde_json::json!({}));
        assert_eq!(msg.to_wire(), "Ping {}\n");
    }

    #[test]
    fn test_password_hash() {
        let hash = hash_password("test");
        assert!(!hash.is_empty());
        // MD5 of "test" is 098f6bcd4621d373cade4e832627b4f6
        // base64 of those bytes
        assert_eq!(hash, "CY9rzUYh03PK3k6DJie09g==");
    }

    #[test]
    fn test_login_command_serialization() {
        let cmd = LoginCommand {
            name: "bot".into(),
            password_hash: "abc123==".into(),
            user_id: 0,
            install_id: 0,
            lobby_version: 0,
            steam_auth_token: String::new(),
            dlc: String::new(),
        };
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(json["Name"], "bot");
        assert_eq!(json["PasswordHash"], "abc123==");
    }
}
