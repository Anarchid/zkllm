//! Engine process management — launching and monitoring Recoil/Spring game instances.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};

use crate::lobby::protocol::ConnectSpringData;

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
    pub engine_dir: PathBuf,
    pub write_dir: PathBuf,
    pub headless: bool,
    pub socket_path: String,
    // Agent AI config
    pub agent_ai: String,
    pub agent_team: i32,
    // Opponent AI config (local games only)
    pub opponent_ai: Option<String>,
    pub opponent_team: i32,
    // Multiplayer client config
    pub multiplayer: Option<MultiplayerConfig>,
}

#[derive(Debug, Clone)]
pub struct MultiplayerConfig {
    pub host_ip: String,
    pub host_port: i32,
    pub player_name: String,
    pub script_password: String,
}

/// Resolve the engine binary path from an engine directory.
pub fn resolve_engine_binary(engine_dir: &Path, headless: bool) -> PathBuf {
    if headless {
        engine_dir.join("spring-headless")
    } else {
        engine_dir.join("spring")
    }
}

/// Find the engine directory, either by explicit version or by picking the latest.
pub fn find_engine_dir(spring_home: &Path, version: Option<&str>) -> anyhow::Result<PathBuf> {
    let engines_base = spring_home.join("engine/linux64");

    if let Some(ver) = version {
        // Try exact match first
        let exact = engines_base.join(ver);
        if exact.exists() {
            return Ok(exact);
        }
        // Try with engine_linux64_ prefix
        let prefixed = engines_base.join(format!("engine_linux64_{}", ver));
        if prefixed.exists() {
            return Ok(prefixed);
        }
        anyhow::bail!(
            "Engine version '{}' not found in {}",
            ver,
            engines_base.display()
        );
    }

    // Find latest — sort directory entries by name, take last
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&engines_base)
        .map_err(|e| anyhow::anyhow!("Cannot read engine dir {}: {}", engines_base.display(), e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    entries.sort();

    let latest = entries
        .last()
        .ok_or_else(|| anyhow::anyhow!("No engine versions found in {}", engines_base.display()))?;

    // Verify spring-headless exists
    let headless_bin = latest.join("spring-headless");
    if !headless_bin.exists() {
        anyhow::bail!(
            "spring-headless not found in {}",
            latest.display()
        );
    }

    Ok(latest.clone())
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

    /// Launch the engine process.
    pub async fn start(&mut self) -> Result<(), String> {
        let script = if self.config.multiplayer.is_some() {
            self.generate_multiplayer_script()
        } else {
            self.generate_local_script()
        };

        let script_path = self
            .config
            .write_dir
            .join(format!("temp/gm_script_{}.txt", self.channel_id.replace(':', "_")));
        tokio::fs::write(&script_path, &script)
            .await
            .map_err(|e| format!("Failed to write script.txt: {}", e))?;

        let engine_bin = resolve_engine_binary(&self.config.engine_dir, self.config.headless);
        tracing::info!(
            "Launching engine: {} --write-dir {} {}",
            engine_bin.display(),
            self.config.write_dir.display(),
            script_path.display()
        );

        let child = Command::new(&engine_bin)
            .arg("--write-dir")
            .arg(&self.config.write_dir)
            .arg(&script_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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
                Ok(None) => true,
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

    /// Generate a local scrimmage script: spectator GameManager + AgentBridge vs opponent AI.
    fn generate_local_script(&self) -> String {
        let opponent = self
            .config
            .opponent_ai
            .as_deref()
            .unwrap_or("CircuitAINovice");

        format!(
            r#"[GAME]
{{
    Mapname={map};
    Gametype={game};
    IsHost=1;
    MyPlayerNum=0;
    MyPlayerName=GameManager;
    StartPosType=2;
    NumPlayers=1;
    NumUsers=3;
    NumTeams=2;
    NumAllyTeams=2;

    [PLAYER0]
    {{
        Name=GameManager;
        Team=-1;
        Spectator=1;
    }}

    [AI0]
    {{
        Name=AgentBridge;
        ShortName={agent_ai};
        Version=0.1;
        Team={agent_team};
        Host=0;
        [Options]
        {{
            socket_path={socket_path};
        }}
    }}

    [AI1]
    {{
        Name={opponent};
        ShortName={opponent};
        Team={opponent_team};
        Host=0;
    }}

    [TEAM0] {{ TeamLeader=0; AllyTeam=0; }}
    [TEAM1] {{ TeamLeader=0; AllyTeam=1; }}
    [ALLYTEAM0] {{ NumAllies=0; }}
    [ALLYTEAM1] {{ NumAllies=0; }}
}}"#,
            map = self.config.map,
            game = self.config.game,
            agent_ai = self.config.agent_ai,
            agent_team = self.config.agent_team,
            opponent = opponent,
            opponent_team = self.config.opponent_team,
            socket_path = self.config.socket_path,
        )
    }

    /// Generate a multiplayer client script — connects to a remote game server.
    fn generate_multiplayer_script(&self) -> String {
        let mp = self.config.multiplayer.as_ref().unwrap();
        format!(
            r#"[GAME]
{{
    HostIP={ip};
    HostPort={port};
    MyPlayerName={player};
    MyPasswd={password};
    IsHost=0;
}}"#,
            ip = mp.host_ip,
            port = mp.host_port,
            player = mp.player_name,
            password = mp.script_password,
        )
    }
}

/// Manages all active engine instances.
pub struct EngineManager {
    pub instances: HashMap<String, EngineInstance>,
    next_id: u32,
    pub engine_dir: PathBuf,
    pub write_dir: PathBuf,
    pub socket_dir: String,
}

impl EngineManager {
    pub fn new(engine_dir: PathBuf, write_dir: PathBuf, socket_dir: String) -> Self {
        Self {
            instances: HashMap::new(),
            next_id: 1,
            engine_dir,
            write_dir,
            socket_dir,
        }
    }

    /// Start a local scrimmage game: AgentBridge vs opponent AI.
    pub async fn start_local_game(
        &mut self,
        map: &str,
        game: &str,
        opponent: Option<&str>,
        headless: bool,
    ) -> Result<String, String> {
        let id = self.next_id;
        self.next_id += 1;
        let channel_id = format!("game:local-{}", id);
        let socket_path = format!("{}/sai_{}.sock", self.socket_dir, id);

        let config = GameConfig {
            map: map.to_string(),
            game: game.to_string(),
            engine_dir: self.engine_dir.clone(),
            write_dir: self.write_dir.clone(),
            headless,
            socket_path,
            agent_ai: "AgentBridge".to_string(),
            agent_team: 0,
            opponent_ai: Some(
                opponent.unwrap_or("CircuitAINovice").to_string(),
            ),
            opponent_team: 1,
            multiplayer: None,
        };

        let mut instance = EngineInstance::new(channel_id.clone(), config);
        instance.start().await?;
        self.instances.insert(channel_id.clone(), instance);
        Ok(channel_id)
    }

    /// Start a multiplayer game from a ConnectSpring lobby event.
    pub async fn start_multiplayer_game(
        &mut self,
        data: &ConnectSpringData,
        player_name: &str,
    ) -> Result<String, String> {
        let id = self.next_id;
        self.next_id += 1;
        let channel_id = format!("game:mp-{}", id);
        let socket_path = format!("{}/sai_mp_{}.sock", self.socket_dir, id);

        let config = GameConfig {
            map: data.map.clone(),
            game: data.game.clone(),
            engine_dir: self.engine_dir.clone(),
            write_dir: self.write_dir.clone(),
            headless: true,
            socket_path,
            agent_ai: "AgentBridge".to_string(),
            agent_team: 0,
            opponent_ai: None,
            opponent_team: 1,
            multiplayer: Some(MultiplayerConfig {
                host_ip: data.ip.clone(),
                host_port: data.port,
                player_name: player_name.to_string(),
                script_password: data.script_password.clone(),
            }),
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
