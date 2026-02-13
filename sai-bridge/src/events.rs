//! Event topic IDs and their data structs.
//! Maps from the C `topicId` + `data` pointer to serializable Rust types.

use crate::callbacks::EngineCallbacks;
use serde::Serialize;
use std::ffi::{c_char, c_float, c_int, c_void, CStr};

// ── Event topic constants ──

pub const EVENT_NULL: c_int = 0;
pub const EVENT_INIT: c_int = 1;
pub const EVENT_RELEASE: c_int = 2;
pub const EVENT_UPDATE: c_int = 3;
pub const EVENT_MESSAGE: c_int = 4;
pub const EVENT_UNIT_CREATED: c_int = 5;
pub const EVENT_UNIT_FINISHED: c_int = 6;
pub const EVENT_UNIT_IDLE: c_int = 7;
pub const EVENT_UNIT_MOVE_FAILED: c_int = 8;
pub const EVENT_UNIT_DAMAGED: c_int = 9;
pub const EVENT_UNIT_DESTROYED: c_int = 10;
pub const EVENT_UNIT_GIVEN: c_int = 11;
pub const EVENT_UNIT_CAPTURED: c_int = 12;
pub const EVENT_ENEMY_ENTER_LOS: c_int = 13;
pub const EVENT_ENEMY_LEAVE_LOS: c_int = 14;
pub const EVENT_ENEMY_ENTER_RADAR: c_int = 15;
pub const EVENT_ENEMY_LEAVE_RADAR: c_int = 16;
pub const EVENT_ENEMY_DAMAGED: c_int = 17;
pub const EVENT_ENEMY_DESTROYED: c_int = 18;
pub const EVENT_WEAPON_FIRED: c_int = 19;
pub const EVENT_PLAYER_COMMAND: c_int = 20;
pub const EVENT_SEISMIC_PING: c_int = 21;
pub const EVENT_COMMAND_FINISHED: c_int = 22;
pub const EVENT_LOAD: c_int = 23;
pub const EVENT_SAVE: c_int = 24;
pub const EVENT_ENEMY_CREATED: c_int = 25;
pub const EVENT_ENEMY_FINISHED: c_int = 26;
pub const EVENT_LUA_MESSAGE: c_int = 27;

// ── C event data structs (repr(C), read-only) ──

#[repr(C)]
pub struct SInitEvent {
    pub skirmish_ai_id: c_int,
    pub callback: *const crate::callbacks::SSkirmishAICallback,
    pub saved_game: bool,
}

#[repr(C)]
pub struct SReleaseEvent {
    pub reason: c_int, // 0=unspec, 1=game_ended, 2=team_died, 3=ai_killed, etc.
}

#[repr(C)]
pub struct SUpdateEvent {
    pub frame: c_int,
}

#[repr(C)]
pub struct SMessageEvent {
    pub player: c_int,
    pub message: *const c_char,
}

#[repr(C)]
pub struct SUnitCreatedEvent {
    pub unit: c_int,
    pub builder: c_int,
}

#[repr(C)]
pub struct SUnitFinishedEvent {
    pub unit: c_int,
}

#[repr(C)]
pub struct SUnitIdleEvent {
    pub unit: c_int,
}

#[repr(C)]
pub struct SUnitMoveFailedEvent {
    pub unit: c_int,
}

#[repr(C)]
pub struct SUnitDamagedEvent {
    pub unit: c_int,
    pub attacker: c_int,
    pub damage: c_float,
    pub dir: *const [c_float; 3],
    pub weapon_def_id: c_int,
    pub paralyzer: bool,
}

#[repr(C)]
pub struct SUnitDestroyedEvent {
    pub unit: c_int,
    pub attacker: c_int,
    pub weapon_def_id: c_int,
}

#[repr(C)]
pub struct SUnitGivenEvent {
    pub unit_id: c_int,
    pub old_team_id: c_int,
    pub new_team_id: c_int,
}

#[repr(C)]
pub struct SUnitCapturedEvent {
    pub unit_id: c_int,
    pub old_team_id: c_int,
    pub new_team_id: c_int,
}

#[repr(C)]
pub struct SEnemyEnterLOSEvent {
    pub enemy: c_int,
}

#[repr(C)]
pub struct SEnemyLeaveLOSEvent {
    pub enemy: c_int,
}

#[repr(C)]
pub struct SEnemyEnterRadarEvent {
    pub enemy: c_int,
}

#[repr(C)]
pub struct SEnemyLeaveRadarEvent {
    pub enemy: c_int,
}

#[repr(C)]
pub struct SEnemyDamagedEvent {
    pub enemy: c_int,
    pub attacker: c_int,
    pub damage: c_float,
    pub dir: *const [c_float; 3],
    pub weapon_def_id: c_int,
    pub paralyzer: bool,
}

#[repr(C)]
pub struct SEnemyDestroyedEvent {
    pub enemy: c_int,
    pub attacker: c_int,
}

#[repr(C)]
pub struct SWeaponFiredEvent {
    pub unit_id: c_int,
    pub weapon_def_id: c_int,
}

#[repr(C)]
pub struct SCommandFinishedEvent {
    pub unit_id: c_int,
    pub command_id: c_int,
    pub command_topic_id: c_int,
}

#[repr(C)]
pub struct SLuaMessageEvent {
    pub in_data: *const c_char,
}

#[repr(C)]
pub struct SEnemyCreatedEvent {
    pub enemy: c_int,
}

#[repr(C)]
pub struct SEnemyFinishedEvent {
    pub enemy: c_int,
}

// ── Serializable game event (sent over IPC to GameManager) ──

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum GameEvent {
    #[serde(rename = "init")]
    Init { frame: i32, saved_game: bool },

    #[serde(rename = "release")]
    Release { reason: i32 },

    #[serde(rename = "update")]
    Update { frame: i32 },

    #[serde(rename = "message")]
    Message { player: i32, text: String },

    #[serde(rename = "unit_created")]
    UnitCreated {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        builder: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        builder_name: Option<String>,
    },

    #[serde(rename = "unit_finished")]
    UnitFinished {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
    },

    #[serde(rename = "unit_idle")]
    UnitIdle {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
    },

    #[serde(rename = "unit_move_failed")]
    UnitMoveFailed {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
    },

    #[serde(rename = "unit_damaged")]
    UnitDamaged {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        attacker: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        attacker_name: Option<String>,
        damage: f32,
        weapon_def_id: i32,
        paralyzer: bool,
    },

    #[serde(rename = "unit_destroyed")]
    UnitDestroyed {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        attacker: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        attacker_name: Option<String>,
        weapon_def_id: i32,
    },

    #[serde(rename = "unit_given")]
    UnitGiven {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        old_team: i32,
        new_team: i32,
    },

    #[serde(rename = "unit_captured")]
    UnitCaptured {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        old_team: i32,
        new_team: i32,
    },

    #[serde(rename = "enemy_enter_los")]
    EnemyEnterLos {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
    },

    #[serde(rename = "enemy_leave_los")]
    EnemyLeaveLos {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
    },

    #[serde(rename = "enemy_enter_radar")]
    EnemyEnterRadar {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
    },

    #[serde(rename = "enemy_leave_radar")]
    EnemyLeaveRadar {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
    },

    #[serde(rename = "enemy_damaged")]
    EnemyDamaged {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
        attacker: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        attacker_name: Option<String>,
        damage: f32,
        weapon_def_id: i32,
        paralyzer: bool,
    },

    #[serde(rename = "enemy_destroyed")]
    EnemyDestroyed {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
        attacker: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        attacker_name: Option<String>,
    },

    #[serde(rename = "enemy_created")]
    EnemyCreated {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
    },

    #[serde(rename = "enemy_finished")]
    EnemyFinished {
        enemy: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        enemy_name: Option<String>,
    },

    #[serde(rename = "weapon_fired")]
    WeaponFired {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        weapon_def_id: i32,
    },

    #[serde(rename = "command_finished")]
    CommandFinished {
        unit: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit_name: Option<String>,
        command_id: i32,
        command_topic: i32,
    },

    #[serde(rename = "lua_message")]
    LuaMessage { data: String },

    #[serde(rename = "command_error")]
    CommandError { error: String, command: String },
}

/// Convert a raw C event (topic + data pointer) into a serializable GameEvent.
///
/// # Safety
/// `data` must be a valid pointer to the correct struct for the given `topic`.
pub unsafe fn parse_event(topic: c_int, data: *const c_void) -> Option<GameEvent> {
    match topic {
        EVENT_INIT => {
            let e = &*(data as *const SInitEvent);
            Some(GameEvent::Init {
                frame: 0,
                saved_game: e.saved_game,
            })
        }
        EVENT_RELEASE => {
            let e = &*(data as *const SReleaseEvent);
            Some(GameEvent::Release { reason: e.reason })
        }
        EVENT_UPDATE => {
            let e = &*(data as *const SUpdateEvent);
            Some(GameEvent::Update { frame: e.frame })
        }
        EVENT_MESSAGE => {
            let e = &*(data as *const SMessageEvent);
            let text = if e.message.is_null() {
                String::new()
            } else {
                CStr::from_ptr(e.message).to_string_lossy().into_owned()
            };
            Some(GameEvent::Message {
                player: e.player,
                text,
            })
        }
        EVENT_UNIT_CREATED => {
            let e = &*(data as *const SUnitCreatedEvent);
            Some(GameEvent::UnitCreated {
                unit: e.unit,
                unit_name: None,
                builder: e.builder,
                builder_name: None,
            })
        }
        EVENT_UNIT_FINISHED => {
            let e = &*(data as *const SUnitFinishedEvent);
            Some(GameEvent::UnitFinished { unit: e.unit, unit_name: None })
        }
        EVENT_UNIT_IDLE => {
            let e = &*(data as *const SUnitIdleEvent);
            Some(GameEvent::UnitIdle { unit: e.unit, unit_name: None })
        }
        EVENT_UNIT_MOVE_FAILED => {
            let e = &*(data as *const SUnitMoveFailedEvent);
            Some(GameEvent::UnitMoveFailed { unit: e.unit, unit_name: None })
        }
        EVENT_UNIT_DAMAGED => {
            let e = &*(data as *const SUnitDamagedEvent);
            Some(GameEvent::UnitDamaged {
                unit: e.unit,
                unit_name: None,
                attacker: e.attacker,
                attacker_name: None,
                damage: e.damage,
                weapon_def_id: e.weapon_def_id,
                paralyzer: e.paralyzer,
            })
        }
        EVENT_UNIT_DESTROYED => {
            let e = &*(data as *const SUnitDestroyedEvent);
            Some(GameEvent::UnitDestroyed {
                unit: e.unit,
                unit_name: None,
                attacker: e.attacker,
                attacker_name: None,
                weapon_def_id: e.weapon_def_id,
            })
        }
        EVENT_UNIT_GIVEN => {
            let e = &*(data as *const SUnitGivenEvent);
            Some(GameEvent::UnitGiven {
                unit: e.unit_id,
                unit_name: None,
                old_team: e.old_team_id,
                new_team: e.new_team_id,
            })
        }
        EVENT_UNIT_CAPTURED => {
            let e = &*(data as *const SUnitCapturedEvent);
            Some(GameEvent::UnitCaptured {
                unit: e.unit_id,
                unit_name: None,
                old_team: e.old_team_id,
                new_team: e.new_team_id,
            })
        }
        EVENT_ENEMY_ENTER_LOS => {
            let e = &*(data as *const SEnemyEnterLOSEvent);
            Some(GameEvent::EnemyEnterLos { enemy: e.enemy, enemy_name: None })
        }
        EVENT_ENEMY_LEAVE_LOS => {
            let e = &*(data as *const SEnemyLeaveLOSEvent);
            Some(GameEvent::EnemyLeaveLos { enemy: e.enemy, enemy_name: None })
        }
        EVENT_ENEMY_ENTER_RADAR => {
            let e = &*(data as *const SEnemyEnterRadarEvent);
            Some(GameEvent::EnemyEnterRadar { enemy: e.enemy, enemy_name: None })
        }
        EVENT_ENEMY_LEAVE_RADAR => {
            let e = &*(data as *const SEnemyLeaveRadarEvent);
            Some(GameEvent::EnemyLeaveRadar { enemy: e.enemy, enemy_name: None })
        }
        EVENT_ENEMY_DAMAGED => {
            let e = &*(data as *const SEnemyDamagedEvent);
            Some(GameEvent::EnemyDamaged {
                enemy: e.enemy,
                enemy_name: None,
                attacker: e.attacker,
                attacker_name: None,
                damage: e.damage,
                weapon_def_id: e.weapon_def_id,
                paralyzer: e.paralyzer,
            })
        }
        EVENT_ENEMY_DESTROYED => {
            let e = &*(data as *const SEnemyDestroyedEvent);
            Some(GameEvent::EnemyDestroyed {
                enemy: e.enemy,
                enemy_name: None,
                attacker: e.attacker,
                attacker_name: None,
            })
        }
        EVENT_ENEMY_CREATED => {
            let e = &*(data as *const SEnemyCreatedEvent);
            Some(GameEvent::EnemyCreated { enemy: e.enemy, enemy_name: None })
        }
        EVENT_ENEMY_FINISHED => {
            let e = &*(data as *const SEnemyFinishedEvent);
            Some(GameEvent::EnemyFinished { enemy: e.enemy, enemy_name: None })
        }
        EVENT_WEAPON_FIRED => {
            let e = &*(data as *const SWeaponFiredEvent);
            Some(GameEvent::WeaponFired {
                unit: e.unit_id,
                unit_name: None,
                weapon_def_id: e.weapon_def_id,
            })
        }
        EVENT_COMMAND_FINISHED => {
            let e = &*(data as *const SCommandFinishedEvent);
            Some(GameEvent::CommandFinished {
                unit: e.unit_id,
                unit_name: None,
                command_id: e.command_id,
                command_topic: e.command_topic_id,
            })
        }
        EVENT_LUA_MESSAGE => {
            let e = &*(data as *const SLuaMessageEvent);
            let data_str = if e.in_data.is_null() {
                String::new()
            } else {
                CStr::from_ptr(e.in_data).to_string_lossy().into_owned()
            };
            Some(GameEvent::LuaMessage { data: data_str })
        }
        _ => None,
    }
}

/// Resolve a unit instance ID to its definition name via engine callbacks.
/// Returns None for invalid IDs (e.g. 0 or -1 for "no attacker").
fn resolve_unit_name(cb: &EngineCallbacks, unit_id: i32) -> Option<String> {
    if unit_id <= 0 {
        return None;
    }
    let def_id = cb.unit_get_def(unit_id);
    if def_id < 0 {
        cb.log(&format!("[SAI enrich] unit_get_def({}) returned {}", unit_id, def_id));
        return None;
    }
    let name = cb.unit_def_get_name(def_id);
    cb.log(&format!("[SAI enrich] unit {} -> def {} -> {:?}", unit_id, def_id, name));
    name
}

/// Enrich a parsed event with human-readable unit names from the engine.
pub fn enrich_event(event: &mut GameEvent, cb: &EngineCallbacks) {
    match event {
        GameEvent::UnitCreated { unit, unit_name, builder, builder_name, .. } => {
            *unit_name = resolve_unit_name(cb, *unit);
            *builder_name = resolve_unit_name(cb, *builder);
        }
        GameEvent::UnitFinished { unit, unit_name, .. } |
        GameEvent::UnitIdle { unit, unit_name, .. } |
        GameEvent::UnitMoveFailed { unit, unit_name, .. } => {
            *unit_name = resolve_unit_name(cb, *unit);
        }
        GameEvent::UnitDamaged { unit, unit_name, attacker, attacker_name, .. } |
        GameEvent::UnitDestroyed { unit, unit_name, attacker, attacker_name, .. } => {
            *unit_name = resolve_unit_name(cb, *unit);
            *attacker_name = resolve_unit_name(cb, *attacker);
        }
        GameEvent::UnitGiven { unit, unit_name, .. } |
        GameEvent::UnitCaptured { unit, unit_name, .. } => {
            *unit_name = resolve_unit_name(cb, *unit);
        }
        GameEvent::EnemyEnterLos { enemy, enemy_name, .. } |
        GameEvent::EnemyLeaveLos { enemy, enemy_name, .. } |
        GameEvent::EnemyEnterRadar { enemy, enemy_name, .. } |
        GameEvent::EnemyLeaveRadar { enemy, enemy_name, .. } |
        GameEvent::EnemyCreated { enemy, enemy_name, .. } |
        GameEvent::EnemyFinished { enemy, enemy_name, .. } => {
            *enemy_name = resolve_unit_name(cb, *enemy);
        }
        GameEvent::EnemyDamaged { enemy, enemy_name, attacker, attacker_name, .. } |
        GameEvent::EnemyDestroyed { enemy, enemy_name, attacker, attacker_name, .. } => {
            *enemy_name = resolve_unit_name(cb, *enemy);
            *attacker_name = resolve_unit_name(cb, *attacker);
        }
        GameEvent::WeaponFired { unit, unit_name, .. } |
        GameEvent::CommandFinished { unit, unit_name, .. } => {
            *unit_name = resolve_unit_name(cb, *unit);
        }
        _ => {}
    }
}
