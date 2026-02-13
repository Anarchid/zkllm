//! Binding for SSkirmishAICallback — the 596-entry function pointer table
//! provided by the Recoil engine to skirmish AIs.
//!
//! We represent it as an opaque array of function pointers and access
//! specific fields by their known indices. The indices are derived from
//! the field order in SSkirmishAICallback.h.
//!
//! For a full binding, use bindgen. This is a Phase 0 minimal subset.

use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::os::raw::c_short;

/// Total number of function pointer fields in SSkirmishAICallback.
const CALLBACK_FIELD_COUNT: usize = 596;

/// The raw callback struct — represented as an array of function pointers.
/// On 64-bit systems each entry is 8 bytes (one pointer).
#[repr(C)]
pub struct SSkirmishAICallback {
    pub vtable: [*const (); CALLBACK_FIELD_COUNT],
}

// ── Field indices (0-based, from SSkirmishAICallback.h field order) ──

const IDX_ENGINE_HANDLE_COMMAND: usize = 0;
const IDX_SKIRMISH_AI_INFO_GET_VALUE_BY_KEY: usize = 22;
const IDX_SKIRMISH_AI_OPTION_VALUES_GET_VALUE_BY_KEY: usize = 26;
const IDX_LOG_LOG: usize = 27;
const IDX_GAME_GET_CURRENT_FRAME: usize = 36;
const IDX_GAME_GET_MY_TEAM: usize = 38;
const IDX_GAME_GET_MY_ALLY_TEAM: usize = 39;
const IDX_GAME_IS_PAUSED: usize = 58;
const IDX_ECONOMY_GET_CURRENT: usize = 74;
const IDX_ECONOMY_GET_INCOME: usize = 75;
const IDX_ECONOMY_GET_USAGE: usize = 76;
const IDX_ECONOMY_GET_STORAGE: usize = 77;
const IDX_GET_UNIT_DEFS: usize = 87;
const IDX_GET_UNIT_DEF_BY_NAME: usize = 88;
const IDX_UNIT_DEF_GET_NAME: usize = 91;
const IDX_UNIT_DEF_GET_HUMAN_NAME: usize = 92;
const IDX_UNIT_GET_DEF: usize = 293;
const IDX_MAP_GET_WIDTH: usize = 394;
const IDX_MAP_GET_HEIGHT: usize = 395;

/// Safe wrapper around the raw callback pointer table.
pub struct EngineCallbacks {
    ai_id: c_int,
    raw: *const SSkirmishAICallback,
}

// SAFETY: The callback pointer table is valid for the AI's entire lifetime
// (between init() and release()). The engine owns the memory.
unsafe impl Send for EngineCallbacks {}

impl EngineCallbacks {
    /// # Safety
    /// `raw` must be a valid, non-null pointer that remains valid until release().
    pub unsafe fn new(ai_id: c_int, raw: *const SSkirmishAICallback) -> Self {
        Self { ai_id, raw }
    }

    /// Read a function pointer from the vtable at the given index and
    /// transmute it to the desired type.
    unsafe fn fn_at<F>(&self, idx: usize) -> F {
        debug_assert!(idx < CALLBACK_FIELD_COUNT);
        let ptr = (*self.raw).vtable[idx];
        std::mem::transmute_copy(&ptr)
    }

    // ── Game state ──

    pub fn get_current_frame(&self) -> i32 {
        type Fn = unsafe extern "C" fn(c_int) -> c_int;
        unsafe { self.fn_at::<Fn>(IDX_GAME_GET_CURRENT_FRAME)(self.ai_id) }
    }

    pub fn get_my_team(&self) -> i32 {
        type Fn = unsafe extern "C" fn(c_int) -> c_int;
        unsafe { self.fn_at::<Fn>(IDX_GAME_GET_MY_TEAM)(self.ai_id) }
    }

    pub fn get_my_ally_team(&self) -> i32 {
        type Fn = unsafe extern "C" fn(c_int) -> c_int;
        unsafe { self.fn_at::<Fn>(IDX_GAME_GET_MY_ALLY_TEAM)(self.ai_id) }
    }

    pub fn is_paused(&self) -> bool {
        type Fn = unsafe extern "C" fn(c_int) -> bool;
        unsafe { self.fn_at::<Fn>(IDX_GAME_IS_PAUSED)(self.ai_id) }
    }

    // ── Economy ──

    pub fn economy_current(&self, resource_id: i32) -> f32 {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> c_float;
        unsafe { self.fn_at::<Fn>(IDX_ECONOMY_GET_CURRENT)(self.ai_id, resource_id) }
    }

    pub fn economy_income(&self, resource_id: i32) -> f32 {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> c_float;
        unsafe { self.fn_at::<Fn>(IDX_ECONOMY_GET_INCOME)(self.ai_id, resource_id) }
    }

    pub fn economy_usage(&self, resource_id: i32) -> f32 {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> c_float;
        unsafe { self.fn_at::<Fn>(IDX_ECONOMY_GET_USAGE)(self.ai_id, resource_id) }
    }

    pub fn economy_storage(&self, resource_id: i32) -> f32 {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> c_float;
        unsafe { self.fn_at::<Fn>(IDX_ECONOMY_GET_STORAGE)(self.ai_id, resource_id) }
    }

    // ── Unit queries ──

    /// Resolve a unit definition name (e.g. "cloakraid") to its numeric ID.
    /// Returns the def ID, or a negative value if not found.
    pub fn get_unit_def_by_name(&self, name: &str) -> Option<i32> {
        let c_name = CString::new(name).ok()?;
        type Fn = unsafe extern "C" fn(c_int, *const c_char) -> c_int;
        let id = unsafe {
            self.fn_at::<Fn>(IDX_GET_UNIT_DEF_BY_NAME)(self.ai_id, c_name.as_ptr())
        };
        if id < 0 { None } else { Some(id) }
    }

    /// Get the unit definition ID for a given unit instance.
    pub fn unit_get_def(&self, unit_id: i32) -> i32 {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> c_int;
        unsafe { self.fn_at::<Fn>(IDX_UNIT_GET_DEF)(self.ai_id, unit_id) }
    }

    /// Get the internal name of a unit definition (e.g. "cloakraid", "armcom1").
    pub fn unit_def_get_name(&self, unit_def_id: i32) -> Option<String> {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> *const c_char;
        unsafe {
            let ptr = self.fn_at::<Fn>(IDX_UNIT_DEF_GET_NAME)(self.ai_id, unit_def_id);
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            }
        }
    }

    /// Get the human-readable name of a unit definition (e.g. "Glaive", "Commander").
    pub fn unit_def_get_human_name(&self, unit_def_id: i32) -> Option<String> {
        type Fn = unsafe extern "C" fn(c_int, c_int) -> *const c_char;
        unsafe {
            let ptr = self.fn_at::<Fn>(IDX_UNIT_DEF_GET_HUMAN_NAME)(self.ai_id, unit_def_id);
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            }
        }
    }

    // ── Map ──

    pub fn map_width(&self) -> i32 {
        type Fn = unsafe extern "C" fn(c_int) -> c_int;
        unsafe { self.fn_at::<Fn>(IDX_MAP_GET_WIDTH)(self.ai_id) }
    }

    pub fn map_height(&self) -> i32 {
        type Fn = unsafe extern "C" fn(c_int) -> c_int;
        unsafe { self.fn_at::<Fn>(IDX_MAP_GET_HEIGHT)(self.ai_id) }
    }

    // ── Logging ──

    pub fn log(&self, msg: &str) {
        if let Ok(c_msg) = CString::new(msg) {
            type Fn = unsafe extern "C" fn(c_int, *const c_char);
            unsafe { self.fn_at::<Fn>(IDX_LOG_LOG)(self.ai_id, c_msg.as_ptr()) }
        }
    }

    // ── Commands ──

    pub fn handle_command(
        &self,
        command_id: c_int,
        command_topic: c_int,
        command_data: *mut c_void,
    ) -> c_int {
        type Fn = unsafe extern "C" fn(c_int, c_int, c_int, c_int, *mut c_void) -> c_int;
        unsafe {
            self.fn_at::<Fn>(IDX_ENGINE_HANDLE_COMMAND)(
                self.ai_id,
                COMMAND_TO_ID_ENGINE,
                command_id,
                command_topic,
                command_data,
            )
        }
    }

    // ── AI info / options ──

    pub fn get_info_value(&self, key: &str) -> Option<String> {
        let c_key = CString::new(key).ok()?;
        type Fn = unsafe extern "C" fn(c_int, *const c_char) -> *const c_char;
        unsafe {
            let ptr = self.fn_at::<Fn>(IDX_SKIRMISH_AI_INFO_GET_VALUE_BY_KEY)(
                self.ai_id,
                c_key.as_ptr(),
            );
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            }
        }
    }

    pub fn get_option_value(&self, key: &str) -> Option<String> {
        let c_key = CString::new(key).ok()?;
        type Fn = unsafe extern "C" fn(c_int, *const c_char) -> *const c_char;
        unsafe {
            let ptr = self.fn_at::<Fn>(IDX_SKIRMISH_AI_OPTION_VALUES_GET_VALUE_BY_KEY)(
                self.ai_id,
                c_key.as_ptr(),
            );
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            }
        }
    }
}

// ── Constants ──

pub const COMMAND_TO_ID_ENGINE: c_int = -1;

// Engine-level command topics
pub const COMMAND_PAUSE: c_int = 5;
pub const COMMAND_SEND_TEXT_MESSAGE: c_int = 6;
pub const COMMAND_SET_GAME_SPEED: c_int = 7;
pub const COMMAND_UNIT_BUILD: c_int = 35;
pub const COMMAND_UNIT_STOP: c_int = 36;
pub const COMMAND_UNIT_MOVE: c_int = 42;
pub const COMMAND_UNIT_PATROL: c_int = 43;
pub const COMMAND_UNIT_FIGHT: c_int = 44;
pub const COMMAND_UNIT_ATTACK: c_int = 45;
pub const COMMAND_UNIT_ATTACK_AREA: c_int = 46;
pub const COMMAND_UNIT_GUARD: c_int = 47;
pub const COMMAND_UNIT_REPAIR: c_int = 51;
pub const COMMAND_UNIT_SET_FIRE_STATE: c_int = 52;
pub const COMMAND_UNIT_SET_MOVE_STATE: c_int = 53;
pub const COMMAND_UNIT_RECLAIM_UNIT: c_int = 63;
pub const COMMAND_UNIT_RECLAIM_AREA: c_int = 64;

// Command option flags
pub const UNIT_COMMAND_OPTION_SHIFT_KEY: c_short = 1 << 5;

// ── Command data structs ──

#[repr(C)]
pub struct SMoveUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_pos: *mut [c_float; 3],
}

#[repr(C)]
pub struct SStopUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
}

#[repr(C)]
pub struct SAttackUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_attack_unit_id: c_int,
}

#[repr(C)]
pub struct SBuildUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_build_unit_def_id: c_int,
    pub build_pos: *mut [c_float; 3],
    pub facing: c_int,
}

#[repr(C)]
pub struct SPatrolUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_pos: *mut [c_float; 3],
}

#[repr(C)]
pub struct SFightUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_pos: *mut [c_float; 3],
}

#[repr(C)]
pub struct SGuardUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_guard_unit_id: c_int,
}

#[repr(C)]
pub struct SRepairUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub to_repair_unit_id: c_int,
}

#[repr(C)]
pub struct SSendTextMessageCommand {
    pub text: *const c_char,
    pub zone: c_int,
}

#[repr(C)]
pub struct SSetFireStateUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub fire_state: c_int,
}

#[repr(C)]
pub struct SSetMoveStateUnitCommand {
    pub unit_id: c_int,
    pub group_id: c_int,
    pub options: c_short,
    pub time_out: c_int,
    pub move_state: c_int,
}

#[repr(C)]
pub struct SPauseCommand {
    pub enable: bool,
    pub is_message: bool,
}

#[repr(C)]
pub struct SSetGameSpeedCommand {
    pub speed: c_float,
}
