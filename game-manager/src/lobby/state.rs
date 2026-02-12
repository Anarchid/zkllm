use std::collections::HashMap;

use super::protocol::*;

/// Tracked lobby state, updated as messages arrive.
#[derive(Debug, Default)]
pub struct LobbyState {
    pub connected: bool,
    pub logged_in: bool,
    pub my_username: Option<String>,
    pub server_engine: String,
    pub server_game: String,
    pub user_count: i32,
    pub users: HashMap<String, UserInfo>,
    pub battles: HashMap<i64, BattleInfo>,
    pub channels: HashMap<String, ChannelInfo>,
    pub my_battle: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub account_id: i64,
    pub name: String,
    pub display_name: String,
    pub clan: String,
    pub country: String,
    pub is_bot: bool,
    pub is_admin: bool,
    pub level: i32,
    pub elo: f64,
    pub battle_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct BattleInfo {
    pub battle_id: i64,
    pub title: String,
    pub founder: String,
    pub map: String,
    pub game: String,
    pub engine: String,
    pub max_players: i32,
    pub player_count: i32,
    pub spectator_count: i32,
    pub is_running: bool,
    pub is_password_protected: bool,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    pub name: String,
    pub topic: Option<String>,
    pub users: Vec<String>,
}

/// Event emitted when lobby state changes, for forwarding as MCPL push events.
#[derive(Debug, Clone)]
pub enum LobbyEvent {
    Connected { engine: String, game: String },
    Disconnected { reason: String },
    LoggedIn { username: String },
    LoginFailed { code: i32, message: String },
    RegisterSuccess,
    RegisterFailed { code: i32, reason: String },
    UserJoined(UserInfo),
    UserLeft { name: String, reason: String },
    ChatMessage { user: String, text: String, target: String, place: i32, is_emote: bool, time: String },
    BattleOpened(BattleInfo),
    BattleUpdated(BattleInfo),
    BattleClosed { battle_id: i64 },
    ChannelJoined { channel: String, users: Vec<String>, topic: Option<String> },
    ChannelUserJoined { channel: String, user: String },
    ChannelUserLeft { channel: String, user: String },
    ConnectSpring(ConnectSpringData),
}

impl LobbyState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a lobby message and update state. Returns events to forward.
    pub fn handle_message(&mut self, msg: &LobbyMessage) -> Vec<LobbyEvent> {
        let mut events = Vec::new();

        match msg.command.as_str() {
            "Welcome" => {
                if let Ok(data) = serde_json::from_value::<WelcomeData>(msg.data.clone()) {
                    self.connected = true;
                    self.server_engine = data.engine.clone();
                    self.server_game = data.game.clone();
                    self.user_count = data.user_count;
                    events.push(LobbyEvent::Connected {
                        engine: data.engine,
                        game: data.game,
                    });
                }
            }
            "LoginResponse" => {
                if let Ok(data) = serde_json::from_value::<LoginResponseData>(msg.data.clone()) {
                    if data.result_code == LOGIN_OK {
                        self.logged_in = true;
                        self.my_username = Some(data.name.clone());
                        events.push(LobbyEvent::LoggedIn { username: data.name });
                    } else {
                        events.push(LobbyEvent::LoginFailed {
                            code: data.result_code,
                            message: data.message,
                        });
                    }
                }
            }
            "RegisterResponse" => {
                if let Ok(data) = serde_json::from_value::<RegisterResponseData>(msg.data.clone()) {
                    if data.result_code == REGISTER_OK {
                        events.push(LobbyEvent::RegisterSuccess);
                    } else {
                        events.push(LobbyEvent::RegisterFailed {
                            code: data.result_code,
                            reason: data.ban_reason.unwrap_or_default(),
                        });
                    }
                }
            }
            "User" => {
                if let Ok(data) = serde_json::from_value::<UserData>(msg.data.clone()) {
                    let info = UserInfo {
                        account_id: data.account_id,
                        name: data.name.clone(),
                        display_name: data.display_name,
                        clan: data.clan,
                        country: data.country,
                        is_bot: data.is_bot,
                        is_admin: data.is_admin,
                        level: data.level,
                        elo: data.effective_elo,
                        battle_id: data.battle_id,
                    };
                    let is_new = !self.users.contains_key(&data.name);
                    self.users.insert(data.name, info.clone());
                    if is_new {
                        events.push(LobbyEvent::UserJoined(info));
                    }
                }
            }
            "UserDisconnected" => {
                if let Ok(data) = serde_json::from_value::<UserDisconnectedData>(msg.data.clone()) {
                    self.users.remove(&data.name);
                    events.push(LobbyEvent::UserLeft {
                        name: data.name,
                        reason: data.reason,
                    });
                }
            }
            "Say" => {
                if let Ok(data) = serde_json::from_value::<SayData>(msg.data.clone()) {
                    events.push(LobbyEvent::ChatMessage {
                        user: data.user,
                        text: data.text,
                        target: data.target,
                        place: data.place,
                        is_emote: data.is_emote,
                        time: data.time,
                    });
                }
            }
            "BattleAdded" => {
                if let Ok(data) = serde_json::from_value::<BattleAddedData>(msg.data.clone()) {
                    let info = battle_info_from_header(&data.header);
                    self.battles.insert(info.battle_id, info.clone());
                    events.push(LobbyEvent::BattleOpened(info));
                }
            }
            "BattleUpdate" => {
                if let Ok(data) = serde_json::from_value::<BattleUpdateData>(msg.data.clone()) {
                    let info = battle_info_from_header(&data.header);
                    self.battles.insert(info.battle_id, info.clone());
                    events.push(LobbyEvent::BattleUpdated(info));
                }
            }
            "BattleRemoved" => {
                if let Ok(data) = serde_json::from_value::<BattleRemovedData>(msg.data.clone()) {
                    self.battles.remove(&data.battle_id);
                    events.push(LobbyEvent::BattleClosed {
                        battle_id: data.battle_id,
                    });
                }
            }
            "JoinChannelResponse" => {
                if let Ok(data) = serde_json::from_value::<JoinChannelResponseData>(msg.data.clone()) {
                    if data.success {
                        let channel_data = data.channel.unwrap_or_default();
                        let info = ChannelInfo {
                            name: data.channel_name.clone(),
                            topic: channel_data.topic.map(|t| t.text),
                            users: channel_data.users,
                        };
                        let users = info.users.clone();
                        let topic = info.topic.clone();
                        self.channels.insert(data.channel_name.clone(), info);
                        events.push(LobbyEvent::ChannelJoined {
                            channel: data.channel_name,
                            users,
                            topic,
                        });
                    }
                }
            }
            "ChannelUserAdded" => {
                if let Ok(data) = serde_json::from_value::<ChannelUserAddedData>(msg.data.clone()) {
                    if let Some(channel) = self.channels.get_mut(&data.channel_name) {
                        if !channel.users.contains(&data.user_name) {
                            channel.users.push(data.user_name.clone());
                        }
                    }
                    events.push(LobbyEvent::ChannelUserJoined {
                        channel: data.channel_name,
                        user: data.user_name,
                    });
                }
            }
            "ChannelUserRemoved" => {
                if let Ok(data) = serde_json::from_value::<ChannelUserRemovedData>(msg.data.clone()) {
                    if let Some(channel) = self.channels.get_mut(&data.channel_name) {
                        channel.users.retain(|u| u != &data.user_name);
                    }
                    events.push(LobbyEvent::ChannelUserLeft {
                        channel: data.channel_name,
                        user: data.user_name,
                    });
                }
            }
            "ConnectSpring" => {
                if let Ok(data) = serde_json::from_value::<ConnectSpringData>(msg.data.clone()) {
                    events.push(LobbyEvent::ConnectSpring(data));
                }
            }
            "Ping" => {
                // Handled by caller (respond with Ping)
            }
            _ => {
                tracing::trace!("Unhandled lobby command: {}", msg.command);
            }
        }

        events
    }
}

fn battle_info_from_header(h: &BattleHeader) -> BattleInfo {
    BattleInfo {
        battle_id: h.battle_id,
        title: h.title.clone(),
        founder: h.founder.clone(),
        map: h.map.clone(),
        game: h.game.clone(),
        engine: h.engine.clone(),
        max_players: h.max_players,
        player_count: h.player_count,
        spectator_count: h.spectator_count,
        is_running: h.is_running,
        is_password_protected: h.is_password_protected,
        mode: h.mode.clone(),
    }
}

impl Default for ChannelData {
    fn default() -> Self {
        Self {
            topic: None,
            users: Vec::new(),
            is_deluge: false,
        }
    }
}
