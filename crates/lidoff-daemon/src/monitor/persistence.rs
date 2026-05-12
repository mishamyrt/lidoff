use std::path::Path;

use super::effects::restore_display_state;
use super::state::{MonitorState, SharedMonitorState, lock_state};
use crate::logging;
use crate::recovery_state::{self, RecoveryStateData};

fn recovery_state_data(state: &MonitorState) -> RecoveryStateData {
    RecoveryStateData {
        internal_display_state: state.internal_display_state,
        external_display_state: state.external_display_state.clone(),
        keyboard_backlight_state: state.keyboard_backlight_state,
    }
}

fn persist_recovery_state_data(recovery_state: &RecoveryStateData, recovery_cache_dir: &Path) {
    if recovery_state.internal_display_state.is_some()
        || recovery_state.external_display_state.is_some()
        || recovery_state.keyboard_backlight_state.is_some()
    {
        if let Err(error) = recovery_state::save(recovery_cache_dir, recovery_state) {
            logging::error!("failed to persist recovery state: {error}");
        }
    } else {
        if let Err(error) = recovery_state::clear(recovery_cache_dir) {
            logging::error!("failed to clear recovery state: {error}");
        }
    }
}

pub(super) fn persist_recovery_state(
    shared_state: &SharedMonitorState,
    recovery_cache_dir: &Path,
) {
    let recovery_state = {
        let state = lock_state(shared_state);
        recovery_state_data(&state)
    };
    persist_recovery_state_data(&recovery_state, recovery_cache_dir);
}

pub(super) fn recover_state_if_needed(
    shared_state: &SharedMonitorState,
    recovery_cache_dir: &Path,
) {
    let Some(recovery_state) = recovery_state::load(recovery_cache_dir) else {
        return;
    };

    logging::info!("recovery state detected, attempting restore");
    {
        let mut state = lock_state(shared_state);
        state.internal_display_state = recovery_state.internal_display_state;
        state.external_display_state = recovery_state.external_display_state;
        state.keyboard_backlight_state = recovery_state.keyboard_backlight_state;
    }
    restore_display_state(shared_state, true, true);
    persist_recovery_state(shared_state, recovery_cache_dir);
}
