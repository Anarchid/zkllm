//! SAI Bridge — Recoil/Spring SkirmishAI shared library.
//!
//! Exports init(), release(), handleEvent() as C functions.
//! Routes engine events to GameManager via Unix socket IPC,
//! receives commands back, and dispatches them to the engine.

pub mod callbacks;
pub mod commands;
pub mod events;
pub mod ipc;

use callbacks::{EngineCallbacks, SSkirmishAICallback};
use events::{parse_event, GameEvent, EVENT_INIT, EVENT_UPDATE};
use ipc::IpcClient;
use std::ffi::{c_int, c_void};
use std::sync::Mutex;

/// Per-AI instance state.
struct AiInstance {
    callbacks: EngineCallbacks,
    ipc: Option<IpcClient>,
    frame_counter: u32,
}

/// Global AI instance storage. Recoil supports up to 255 AIs,
/// but we typically only have one.
static INSTANCES: Mutex<Vec<Option<AiInstance>>> = Mutex::new(Vec::new());

/// How often to send UPDATE events over IPC (not every frame).
/// At 30 fps, every 30 frames = ~1 second.
const UPDATE_INTERVAL: u32 = 30;

fn get_socket_path(cb: &EngineCallbacks) -> String {
    // 1. connection.json in AI data dir (written by GM before each launch).
    //    Checked first because AIOptions.lua declares a default for socket_path,
    //    so get_option_value always returns *something* — even for dynamically
    //    created AIs via /aicontrol that have no startscript [Options] block.
    if let Some(data_dir) = cb.get_info_value("dataDir") {
        let config_path = format!("{}/connection.json", data_dir.trim_end_matches('/'));
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(path) = config.get("socket_path").and_then(|v| v.as_str()) {
                    cb.log(&format!("[SAI Bridge] Socket path from {}", config_path));
                    return path.to_string();
                }
            }
        }
    }

    // 2. AI option (startscript [Options] — AI-slot mode fallback)
    if let Some(path) = cb.get_option_value("socket_path") {
        cb.log("[SAI Bridge] Socket path from AI option");
        return path;
    }

    // 3. Environment variable
    if let Ok(path) = std::env::var("SAI_SOCKET_PATH") {
        cb.log("[SAI Bridge] Socket path from SAI_SOCKET_PATH env");
        return path;
    }

    // 4. Default
    cb.log("[SAI Bridge] Using default socket path");
    "/tmp/game-manager.sock".to_string()
}

/// Called by the engine when this AI is instantiated.
///
/// # Safety
/// Called by the Recoil engine with valid parameters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init(
    skirmish_ai_id: c_int,
    callback: *const SSkirmishAICallback,
) -> c_int {
    let cb = unsafe { EngineCallbacks::new(skirmish_ai_id, callback) };
    cb.log("[SAI Bridge] Initializing...");

    // Connect to GameManager
    let socket_path = get_socket_path(&cb);
    let ipc = match IpcClient::connect(&socket_path) {
        Ok(mut client) => {
            cb.log(&format!(
                "[SAI Bridge] Connected to GameManager at {}",
                socket_path
            ));
            // Send init event
            let init_event = GameEvent::Init {
                frame: 0,
                saved_game: false,
            };
            if let Err(e) = client.send_event(&init_event) {
                cb.log(&format!("[SAI Bridge] Failed to send init event: {}", e));
            }
            Some(client)
        }
        Err(e) => {
            cb.log(&format!(
                "[SAI Bridge] Failed to connect to GameManager at {}: {}",
                socket_path, e
            ));
            None
        }
    };

    let instance = AiInstance {
        callbacks: cb,
        ipc,
        frame_counter: 0,
    };

    // Store instance
    let mut instances = INSTANCES.lock().unwrap();
    let id = skirmish_ai_id as usize;
    while instances.len() <= id {
        instances.push(None);
    }
    instances[id] = Some(instance);

    0 // success
}

/// Called by the engine when this AI is removed.
///
/// # Safety
/// Called by the Recoil engine with valid parameters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn release(skirmish_ai_id: c_int) -> c_int {
    let mut instances = INSTANCES.lock().unwrap();
    let id = skirmish_ai_id as usize;
    if let Some(Some(instance)) = instances.get_mut(id) {
        instance.callbacks.log("[SAI Bridge] Releasing...");

        // Send release event
        if let Some(ref mut ipc) = instance.ipc {
            let _ = ipc.send_event(&GameEvent::Release { reason: 0 });
        }

        instances[id] = None;
    }
    0
}

/// Main event handler — called by the engine for every game event.
///
/// # Safety
/// Called by the Recoil engine. `data` points to the event-specific struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn handleEvent(
    skirmish_ai_id: c_int,
    topic: c_int,
    data: *const c_void,
) -> c_int {
    let mut instances = INSTANCES.lock().unwrap();
    let id = skirmish_ai_id as usize;

    let instance = match instances.get_mut(id).and_then(|i| i.as_mut()) {
        Some(i) => i,
        None => return -1,
    };

    // Handle EVENT_INIT specially — it also carries the callback pointer
    if topic == EVENT_INIT {
        let init_data = unsafe { &*(data as *const events::SInitEvent) };
        instance.callbacks =
            unsafe { EngineCallbacks::new(skirmish_ai_id, init_data.callback) };

        if let Some(ref mut ipc) = instance.ipc {
            let event = GameEvent::Init {
                frame: 0,
                saved_game: init_data.saved_game,
            };
            let _ = ipc.send_event(&event);
        }
        return 0;
    }

    // For UPDATE events, throttle and poll for incoming commands
    if topic == EVENT_UPDATE {
        instance.frame_counter += 1;

        // Poll for commands from GameManager every frame
        if let Some(ref mut ipc) = instance.ipc {
            let cmds = ipc.poll_commands();
            for cmd in &cmds {
                if let Err(e) = commands::dispatch(&instance.callbacks, cmd) {
                    instance
                        .callbacks
                        .log(&format!("[SAI Bridge] Command error: {}", e));
                }
            }
        }

        // Only send update events at throttled rate
        if instance.frame_counter % UPDATE_INTERVAL != 0 {
            return 0;
        }
    }

    // Parse and forward the event
    if let Some(event) = unsafe { parse_event(topic, data) } {
        if let Some(ref mut ipc) = instance.ipc {
            if let Err(e) = ipc.send_event(&event) {
                instance
                    .callbacks
                    .log(&format!("[SAI Bridge] IPC send error: {}", e));
                // Connection lost — clear it
                instance.ipc = None;
            }
        }
    }

    0
}
