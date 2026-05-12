use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DisplayController,
    coregraphics::{
        capture_skylight_display, clear_skylight_backups, copy_skylight_state,
        disable_skylight_display, finalize_skylight, is_builtin, online_displays,
        prepare_skylight, restore_skylight_state,
    },
};

static EXTERNAL_DISPLAYS_DISABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, Default)]
pub struct ExternalDisplays;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExternalDisplayState {
    pub skylight_display_ids: Vec<u32>,
}

#[derive(Error, Debug)]
pub enum ExternalDisplayError {
    #[error("already disabled")]
    AlreadyDisabled,

    #[error("failed to get online displays")]
    GetOnlineDisplaysFailed,

    #[error("failed to prepare Skylight backend")]
    SkylightPrepareFailed,

    #[error("failed to capture Skylight state")]
    SkylightCaptureFailed,

    #[error("failed to copy Skylight state")]
    SkylightCopyStateFailed,

    #[error("failed to disable {failed} external display(s), disabled {disabled}")]
    DisableFailed { disabled: usize, failed: usize },

    #[error("failed to restore {failed} external display(s), restored {restored}")]
    RestoreFailed { restored: usize, failed: usize },
}

impl DisplayController for ExternalDisplays {
    type State = ExternalDisplayState;
    type Error = ExternalDisplayError;

    fn is_disabled(&self) -> bool {
        EXTERNAL_DISPLAYS_DISABLED.load(Ordering::Relaxed)
    }

    fn disable(&self) -> Result<(), Self::Error> {
        if self.is_disabled() {
            return Err(ExternalDisplayError::AlreadyDisabled);
        }

        let displays =
            online_displays().ok_or(ExternalDisplayError::GetOnlineDisplaysFailed)?;

        if !prepare_skylight(displays.len()) {
            clear_skylight_backups();
            return Err(ExternalDisplayError::SkylightPrepareFailed);
        }

        let mut disabled = 0;
        let mut failed = 0;
        for display_id in external_display_ids(&displays) {
            if disable_skylight_display(display_id) {
                disabled += 1;
            } else {
                failed += 1;
            }
        }

        finalize_skylight();
        EXTERNAL_DISPLAYS_DISABLED.store(disabled > 0, Ordering::Relaxed);

        if failed > 0 {
            Err(ExternalDisplayError::DisableFailed { disabled, failed })
        } else {
            Ok(())
        }
    }

    fn get_state(&self) -> Option<Self::State> {
        let displays = online_displays()?;
        copy_state_with_displays(&displays).ok()
    }

    fn restore_state(&self, state: Self::State) -> Result<(), Self::Error> {
        if state.skylight_display_ids.is_empty() {
            EXTERNAL_DISPLAYS_DISABLED.store(false, Ordering::Relaxed);
            return Ok(());
        }

        let restored = restore_skylight_state(&state.skylight_display_ids);

        EXTERNAL_DISPLAYS_DISABLED.store(false, Ordering::Relaxed);

        if restored < state.skylight_display_ids.len() {
            Err(ExternalDisplayError::RestoreFailed {
                restored,
                failed: state.skylight_display_ids.len() - restored,
            })
        } else {
            Ok(())
        }
    }
}

fn copy_state_with_displays(
    displays: &[u32],
) -> Result<ExternalDisplayState, ExternalDisplayError> {
    if !prepare_skylight(displays.len()) {
        clear_skylight_backups();
        return Err(ExternalDisplayError::SkylightPrepareFailed);
    }

    for display_id in external_display_ids(displays) {
        if !capture_skylight_display(display_id) {
            clear_skylight_backups();
            finalize_skylight();
            return Err(ExternalDisplayError::SkylightCaptureFailed);
        }
    }

    let state = copy_skylight_state()
        .map(|skylight_display_ids| ExternalDisplayState { skylight_display_ids })
        .ok_or(ExternalDisplayError::SkylightCopyStateFailed);

    clear_skylight_backups();
    finalize_skylight();
    state
}

fn external_display_ids(displays: &[u32]) -> impl Iterator<Item = u32> + '_ {
    displays.iter().copied().filter(|&display_id| !is_builtin(display_id))
}
