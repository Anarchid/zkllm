//! Engine process management — launching and monitoring Recoil/Spring game instances.

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::{Child, Command};

#[derive(Debug, Clone, PartialEq)]
pub enum GameStatus {
    Starting,
    Running,
    Stopped,
    Crashed(String),
}

pub struct EngineInstance {
    pub channel_id: String,
    pub process: Option<Child>,
    pub status: GameStatus,
    pub config: GameConfig,
    pub checkpoints: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub map: String,
    pub game: String,
    pub engine_path: PathBuf,
    pub ai_name: String,
    pub socket_path: String,
}

impl EngineInstance {
    pub fn new(channel_id: String, config: GameConfig) -> Self {
        Self {
            channel_id,
            process: None,
            status: GameStatus::Starting,
            config,
            checkpoints: Vec::new(),
        }
    }

    /// Launch the engine process with the SAI bridge configured.
    pub async fn start(&mut self) -> Result<(), String> {
        // Build script.txt for the engine
        let script = self.generate_script();
        let script_path = format!("/tmp/gm_script_{}.txt", self.channel_id);
        tokio::fs::write(&script_path, &script)
            .await
            .map_err(|e| format!("Failed to write script.txt: {}", e))?;

        let child = Command::new(&self.config.engine_path)
            .arg("--write-dir")
            .arg("/tmp")
            .arg(&script_path)
            .env("SAI_SOCKET_PATH", &self.config.socket_path)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn engine: {}", e))?;

        self.process = Some(child);
        self.status = GameStatus::Starting;
        Ok(())
    }

    /// Stop the engine process.
    pub async fn stop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill().await;
        }
        self.process = None;
        self.status = GameStatus::Stopped;
    }

    /// Check if the engine process is still running.
    pub async fn check_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        self.status = GameStatus::Stopped;
                    } else {
                        self.status =
                            GameStatus::Crashed(format!("Exit code: {:?}", status.code()));
                    }
                    self.process = None;
                    false
                }
                Ok(None) => true, // still running
                Err(e) => {
                    self.status = GameStatus::Crashed(e.to_string());
                    self.process = None;
                    false
                }
            }
        } else {
            false
        }
    }

    fn generate_script(&self) -> String {
        // Minimal script.txt for a skirmish game with the SAI bridge AI
        format!(
            r#"[GAME]
{{
    Mapname={map};
    Gametype={game};
    IsHost=1;
    MyPlayerNum=0;
    MyPlayerName=GameManager;
    StartPosType=2;
    NumPlayers=0;
    NumUsers=2;
    NumTeams=2;
    NumAllyTeams=2;

    [TEAM0]
    {{
        TeamLeader=0;
        AllyTeam=0;
    }}

    [TEAM1]
    {{
        TeamLeader=0;
        AllyTeam=1;
    }}

    [AI0]
    {{
        Name={ai_name};
        ShortName={ai_name};
        Team=0;
        IsFromDemo=0;
        Host=0;
        [Options]
        {{
            socket_path={socket_path};
        }}
    }}

    [AI1]
    {{
        Name=NullAI;
        ShortName=NullAI;
        Team=1;
        IsFromDemo=0;
        Host=0;
    }}

    [ALLYTEAM0]
    {{
        NumAllies=0;
    }}

    [ALLYTEAM1]
    {{
        NumAllies=0;
    }}
}}"#,
            map = self.config.map,
            game = self.config.game,
            ai_name = self.config.ai_name,
            socket_path = self.config.socket_path,
        )
    }
}

/// Manages all active engine instances.
pub struct EngineManager {
    pub instances: HashMap<String, EngineInstance>,
    next_id: u32,
    pub engine_path: PathBuf,
    pub socket_dir: String,
}

impl EngineManager {
    pub fn new(engine_path: PathBuf, socket_dir: String) -> Self {
        Self {
            instances: HashMap::new(),
            next_id: 1,
            engine_path,
            socket_dir,
        }
    }

    /// Create and start a new game instance.
    /// Returns the channel ID.
    pub async fn start_game(&mut self, map: &str, game: &str) -> Result<String, String> {
        let id = self.next_id;
        self.next_id += 1;
        let channel_id = format!("game:live-{}", id);
        let socket_path = format!("{}/sai_{}.sock", self.socket_dir, id);

        let config = GameConfig {
            map: map.to_string(),
            game: game.to_string(),
            engine_path: self.engine_path.clone(),
            ai_name: "AgentBridge".to_string(),
            socket_path,
        };

        let mut instance = EngineInstance::new(channel_id.clone(), config);
        instance.start().await?;
        self.instances.insert(channel_id.clone(), instance);
        Ok(channel_id)
    }

    /// Stop a game instance.
    pub async fn stop_game(&mut self, channel_id: &str) -> Result<(), String> {
        let instance = self
            .instances
            .get_mut(channel_id)
            .ok_or_else(|| format!("No game instance: {}", channel_id))?;
        instance.stop().await;
        self.instances.remove(channel_id);
        Ok(())
    }

    /// Check all instances for crashes/exits.
    /// Returns IDs of instances that stopped.
    pub async fn check_all(&mut self) -> Vec<(String, GameStatus)> {
        let mut changed = Vec::new();
        for (id, instance) in &mut self.instances {
            let was_alive = instance.process.is_some();
            let alive = instance.check_alive().await;
            if was_alive && !alive {
                changed.push((id.clone(), instance.status.clone()));
            }
        }
        changed
    }
}
