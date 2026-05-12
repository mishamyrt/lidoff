use std::sync::{Arc, Mutex};
use std::time::Instant;

use lidoff_display::{ExternalDisplayState, InternalDisplayState, KeyboardBacklightState};

pub(super) type SharedMonitorState = Arc<Mutex<MonitorState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LidState {
    FullyClosed,
    PartiallyClosed,
    Open,
}

#[derive(Debug)]
pub(super) struct MonitorState {
    pub(super) last_nonzero_brightness: f32,
    pub(super) internal_display_state: Option<InternalDisplayState>,
    pub(super) external_display_state: Option<ExternalDisplayState>,
    pub(super) keyboard_backlight_state: Option<KeyboardBacklightState>,
    pub(super) caffeinate_active: bool,
    pub(super) last_angle: Option<u32>,
    pub(super) last_lid_state: Option<LidState>,
    pub(super) below_threshold_streak: i32,
    pub(super) last_full_close_at: Option<Instant>,
    pub(super) last_open_at: Option<Instant>,
    pub(super) last_wake_at: Option<Instant>,
    pub(super) awaiting_open_after_full_close: bool,
    pub(super) keep_internal_restore_until: Option<Instant>,
    pub(super) system_sleeping: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MonitorAction {
    None,
    RestoreDisplayState { log_restore: bool, clear_internal_after_restore: bool },
    PrepareDisplayStateForSleep { log_restore: bool },
    ResumePartialDim,
    StartPartialDim,
}

impl MonitorState {
    pub(super) fn new() -> Self {
        Self {
            last_nonzero_brightness: -1.0,
            internal_display_state: None,
            external_display_state: None,
            keyboard_backlight_state: None,
            caffeinate_active: false,
            last_angle: None,
            last_lid_state: None,
            below_threshold_streak: 0,
            last_full_close_at: None,
            last_open_at: None,
            last_wake_at: None,
            awaiting_open_after_full_close: false,
            keep_internal_restore_until: None,
            system_sleeping: false,
        }
    }
}

pub(super) fn lock_state(
    shared_state: &SharedMonitorState,
) -> std::sync::MutexGuard<'_, MonitorState> {
    match shared_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
