use std::sync::OnceLock;
use std::time::Instant;

use super::effects::prepare_display_state_for_sleep;
use super::persistence::persist_recovery_state;
use super::state::{SharedMonitorState, lock_state};

static POWER_STATE: OnceLock<SharedMonitorState> = OnceLock::new();

pub(super) fn set_shared_state(shared_state: SharedMonitorState) {
    let _ = POWER_STATE.set(shared_state);
}

pub(super) extern "C" fn handle_will_sleep(_context: *mut std::ffi::c_void) {
    let Some(shared_state) = POWER_STATE.get() else {
        return;
    };

    let now = Instant::now();
    {
        let mut state = lock_state(shared_state);
        state.system_sleeping = true;
        state.last_full_close_at = Some(now);
        state.last_open_at = None;
        state.last_wake_at = None;
        state.awaiting_open_after_full_close = true;
        state.keep_internal_restore_until = None;
        state.last_angle = None;
        state.last_lid_state = None;
        state.below_threshold_streak = 0;
    }
    prepare_display_state_for_sleep(shared_state, false);
    persist_recovery_state(shared_state);
}

pub(super) extern "C" fn handle_did_wake(_context: *mut std::ffi::c_void) {
    let Some(shared_state) = POWER_STATE.get() else {
        return;
    };

    let now = Instant::now();
    let mut state = lock_state(shared_state);
    state.system_sleeping = false;
    state.last_wake_at = Some(now);
    state.last_full_close_at = Some(now);
    state.last_open_at = None;
    state.awaiting_open_after_full_close = true;
    state.keep_internal_restore_until = None;
    state.last_angle = None;
    state.last_lid_state = None;
    state.below_threshold_streak = 0;
}
