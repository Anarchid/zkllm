//! Agent write-dir initialization.
//!
//! Creates an isolated Spring write directory for the agent, symlinking shared
//! content from the human player's `~/.spring/` to avoid duplication.
//! On first boot this sets up the directory structure, installs the SAI bridge
//! .so, and generates default configs.

use std::path::{Path, PathBuf};

/// Directories to symlink from spring_home into the agent write-dir.
/// Note: `cache` is intentionally excluded — ArchiveCache20.lua stores absolute
/// paths, so sharing it across different write-dirs causes a full rescan anyway,
/// and writing back would clobber the human player's cache.
const SHARED_DIRS: &[&str] = &[
    "pool",
    "packages",
    "maps",
    "games",
    "engine",
    "rapid",
];

/// Initialize the agent write directory.
///
/// - Creates the directory structure
/// - Symlinks shared content from `spring_home`
/// - Installs the SAI bridge .so + metadata
/// - Installs the startup widget
/// - Generates default springsettings.cfg
pub fn init_write_dir(
    base: &Path,
    spring_home: &Path,
    sai_bridge_lib: &Path,
    sai_bridge_data: &Path,
    widget_source: &Path,
    agent_name: &str,
) -> anyhow::Result<()> {
    tracing::info!("Initializing agent write-dir: {}", base.display());

    // 1. Create base dir
    std::fs::create_dir_all(base)?;

    // 2. Create subdirs
    let subdirs = [
        "AI/Skirmish/AgentBridge/0.1",
        "AI/Interfaces",
        "LuaUI/Widgets",
        "LuaUI/Config",
        "demos",
        "temp",
    ];
    for sub in &subdirs {
        let p = base.join(sub);
        if !p.exists() {
            std::fs::create_dir_all(&p)?;
            tracing::info!("  Created {}", sub);
        }
    }

    // 3. Symlink shared content
    for dir_name in SHARED_DIRS {
        let target = spring_home.join(dir_name);
        let link = base.join(dir_name);

        if link.exists() || link.symlink_metadata().is_ok() {
            // Already exists (file, dir, or symlink) — check if correct
            if let Ok(existing_target) = std::fs::read_link(&link) {
                if existing_target == target {
                    continue; // correct symlink
                }
                tracing::warn!(
                    "  Symlink {} points to {} (expected {}), skipping",
                    dir_name,
                    existing_target.display(),
                    target.display()
                );
            }
            continue;
        }

        if target.exists() {
            std::os::unix::fs::symlink(&target, &link)?;
            tracing::info!("  Symlinked {} -> {}", dir_name, target.display());
        } else {
            tracing::warn!("  Spring home dir {} not found, skipping symlink", target.display());
        }
    }

    // Symlink AI/Interfaces from spring_home
    let ai_interfaces_target = spring_home.join("AI/Interfaces");
    let ai_interfaces_link = base.join("AI/Interfaces");
    if ai_interfaces_target.exists() {
        // Remove the empty dir we created above, replace with symlink
        if ai_interfaces_link.is_dir()
            && std::fs::read_dir(&ai_interfaces_link)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
        {
            std::fs::remove_dir(&ai_interfaces_link)?;
            std::os::unix::fs::symlink(&ai_interfaces_target, &ai_interfaces_link)?;
            tracing::info!(
                "  Symlinked AI/Interfaces -> {}",
                ai_interfaces_target.display()
            );
        }
    }

    // 4. Install SAI bridge
    let ai_dir = base.join("AI/Skirmish/AgentBridge/0.1");
    let lib_dest = ai_dir.join("libSkirmishAI.so");
    if sai_bridge_lib.exists() {
        if should_update(&lib_dest, sai_bridge_lib)? {
            std::fs::copy(sai_bridge_lib, &lib_dest)?;
            tracing::info!("  Installed libSkirmishAI.so");
        }
    } else {
        tracing::warn!(
            "  SAI bridge lib not found at {}, skipping",
            sai_bridge_lib.display()
        );
    }

    // Copy AIInfo.lua and AIOptions.lua
    for name in &["AIInfo.lua", "AIOptions.lua"] {
        let src = sai_bridge_data.join(name);
        let dest = ai_dir.join(name);
        if src.exists() && should_update(&dest, &src)? {
            std::fs::copy(&src, &dest)?;
            tracing::info!("  Installed {}", name);
        }
    }

    // 5. Install startup widget
    let widget_dest = base.join("LuaUI/Widgets/agent_bootstrap.lua");
    if widget_source.exists() && should_update(&widget_dest, widget_source)? {
        std::fs::copy(widget_source, &widget_dest)?;
        tracing::info!("  Installed agent_bootstrap.lua");
    }

    // 6. Generate agent bootstrap config
    let config_path = base.join("LuaUI/Config/agent_bootstrap.json");
    if !config_path.exists() {
        let config = serde_json::json!({
            "players": {
                agent_name: {
                    "ai": "AgentBridge",
                    "version": "0.1"
                }
            }
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
        tracing::info!("  Generated agent_bootstrap.json for '{}'", agent_name);
    }

    // 7. Generate springsettings.cfg if missing
    let settings_path = base.join("springsettings.cfg");
    if !settings_path.exists() {
        std::fs::write(
            &settings_path,
            HEADLESS_SETTINGS,
        )?;
        tracing::info!("  Generated springsettings.cfg");
    }

    tracing::info!("Write-dir initialization complete");
    Ok(())
}

/// Check if dest file needs updating (missing or older than src).
fn should_update(dest: &Path, src: &Path) -> anyhow::Result<bool> {
    if !dest.exists() {
        return Ok(true);
    }
    let src_meta = std::fs::metadata(src)?;
    let dest_meta = std::fs::metadata(dest)?;
    let src_mod = src_meta.modified()?;
    let dest_mod = dest_meta.modified()?;
    Ok(src_mod > dest_mod)
}

/// Resolve paths for SAI bridge components.
pub struct WriteDirConfig {
    pub write_dir: PathBuf,
    pub spring_home: PathBuf,
    pub sai_bridge_lib: PathBuf,
    pub sai_bridge_data: PathBuf,
    pub widget_source: PathBuf,
    pub agent_name: String,
}

impl WriteDirConfig {
    /// Build config from CLI args / env vars / defaults.
    pub fn from_env(
        write_dir: Option<&str>,
        spring_home: Option<&str>,
        agent_name: Option<&str>,
    ) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());

        let write_dir = write_dir
            .map(PathBuf::from)
            .or_else(|| std::env::var("AGENT_WRITE_DIR").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(format!("{}/.spring-loom", home)));

        let spring_home = spring_home
            .map(PathBuf::from)
            .or_else(|| std::env::var("SPRING_HOME").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(format!("{}/.spring", home)));

        let agent_name = agent_name
            .map(String::from)
            .or_else(|| std::env::var("AGENT_NAME").ok())
            .unwrap_or_else(|| "loom".into());

        // SAI bridge lib: check env, then relative to game-manager binary
        let sai_bridge_lib = std::env::var("SAI_BRIDGE_LIB")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Try path relative to workspace
                let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                workspace
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("sai-bridge/target/release/libSkirmishAI.so")
            });

        let sai_bridge_data = std::env::var("SAI_BRIDGE_DATA")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                workspace
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("sai-bridge/data")
            });

        let widget_source = std::env::var("WIDGET_SOURCE")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                workspace.join("data/widgets/agent_bootstrap.lua")
            });

        Self {
            write_dir,
            spring_home,
            sai_bridge_lib,
            sai_bridge_data,
            widget_source,
            agent_name,
        }
    }

    pub fn init(&self) -> anyhow::Result<()> {
        init_write_dir(
            &self.write_dir,
            &self.spring_home,
            &self.sai_bridge_lib,
            &self.sai_bridge_data,
            &self.widget_source,
            &self.agent_name,
        )
    }
}

const HEADLESS_SETTINGS: &str = "\
XResolution=1
YResolution=1
WindowState=0
Fullscreen=0
VSync=0
ROAM=0
SmoothLines=0
SmoothPoints=0
FSAA=0
FSAALevel=0
AdvSky=0
DynamicSky=0
3DTrees=0
HighResInfoTexture=0
GroundDetail=1
UnitLodDist=0
GrassDetail=0
MaxParticles=0
GroundDecals=0
UnitIconDist=0
MaxSounds=0
snd_volmaster=0
";
