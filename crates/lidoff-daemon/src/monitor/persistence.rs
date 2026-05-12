use super::effects::restore_display_state;
use super::state::{MonitorState, SharedMonitorState, lock_state};
use crate::logging;
use crate::recovery_state::{self, RecoveryStateData};

fn recovery_state_data(state: &MonitorState) -> RecoveryStateData {
    RecoveryStateData {
        internal_display_state: state.internal_display_state,
        external_display_state: state.external_display_state.clone(),
    }
}

fn persist_recovery_state_data(recovery_state: &RecoveryStateData) {
    if recovery_state.internal_display_state.is_some()
        || recovery_state.external_display_state.is_some()
    {
        if !recovery_state::save(recovery_state) {
            logging::error!("failed to persist recovery state");
        }
    } else {
        recovery_state::clear();
    }
}

pub(super) fn persist_recovery_state(shared_state: &SharedMonitorState) {
    let recovery_state = {
        let state = lock_state(shared_state);
        recovery_state_data(&state)
    };
    persist_recovery_state_data(&recovery_state);
}

pub(super) fn recover_state_if_needed(shared_state: &SharedMonitorState) {
    let Some(recovery_state) = recovery_state::load() else {
        return;
    };

    logging::info!("recovery state detected, attempting restore");
    {
        let mut state = lock_state(shared_state);
        state.internal_display_state = recovery_state.internal_display_state;
        state.external_display_state = recovery_state.external_display_state;
    }
    restore_display_state(shared_state, true, true);
    persist_recovery_state(shared_state);
}
