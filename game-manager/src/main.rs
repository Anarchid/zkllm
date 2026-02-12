mod engine;
mod lobby;
mod mcpl_server;
mod sai_ipc;

use engine::EngineManager;
use lobby::*;
use mcpl_core::connection::IncomingMessage as McplIncoming;
use mcpl_core::methods::*;
use mcpl_core::types::*;
use sai_ipc::SaiIpcServer;

use std::path::PathBuf;
use tokio::net::TcpListener;

struct GameManager {
    mcpl: Option<mcpl_core::McplConnection>,
    lobby_conn: Option<LobbyConnection>,
    lobby_state: LobbyState,
    engines: EngineManager,
    sai: SaiIpcServer,
}

impl GameManager {
    fn new(engine_path: PathBuf, socket_dir: String) -> Self {
        Self {
            mcpl: None,
            lobby_conn: None,
            lobby_state: LobbyState::new(),
            engines: EngineManager::new(engine_path, socket_dir),
            sai: SaiIpcServer::new(),
        }
    }

    /// Handle an MCPL tool call from the AF client.
    async fn handle_tool_call(
        &mut self,
        name: &str,
        args: &serde_json::Value,
    ) -> serde_json::Value {
        match name {
            "lobby_connect" => self.tool_lobby_connect(args).await,
            "lobby_login" => self.tool_lobby_login(args).await,
            "lobby_register" => self.tool_lobby_register(args).await,
            "lobby_disconnect" => self.tool_lobby_disconnect().await,
            "lobby_say" => self.tool_lobby_say(args).await,
            "lobby_join_channel" => self.tool_lobby_join_channel(args).await,
            "lobby_leave_channel" => self.tool_lobby_leave_channel(args).await,
            "lobby_list_battles" => self.tool_lobby_list_battles().await,
            "lobby_list_users" => self.tool_lobby_list_users(args).await,
            "lobby_join_battle" => self.tool_lobby_join_battle(args).await,
            "lobby_leave_battle" => self.tool_lobby_leave_battle().await,
            _ => serde_json::json!({
                "content": [{"type": "text", "text": format!("Unknown tool: {}", name)}],
                "isError": true
            }),
        }
    }

    // ── MCPL channel methods ──

    async fn handle_channels_open(
        &mut self,
        params: &serde_json::Value,
    ) -> serde_json::Value {
        let map = params
            .get("address")
            .and_then(|a| a.get("map"))
            .and_then(|v| v.as_str())
            .unwrap_or("Chicken Defence 1.56");
        let game = params
            .get("address")
            .and_then(|a| a.get("game"))
            .and_then(|v| v.as_str())
            .unwrap_or("Zero-K v1.12.1.0");

        match self.engines.start_game(map, game).await {
            Ok(channel_id) => {
                // Set up SAI IPC listener for this channel
                let socket_path = self
                    .engines
                    .instances
                    .get(&channel_id)
                    .map(|i| i.config.socket_path.clone())
                    .unwrap_or_default();

                if let Err(e) = self.sai.listen_for(&channel_id, &socket_path) {
                    tracing::error!("Failed to set up SAI listener: {}", e);
                }

                // Send channels/changed notification
                self.send_channels_changed(
                    vec![ChannelDescriptor {
                        id: channel_id.clone(),
                        channel_type: "game".into(),
                        label: format!("Game on {}", map),
                        direction: ChannelDirection::Bidirectional,
                        address: None,
                        metadata: Some(serde_json::json!({
                            "map": map,
                            "game": game,
                            "status": "starting",
                        })),
                    }],
                    vec![],
                    vec![],
                )
                .await;

                serde_json::json!({
                    "channel": {
                        "id": channel_id,
                        "type": "game",
                        "label": format!("Game on {}", map),
                        "direction": "bidirectional",
                        "metadata": {
                            "map": map,
                            "game": game,
                            "status": "starting"
                        }
                    }
                })
            }
            Err(e) => serde_json::json!({
                "error": { "code": -32000, "message": e }
            }),
        }
    }

    async fn handle_channels_close(
        &mut self,
        params: &serde_json::Value,
    ) -> serde_json::Value {
        let channel_id = match params.get("channelId").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                return serde_json::json!({
                    "closed": false,
                    "error": "Missing channelId"
                })
            }
        };

        self.sai.close_channel(&channel_id);
        if let Err(e) = self.engines.stop_game(&channel_id).await {
            return serde_json::json!({
                "closed": false,
                "error": e
            });
        }

        // Notify channels/changed
        self.send_channels_changed(vec![], vec![channel_id], vec![])
            .await;

        serde_json::json!({ "closed": true })
    }

    async fn handle_channels_list(&self) -> serde_json::Value {
        let channels: Vec<serde_json::Value> = self
            .engines
            .instances
            .iter()
            .map(|(id, inst)| {
                let connected = self.sai.connections.contains_key(id);
                serde_json::json!({
                    "id": id,
                    "type": "game",
                    "label": format!("Game on {}", inst.config.map),
                    "direction": "bidirectional",
                    "metadata": {
                        "map": inst.config.map,
                        "game": inst.config.game,
                        "status": format!("{:?}", inst.status),
                        "saiConnected": connected,
                    }
                })
            })
            .collect();

        serde_json::json!({ "channels": channels })
    }

    async fn handle_channels_publish(
        &mut self,
        params: &serde_json::Value,
    ) -> serde_json::Value {
        let channel_id = match params.get("channelId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return serde_json::json!({
                    "delivered": false,
                    "error": "Missing channelId"
                })
            }
        };

        let content = params
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let cmd = match sai_ipc::parse_publish_command(content) {
            Ok(c) => c,
            Err(e) => {
                return serde_json::json!({
                    "delivered": false,
                    "error": e
                })
            }
        };

        match self.sai.send_to(channel_id, &cmd).await {
            Ok(()) => serde_json::json!({
                "delivered": true,
                "messageId": uuid::Uuid::new_v4().to_string()
            }),
            Err(e) => serde_json::json!({
                "delivered": false,
                "error": e
            }),
        }
    }

    async fn handle_state_rollback(
        &mut self,
        params: &serde_json::Value,
    ) -> serde_json::Value {
        let _feature_set = params
            .get("featureSet")
            .and_then(|v| v.as_str())
            .unwrap_or("game");
        let checkpoint = params
            .get("checkpoint")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // For now, rollback is a placeholder — full implementation
        // requires engine savestate support
        serde_json::json!({
            "success": false,
            "checkpoint": checkpoint,
            "reason": "Rollback not yet implemented — requires engine savestate support"
        })
    }

    // ── Notification helpers ──

    async fn send_channels_changed(
        &mut self,
        added: Vec<ChannelDescriptor>,
        removed: Vec<String>,
        updated: Vec<ChannelDescriptor>,
    ) {
        if let Some(mcpl) = &mut self.mcpl {
            let params = ChannelsChangedParams {
                added: if added.is_empty() {
                    None
                } else {
                    Some(added)
                },
                removed: if removed.is_empty() {
                    None
                } else {
                    Some(removed)
                },
                updated: if updated.is_empty() {
                    None
                } else {
                    Some(updated)
                },
            };
            let _ = mcpl
                .send_notification(
                    method::CHANNELS_CHANGED,
                    Some(serde_json::to_value(&params).unwrap()),
                )
                .await;
        }
    }

    /// Forward a SAI event as channels/incoming to the MCPL client.
    async fn forward_sai_event(
        &mut self,
        channel_id: &str,
        event: &sai_ipc::SaiEvent,
    ) {
        let mcpl = match &mut self.mcpl {
            Some(c) => c,
            None => return,
        };

        let content_text = sai_ipc::event_to_content(event);
        let msg_id = uuid::Uuid::new_v4().to_string();

        let params = ChannelsIncomingParams {
            messages: vec![mcpl_core::methods::IncomingMessage {
                channel_id: channel_id.to_string(),
                message_id: msg_id,
                thread_id: None,
                author: MessageAuthor {
                    id: "engine".into(),
                    name: "Game Engine".into(),
                },
                content: vec![ContentBlock::text(content_text)],
                timestamp: chrono::Utc::now().to_rfc3339(),
                metadata: None,
            }],
        };

        let _ = mcpl
            .send_request(
                method::CHANNELS_INCOMING,
                Some(serde_json::to_value(&params).unwrap()),
            )
            .await;
    }

    // ── Lobby tool implementations (unchanged) ──

    async fn tool_lobby_connect(&mut self, args: &serde_json::Value) -> serde_json::Value {
        let host = args
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or("zero-k.info");
        let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(8200) as u16;

        match LobbyConnection::connect(host, port).await {
            Ok(conn) => {
                self.lobby_conn = Some(conn);
                serde_json::json!({
                    "content": [{"type": "text", "text": format!("Connected to {}:{}", host, port)}]
                })
            }
            Err(e) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Connection failed: {}", e)}],
                "isError": true
            }),
        }
    }

    async fn tool_lobby_login(&mut self, args: &serde_json::Value) -> serde_json::Value {
        let username = match args.get("username").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing username"}],
                    "isError": true
                })
            }
        };
        let password = match args.get("password").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing password"}],
                    "isError": true
                })
            }
        };

        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected to lobby. Call lobby_connect first."}],
                    "isError": true
                })
            }
        };

        let cmd = LoginCommand {
            name: username.to_string(),
            password_hash: hash_password(password),
            user_id: 0,
            install_id: 0,
            lobby_version: 0,
            steam_auth_token: String::new(),
            dlc: String::new(),
        };

        if let Err(e) = conn.send_command("Login", &cmd).await {
            return serde_json::json!({
                "content": [{"type": "text", "text": format!("Failed to send login: {}", e)}],
                "isError": true
            });
        }

        serde_json::json!({
            "content": [{"type": "text", "text": format!("Login sent for user '{}'. Waiting for server response...", username)}]
        })
    }

    async fn tool_lobby_register(&mut self, args: &serde_json::Value) -> serde_json::Value {
        let username = match args.get("username").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing username"}],
                    "isError": true
                })
            }
        };
        let password = match args.get("password").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing password"}],
                    "isError": true
                })
            }
        };
        let email = match args.get("email").and_then(|v| v.as_str()) {
            Some(e) => e,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing email"}],
                    "isError": true
                })
            }
        };

        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected to lobby. Call lobby_connect first."}],
                    "isError": true
                })
            }
        };

        let cmd = RegisterCommand {
            name: username.to_string(),
            password_hash: hash_password(password),
            email: email.to_string(),
            user_id: 0,
            install_id: String::new(),
            steam_auth_token: String::new(),
            dlc: String::new(),
        };

        if let Err(e) = conn.send_command("Register", &cmd).await {
            return serde_json::json!({
                "content": [{"type": "text", "text": format!("Failed to send register: {}", e)}],
                "isError": true
            });
        }

        serde_json::json!({
            "content": [{"type": "text", "text": format!("Registration sent for user '{}' with email '{}'. Waiting for server response...", username, email)}]
        })
    }

    async fn tool_lobby_disconnect(&mut self) -> serde_json::Value {
        self.lobby_conn = None;
        self.lobby_state = LobbyState::new();
        serde_json::json!({
            "content": [{"type": "text", "text": "Disconnected from lobby"}]
        })
    }

    async fn tool_lobby_say(&mut self, args: &serde_json::Value) -> serde_json::Value {
        let target = match args.get("target").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing target"}],
                    "isError": true
                })
            }
        };
        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing text"}],
                    "isError": true
                })
            }
        };
        let place = args
            .get("place")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected"}],
                    "isError": true
                })
            }
        };

        let cmd = SayCommand {
            place,
            target: target.to_string(),
            text: text.to_string(),
            is_emote: false,
        };

        match conn.send_command("Say", &cmd).await {
            Ok(()) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Sent to {}: {}", target, text)}]
            }),
            Err(e) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Send failed: {}", e)}],
                "isError": true
            }),
        }
    }

    async fn tool_lobby_join_channel(
        &mut self,
        args: &serde_json::Value,
    ) -> serde_json::Value {
        let channel = match args.get("channel").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing channel"}],
                    "isError": true
                })
            }
        };

        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected"}],
                    "isError": true
                })
            }
        };

        let cmd = JoinChannelCommand {
            channel_name: channel.to_string(),
            password: String::new(),
        };

        match conn.send_command("JoinChannel", &cmd).await {
            Ok(()) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Join request sent for #{}", channel)}]
            }),
            Err(e) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Failed: {}", e)}],
                "isError": true
            }),
        }
    }

    async fn tool_lobby_leave_channel(
        &mut self,
        args: &serde_json::Value,
    ) -> serde_json::Value {
        let channel = match args.get("channel").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing channel"}],
                    "isError": true
                })
            }
        };

        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected"}],
                    "isError": true
                })
            }
        };

        let cmd = LeaveChannelCommand {
            channel_name: channel.to_string(),
        };

        match conn.send_command("LeaveChannel", &cmd).await {
            Ok(()) => {
                self.lobby_state.channels.remove(channel);
                serde_json::json!({
                    "content": [{"type": "text", "text": format!("Left #{}", channel)}]
                })
            }
            Err(e) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Failed: {}", e)}],
                "isError": true
            }),
        }
    }

    async fn tool_lobby_list_battles(&mut self) -> serde_json::Value {
        let battles: Vec<serde_json::Value> = self
            .lobby_state
            .battles
            .values()
            .map(|b| {
                serde_json::json!({
                    "id": b.battle_id,
                    "title": b.title,
                    "founder": b.founder,
                    "map": b.map,
                    "players": b.player_count,
                    "maxPlayers": b.max_players,
                    "spectators": b.spectator_count,
                    "running": b.is_running,
                    "passwordProtected": b.is_password_protected,
                    "mode": b.mode,
                })
            })
            .collect();

        serde_json::json!({
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&battles).unwrap()}]
        })
    }

    async fn tool_lobby_list_users(
        &mut self,
        args: &serde_json::Value,
    ) -> serde_json::Value {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        let users: Vec<serde_json::Value> = self
            .lobby_state
            .users
            .values()
            .take(limit)
            .map(|u| {
                serde_json::json!({
                    "name": u.name,
                    "level": u.level,
                    "elo": u.elo,
                    "clan": u.clan,
                    "country": u.country,
                    "isBot": u.is_bot,
                    "isAdmin": u.is_admin,
                    "battleId": u.battle_id,
                })
            })
            .collect();

        serde_json::json!({
            "content": [{"type": "text", "text": format!("{} users (showing {})\n{}", self.lobby_state.users.len(), users.len(), serde_json::to_string_pretty(&users).unwrap())}]
        })
    }

    async fn tool_lobby_join_battle(
        &mut self,
        args: &serde_json::Value,
    ) -> serde_json::Value {
        let battle_id = match args.get("battle_id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Missing battle_id"}],
                    "isError": true
                })
            }
        };
        let password = args
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected"}],
                    "isError": true
                })
            }
        };

        let cmd = JoinBattleCommand {
            battle_id,
            password: password.to_string(),
        };

        match conn.send_command("JoinBattle", &cmd).await {
            Ok(()) => {
                self.lobby_state.my_battle = Some(battle_id);
                serde_json::json!({
                    "content": [{"type": "text", "text": format!("Join request sent for battle {}", battle_id)}]
                })
            }
            Err(e) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Failed: {}", e)}],
                "isError": true
            }),
        }
    }

    async fn tool_lobby_leave_battle(&mut self) -> serde_json::Value {
        let conn = match &mut self.lobby_conn {
            Some(c) => c,
            None => {
                return serde_json::json!({
                    "content": [{"type": "text", "text": "Not connected"}],
                    "isError": true
                })
            }
        };

        let cmd = LeaveBattleCommand { battle_id: None };

        match conn.send_command("LeaveBattle", &cmd).await {
            Ok(()) => {
                self.lobby_state.my_battle = None;
                serde_json::json!({
                    "content": [{"type": "text", "text": "Left battle"}]
                })
            }
            Err(e) => serde_json::json!({
                "content": [{"type": "text", "text": format!("Failed: {}", e)}],
                "isError": true
            }),
        }
    }

    /// Convert a lobby event to an MCPL push event and send it.
    async fn push_lobby_event(
        &mut self,
        event: &LobbyEvent,
    ) -> Result<(), mcpl_core::connection::ConnectionError> {
        let mcpl = match &mut self.mcpl {
            Some(c) => c,
            None => return Ok(()),
        };

        let (event_id, content_text) = match event {
            LobbyEvent::Connected { engine, game } => (
                "lobby.connected".to_string(),
                format!("Connected to lobby. Engine: {}, Game: {}", engine, game),
            ),
            LobbyEvent::Disconnected { reason } => (
                "lobby.disconnected".to_string(),
                format!("Disconnected: {}", reason),
            ),
            LobbyEvent::LoggedIn { username } => (
                "lobby.logged_in".to_string(),
                format!("Logged in as {}", username),
            ),
            LobbyEvent::LoginFailed { code, message } => (
                "lobby.login_failed".to_string(),
                format!("Login failed (code {}): {}", code, message),
            ),
            LobbyEvent::RegisterSuccess => (
                "lobby.register_success".to_string(),
                "Account registration successful".to_string(),
            ),
            LobbyEvent::RegisterFailed { code, reason } => (
                "lobby.register_failed".to_string(),
                format!("Registration failed (code {}): {}", code, reason),
            ),
            LobbyEvent::ChatMessage {
                user,
                text,
                target,
                place,
                ..
            } => {
                let place_name = match *place {
                    0 => "channel",
                    1 => "battle",
                    4 => "dm",
                    _ => "other",
                };
                (
                    "lobby.chat".to_string(),
                    format!("[{}] [{}] {}: {}", place_name, target, user, text),
                )
            }
            LobbyEvent::BattleOpened(b) => (
                "lobby.battle_opened".to_string(),
                format!(
                    "Battle opened: {} by {} on {} ({}/{})",
                    b.title, b.founder, b.map, b.player_count, b.max_players
                ),
            ),
            LobbyEvent::BattleClosed { battle_id } => (
                "lobby.battle_closed".to_string(),
                format!("Battle {} closed", battle_id),
            ),
            LobbyEvent::ChannelJoined {
                channel,
                users,
                topic,
            } => (
                "lobby.channel_joined".to_string(),
                format!(
                    "Joined #{} ({} users). Topic: {}",
                    channel,
                    users.len(),
                    topic.as_deref().unwrap_or("(none)")
                ),
            ),
            // Skip high-frequency events
            LobbyEvent::UserJoined(_)
            | LobbyEvent::UserLeft { .. }
            | LobbyEvent::BattleUpdated(_)
            | LobbyEvent::ChannelUserJoined { .. }
            | LobbyEvent::ChannelUserLeft { .. }
            | LobbyEvent::ConnectSpring(_) => {
                return Ok(());
            }
        };

        let params = PushEventParams {
            feature_set: "lobby".into(),
            event_id: format!("{}_{}", event_id, uuid::Uuid::new_v4()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            origin: Some(serde_json::json!({"source": "zk-lobby"})),
            payload: PushEventPayload {
                content: vec![ContentBlock::text(content_text)],
            },
        };

        mcpl.send_request(
            method::PUSH_EVENT,
            Some(serde_json::to_value(&params).unwrap()),
        )
        .await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing always goes to stderr — safe for stdio mode
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "game_manager=info,mcpl_core=debug".parse().unwrap()),
        )
        .init();

    let use_stdio = std::env::args().any(|a| a == "--stdio");

    let engine_path = std::env::var("ENGINE_PATH")
        .unwrap_or_else(|_| "/usr/local/bin/spring-dedicated".into());
    let socket_dir = std::env::var("SOCKET_DIR").unwrap_or_else(|_| "/tmp".into());

    let mcpl_conn = if use_stdio {
        mcpl_server::accept_mcpl_stdio().await?
    } else {
        let mcpl_port: u16 = std::env::var("MCPL_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(9800);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", mcpl_port)).await?;
        tracing::info!("GameManager MCPL server listening on port {}", mcpl_port);

        mcpl_server::accept_mcpl_client(&listener).await?
    };
    tracing::info!("MCPL client connected and initialized");

    let mut gm = GameManager::new(PathBuf::from(engine_path), socket_dir);
    gm.mcpl = Some(mcpl_conn);

    // Engine check interval
    let mut engine_check = tokio::time::interval(tokio::time::Duration::from_secs(2));

    // Main event loop
    loop {
        let lobby_msg = async {
            if let Some(conn) = &mut gm.lobby_conn {
                conn.recv().await
            } else {
                std::future::pending().await
            }
        };

        let mcpl_msg = async {
            if let Some(conn) = &mut gm.mcpl {
                conn.next_message().await
            } else {
                std::future::pending().await
            }
        };

        tokio::select! {
            result = lobby_msg => {
                match result {
                    Ok(msg) => {
                        if msg.command == "Ping" {
                            if let Some(conn) = &mut gm.lobby_conn {
                                let pong = LobbyMessage::new("Ping", serde_json::json!({}));
                                if let Err(e) = conn.send(&pong).await {
                                    tracing::error!("Failed to send ping response: {}", e);
                                }
                            }
                            continue;
                        }

                        let events = gm.lobby_state.handle_message(&msg);
                        for event in &events {
                            if let Err(e) = gm.push_lobby_event(event).await {
                                tracing::error!("Failed to push lobby event: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Lobby connection error: {}", e);
                        gm.lobby_conn = None;
                        gm.lobby_state.connected = false;
                        gm.lobby_state.logged_in = false;
                        let event = LobbyEvent::Disconnected { reason: e.to_string() };
                        let _ = gm.push_lobby_event(&event).await;
                    }
                }
            }

            result = mcpl_msg => {
                match result {
                    Ok(msg) => {
                        match msg {
                            McplIncoming::Request(req) => {
                                let result = match req.method.as_str() {
                                    "tools/list" => {
                                        mcpl_server::lobby_tools()
                                    }
                                    "tools/call" => {
                                        let params = req.params.unwrap_or_default();
                                        let tool_name = params.get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        let tool_args = params.get("arguments")
                                            .cloned()
                                            .unwrap_or(serde_json::json!({}));
                                        gm.handle_tool_call(tool_name, &tool_args).await
                                    }
                                    "channels/open" => {
                                        let params = req.params.unwrap_or_default();
                                        gm.handle_channels_open(&params).await
                                    }
                                    "channels/close" => {
                                        let params = req.params.unwrap_or_default();
                                        gm.handle_channels_close(&params).await
                                    }
                                    "channels/list" => {
                                        gm.handle_channels_list().await
                                    }
                                    "channels/publish" => {
                                        let params = req.params.unwrap_or_default();
                                        gm.handle_channels_publish(&params).await
                                    }
                                    "state/rollback" => {
                                        let params = req.params.unwrap_or_default();
                                        gm.handle_state_rollback(&params).await
                                    }
                                    _ => {
                                        tracing::warn!("Unknown MCPL method: {}", req.method);
                                        serde_json::json!({
                                            "error": { "code": -32601, "message": format!("Method not found: {}", req.method) }
                                        })
                                    }
                                };

                                if let Some(mcpl) = &mut gm.mcpl {
                                    if let Err(e) = mcpl.send_response(req.id, result).await {
                                        tracing::error!("Failed to send response: {}", e);
                                    }
                                }
                            }
                            McplIncoming::Notification(notif) => {
                                match notif.method.as_str() {
                                    "featureSets/update" => {
                                        tracing::info!("Feature sets update: {:?}", notif.params);
                                    }
                                    _ => {
                                        tracing::trace!("Unhandled notification: {}", notif.method);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("MCPL client disconnected: {}", e);
                        break;
                    }
                }
            }

            _ = engine_check.tick() => {
                // Check for SAI connections
                let newly_connected = gm.sai.accept_pending();
                for channel_id in &newly_connected {
                    tracing::info!("SAI connected for channel {}", channel_id);
                    if let Some(inst) = gm.engines.instances.get_mut(channel_id) {
                        inst.status = engine::GameStatus::Running;
                    }
                    gm.send_channels_changed(
                        vec![],
                        vec![],
                        vec![ChannelDescriptor {
                            id: channel_id.clone(),
                            channel_type: "game".into(),
                            label: "Game".into(),
                            direction: ChannelDirection::Bidirectional,
                            address: None,
                            metadata: Some(serde_json::json!({"status": "running", "saiConnected": true})),
                        }],
                    ).await;
                }

                // Check for engine crashes
                let changed = gm.engines.check_all().await;
                for (channel_id, status) in &changed {
                    tracing::warn!("Engine {} status changed: {:?}", channel_id, status);
                    gm.sai.close_channel(channel_id);
                    gm.send_channels_changed(
                        vec![],
                        vec![channel_id.clone()],
                        vec![],
                    ).await;
                }

                // Read events from connected SAIs
                let channel_ids: Vec<String> = gm.sai.connections.keys().cloned().collect();
                for channel_id in channel_ids {
                    // Collect events from this SAI
                    let mut events = Vec::new();
                    if let Some(conn) = gm.sai.connections.get_mut(&channel_id) {
                        // Non-blocking poll: try to read available events
                        // (next_event is async but the socket is set up for line-buffered reads)
                        loop {
                            // Use tokio::time::timeout for a quick check
                            match tokio::time::timeout(
                                tokio::time::Duration::from_millis(1),
                                conn.next_event(),
                            ).await {
                                Ok(Some(event)) => events.push(event),
                                Ok(None) => {
                                    // EOF — SAI disconnected
                                    tracing::warn!("SAI disconnected for {}", channel_id);
                                    break;
                                }
                                Err(_) => break, // timeout — no more events
                            }
                        }
                    }

                    // Forward events
                    for event in &events {
                        gm.forward_sai_event(&channel_id, event).await;
                    }
                }
            }
        }
    }

    tracing::info!("GameManager shutting down");
    Ok(())
}
