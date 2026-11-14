use std::marker::PhantomData;
use std::rc::Rc;

use rkyv::{Archive, Deserialize, Serialize};
use smallvec::SmallVec;
use thiserror::Error;

use crate::shim::{
    disable_skylight_display, finalize_skylight, is_builtin, online_displays,
    restore_skylight_state,
};

#[derive(Debug, Default)]
pub struct ExternalDisplays {
    _not_thread_safe: PhantomData<Rc<()>>,
}

#[derive(Clone, Debug, Default, PartialEq, Archive, Serialize, Deserialize)]
pub struct ExternalDisplayState {
    pub skylight_display_ids: SmallVec<[u32; 4]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalDisplayDisableResult {
    pub state: ExternalDisplayState,
    pub disabled: usize,
    pub failed: usize,
}

#[derive(Error, Debug)]
pub enum ExternalDisplayError {
    #[error("failed to get online displays")]
    GetOnlineDisplaysFailed,

    #[error("failed to disable {failed} external display(s), disabled {disabled}")]
    DisableFailed { disabled: usize, failed: usize },

    #[error("failed to restore {failed} external display(s), restored {restored}")]
    RestoreFailed { restored: usize, failed: usize },
}

impl ExternalDisplays {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restores displays and removes successfully restored IDs from the state.
    pub fn restore_state(
        &mut self,
        state: &mut ExternalDisplayState,
    ) -> Result<(), ExternalDisplayError> {
        let total = state.skylight_display_ids.len();
        state
            .skylight_display_ids
            .retain(|id| restore_skylight_state(std::slice::from_ref(id)) == 0);
        let failed = state.skylight_display_ids.len();

        if failed > 0 {
            Err(ExternalDisplayError::RestoreFailed { restored: total - failed, failed })
        } else {
            Ok(())
        }
    }

    pub fn capture_and_disable(
        &mut self,
        already_disabled: &ExternalDisplayState,
    ) -> Result<ExternalDisplayDisableResult, ExternalDisplayError> {
        let displays =
            online_displays().ok_or(ExternalDisplayError::GetOnlineDisplaysFailed)?;

        let (mut state, mut failed) =
            capture_disabled_external_displays(&displays, already_disabled);
        let mut disabled = state.skylight_display_ids.len();

        let committed = finalize_skylight();
        if !committed {
            failed += disabled;
            state.skylight_display_ids.clear();
            disabled = 0;
        }
        Ok(ExternalDisplayDisableResult { state, disabled, failed })
    }
}

fn capture_disabled_external_displays(
    displays: &[u32],
    already_disabled: &ExternalDisplayState,
) -> (ExternalDisplayState, usize) {
    let mut skylight_display_ids = SmallVec::new();
    let mut failed = 0;
    for display_id in external_display_ids(displays) {
        if already_disabled.skylight_display_ids.contains(&display_id) {
            continue;
        }
        if disable_skylight_display(display_id) {
            skylight_display_ids.push(display_id);
        } else {
            failed += 1;
        }
    }

    (ExternalDisplayState { skylight_display_ids }, failed)
}

fn external_display_ids(displays: &[u32]) -> impl Iterator<Item = u32> + '_ {
    displays.iter().copied().filter(|&display_id| !is_builtin(display_id))
}
