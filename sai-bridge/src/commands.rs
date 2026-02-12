//! Command dispatch: receives JSON commands from GameManager,
//! converts them to C structs, and calls Engine_handleCommand.

use crate::callbacks::*;
use serde::Deserialize;
use std::ffi::{c_float, c_int, c_void, CString};

/// Commands received from GameManager over IPC.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum GameCommand {
    #[serde(rename = "move")]
    Move {
        unit_id: i32,
        x: f32,
        y: f32,
        z: f32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "stop")]
    Stop { unit_id: i32 },

    #[serde(rename = "attack")]
    Attack {
        unit_id: i32,
        target_id: i32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "build")]
    Build {
        unit_id: i32,
        build_def_id: i32,
        x: f32,
        y: f32,
        z: f32,
        #[serde(default)]
        facing: i32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "patrol")]
    Patrol {
        unit_id: i32,
        x: f32,
        y: f32,
        z: f32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "fight")]
    Fight {
        unit_id: i32,
        x: f32,
        y: f32,
        z: f32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "guard")]
    Guard {
        unit_id: i32,
        guard_id: i32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "repair")]
    Repair {
        unit_id: i32,
        repair_id: i32,
        #[serde(default)]
        queue: bool,
    },

    #[serde(rename = "set_fire_state")]
    SetFireState { unit_id: i32, state: i32 },

    #[serde(rename = "set_move_state")]
    SetMoveState { unit_id: i32, state: i32 },

    #[serde(rename = "send_chat")]
    SendChat { text: String },
}

/// Dispatch a GameCommand to the engine via callbacks.
/// Returns Ok(()) on success, Err with description on failure.
pub fn dispatch(cb: &EngineCallbacks, cmd: &GameCommand) -> Result<(), String> {
    let result = match cmd {
        GameCommand::Move {
            unit_id,
            x,
            y,
            z,
            queue,
        } => {
            let mut pos: [c_float; 3] = [*x, *y, *z];
            let mut data = SMoveUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_pos: &mut pos as *mut [c_float; 3],
            };
            cb.handle_command(0, COMMAND_UNIT_MOVE, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Stop { unit_id } => {
            let mut data = SStopUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: 0,
                time_out: i32::MAX,
            };
            cb.handle_command(0, COMMAND_UNIT_STOP, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Attack {
            unit_id,
            target_id,
            queue,
        } => {
            let mut data = SAttackUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_attack_unit_id: *target_id as c_int,
            };
            cb.handle_command(0, COMMAND_UNIT_ATTACK, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Build {
            unit_id,
            build_def_id,
            x,
            y,
            z,
            facing,
            queue,
        } => {
            let mut pos: [c_float; 3] = [*x, *y, *z];
            let mut data = SBuildUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_build_unit_def_id: *build_def_id as c_int,
                build_pos: &mut pos as *mut [c_float; 3],
                facing: *facing as c_int,
            };
            cb.handle_command(0, COMMAND_UNIT_BUILD, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Patrol {
            unit_id,
            x,
            y,
            z,
            queue,
        } => {
            let mut pos: [c_float; 3] = [*x, *y, *z];
            let mut data = SPatrolUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_pos: &mut pos as *mut [c_float; 3],
            };
            cb.handle_command(0, COMMAND_UNIT_PATROL, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Fight {
            unit_id,
            x,
            y,
            z,
            queue,
        } => {
            let mut pos: [c_float; 3] = [*x, *y, *z];
            let mut data = SFightUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_pos: &mut pos as *mut [c_float; 3],
            };
            cb.handle_command(0, COMMAND_UNIT_FIGHT, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Guard {
            unit_id,
            guard_id,
            queue,
        } => {
            let mut data = SGuardUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_guard_unit_id: *guard_id as c_int,
            };
            cb.handle_command(0, COMMAND_UNIT_GUARD, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::Repair {
            unit_id,
            repair_id,
            queue,
        } => {
            let mut data = SRepairUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: if *queue { UNIT_COMMAND_OPTION_SHIFT_KEY } else { 0 },
                time_out: i32::MAX,
                to_repair_unit_id: *repair_id as c_int,
            };
            cb.handle_command(0, COMMAND_UNIT_REPAIR, &mut data as *mut _ as *mut c_void)
        }

        GameCommand::SetFireState { unit_id, state } => {
            let mut data = SSetFireStateUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: 0,
                time_out: i32::MAX,
                fire_state: *state as c_int,
            };
            cb.handle_command(
                0,
                COMMAND_UNIT_SET_FIRE_STATE,
                &mut data as *mut _ as *mut c_void,
            )
        }

        GameCommand::SetMoveState { unit_id, state } => {
            let mut data = SSetMoveStateUnitCommand {
                unit_id: *unit_id as c_int,
                group_id: -1,
                options: 0,
                time_out: i32::MAX,
                move_state: *state as c_int,
            };
            cb.handle_command(
                0,
                COMMAND_UNIT_SET_MOVE_STATE,
                &mut data as *mut _ as *mut c_void,
            )
        }

        GameCommand::SendChat { text } => {
            let c_text = CString::new(text.as_str()).map_err(|e| e.to_string())?;
            let mut data = SSendTextMessageCommand {
                text: c_text.as_ptr(),
                zone: 0,
            };
            cb.handle_command(
                0,
                COMMAND_SEND_TEXT_MESSAGE,
                &mut data as *mut _ as *mut c_void,
            )
        }
    };

    if result == 0 {
        Ok(())
    } else {
        Err(format!("Engine_handleCommand returned {}", result))
    }
}
