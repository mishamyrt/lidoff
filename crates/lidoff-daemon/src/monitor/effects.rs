use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use lidoff_display::{
    DisplayController, ExternalDisplayDisableResult, ExternalDisplayError, ExternalDisplays,
    InternalDisplay, InternalDisplayError, InternalDisplayState, KeyboardBacklight,
    KeyboardBacklightError, KeyboardBacklightState,
};
use lidoff_power::{CaffeinateError, caffeinate_start, caffeinate_stop};

use super::persistence::persist_recovery_state;
use super::state::{MonitorAction, MonitorState, SharedMonitorState, lock_state};
use crate::logging;

static EFFECTS_LOCK: Mutex<()> = Mutex::new(());

fn lock_effects() -> MutexGuard<'static, ()> {
    match EFFECTS_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(super) fn execute_monitor_action(
    shared_state: &SharedMonitorState,
    action: MonitorAction,
    recovery_cache_dir: &Path,
) {
    if action == MonitorAction::None {
        return;
    }

    let _effects = lock_effects();
    execute_monitor_action_locked(shared_state, action, recovery_cache_dir);
}

fn execute_monitor_action_locked(
    shared_state: &SharedMonitorState,
    action: MonitorAction,
    recovery_cache_dir: &Path,
) {
    match action {
        MonitorAction::None => {}
        MonitorAction::RestoreDisplayState { log_restore, clear_internal_after_restore } => {
            restore_display_state_locked(
                shared_state,
                log_restore,
                clear_internal_after_restore,
            );
            persist_recovery_state(shared_state, recovery_cache_dir);
        }
        MonitorAction::PrepareDisplayStateForSleep { log_restore } => {
            prepare_display_state_for_sleep_locked(shared_state, log_restore);
            persist_recovery_state(shared_state, recovery_cache_dir);
        }
        MonitorAction::ResumePartialDim => {
            resume_partial_dim(shared_state, recovery_cache_dir);
        }
        MonitorAction::StartPartialDim => {
            start_partial_dim(shared_state, recovery_cache_dir);
        }
    }
}

pub(super) fn restore_display_state(
    shared_state: &SharedMonitorState,
    log_restore: bool,
    clear_internal_after_restore: bool,
) {
    let _effects = lock_effects();
    restore_display_state_locked(shared_state, log_restore, clear_internal_after_restore);
}

fn restore_display_state_locked(
    shared_state: &SharedMonitorState,
    log_restore: bool,
    clear_internal_after_restore: bool,
) {
    restore_external_display_state(shared_state);
    restore_keyboard_backlight_state(shared_state, log_restore, true);
    restore_internal_display_state(shared_state, log_restore, clear_internal_after_restore);
    stop_caffeinate(shared_state);
}

pub(super) fn prepare_display_state_for_sleep(
    shared_state: &SharedMonitorState,
    log_restore: bool,
) {
    let _effects = lock_effects();
    prepare_display_state_for_sleep_locked(shared_state, log_restore);
}

fn prepare_display_state_for_sleep_locked(
    shared_state: &SharedMonitorState,
    log_restore: bool,
) {
    restore_keyboard_backlight_state(shared_state, log_restore, false);
    apply_internal_display_state(
        shared_state,
        false,
        log_restore,
        "preparing sleep brightness",
        "failed to prepare brightness before sleep",
    );
    stop_caffeinate(shared_state);
}

fn restore_keyboard_backlight_state(
    shared_state: &SharedMonitorState,
    log_restore: bool,
    clear_after_restore: bool,
) -> bool {
    let Some(saved_state) = lock_state(shared_state).keyboard_backlight_state else {
        return false;
    };

    if log_restore {
        logging::info!("restoring keyboard backlight to {:.2}", saved_state.brightness);
    }

    let keyboard = KeyboardBacklight;
    if keyboard.restore_state(saved_state).is_ok() {
        let mut state = lock_state(shared_state);
        if state.keyboard_backlight_state == Some(saved_state) && clear_after_restore {
            state.keyboard_backlight_state = None;
        }
        true
    } else {
        if log_restore {
            logging::error!("failed to restore keyboard backlight");
        }
        false
    }
}

fn start_caffeinate(shared_state: &SharedMonitorState) -> bool {
    if lock_state(shared_state).caffeinate_active {
        return false;
    }

    let active = match caffeinate_start() {
        Ok(()) | Err(CaffeinateError::AlreadyActive) => true,
        Err(error) => {
            logging::error!("failed to start caffeinate session: {error}");
            false
        }
    };

    if active {
        lock_state(shared_state).caffeinate_active = true;
    }

    active
}

fn stop_caffeinate(shared_state: &SharedMonitorState) -> bool {
    if !lock_state(shared_state).caffeinate_active {
        return false;
    }

    let inactive = match caffeinate_stop() {
        Ok(()) | Err(CaffeinateError::NotActive) => true,
        Err(error) => {
            logging::error!("failed to stop caffeinate session: {error}");
            false
        }
    };

    if inactive {
        lock_state(shared_state).caffeinate_active = false;
    }

    inactive
}

fn restore_external_display_state(shared_state: &SharedMonitorState) -> bool {
    let Some(saved_state) = lock_state(shared_state).external_display_state.clone() else {
        return false;
    };

    let external = ExternalDisplays;
    match external.restore_state(saved_state.clone()) {
        Ok(()) => {
            logging::info!("restored external displays");
            let mut state = lock_state(shared_state);
            if state.external_display_state.as_ref() == Some(&saved_state) {
                state.external_display_state = None;
            }
            true
        }
        Err(error) => {
            logging::error!("external display restore incomplete: {error}");
            false
        }
    }
}

fn restore_internal_display_state(
    shared_state: &SharedMonitorState,
    log_restore: bool,
    clear_after_restore: bool,
) {
    apply_internal_display_state(
        shared_state,
        clear_after_restore,
        log_restore,
        "restoring brightness",
        "failed to restore brightness",
    );
}

fn apply_internal_display_state(
    shared_state: &SharedMonitorState,
    clear_after_restore: bool,
    log_restore: bool,
    action: &str,
    error: &str,
) -> bool {
    let Some(saved_state) = lock_state(shared_state).internal_display_state else {
        return false;
    };

    if log_restore {
        logging::info!("{action} to {:.2}", saved_state.brightness);
    }

    let internal = InternalDisplay;
    if internal.restore_state(saved_state).is_ok() {
        let mut state = lock_state(shared_state);
        if state.internal_display_state == Some(saved_state) {
            state.last_nonzero_brightness = saved_state.brightness;
            if clear_after_restore {
                state.internal_display_state = None;
            }
        }
        true
    } else {
        if log_restore {
            logging::error!("{error}");
        }
        false
    }
}

fn capture_and_disable_external_display_state(shared_state: &SharedMonitorState) -> bool {
    let external = ExternalDisplays;
    if lock_state(shared_state).external_display_state.is_none() {
        let disable_result = match external.capture_and_disable() {
            Ok(result) => result,
            Err(ExternalDisplayError::AlreadyDisabled) => return true,
            Err(error) => {
                logging::error!("external display disable failed: {error}");
                return false;
            }
        };

        let ExternalDisplayDisableResult { state: saved_state, disabled, failed } =
            disable_result;

        {
            let mut state = lock_state(shared_state);
            if state.external_display_state.is_none() {
                state.external_display_state = Some(saved_state);
            }
        }

        if failed > 0 {
            logging::error!(
                "external display disable failed: {}",
                ExternalDisplayError::DisableFailed { disabled, failed }
            );
            return false;
        }

        return true;
    }

    match external.disable() {
        Ok(()) | Err(ExternalDisplayError::AlreadyDisabled) => true,
        Err(error) => {
            logging::error!("external display disable failed: {error}");
            false
        }
    }
}

fn resume_partial_dim(shared_state: &SharedMonitorState, recovery_cache_dir: &Path) {
    let mut changed = false;
    let internal = InternalDisplay;
    if !internal.is_disabled() {
        if matches!(internal.disable(), Err(InternalDisplayError::BrightnessFailed)) {
            clear_pending_display_state(shared_state);
            persist_recovery_state(shared_state, recovery_cache_dir);
            logging::error!("failed to dim display");
            return;
        }

        logging::info!("dimming display to 0.0");
        changed = true;
    }

    if disable_keyboard_backlight(shared_state) {
        changed = true;
    }

    let external = ExternalDisplays;
    if lock_state(shared_state).external_display_state.is_none()
        && !external.is_disabled()
        && capture_and_disable_external_display_state(shared_state)
    {
        changed = true;
    }

    if start_caffeinate(shared_state) {
        changed = true;
    }

    if changed {
        persist_recovery_state(shared_state, recovery_cache_dir);
    }
}

fn start_partial_dim(shared_state: &SharedMonitorState, recovery_cache_dir: &Path) {
    let internal = InternalDisplay;
    let Some(current_state) = internal.get_state() else {
        logging::error!("failed to read brightness");
        return;
    };

    {
        let mut state = lock_state(shared_state);
        if state.internal_display_state.is_none() {
            let brightness_to_restore =
                brightness_snapshot_for_dim(&mut state, current_state.brightness);
            state.internal_display_state =
                Some(InternalDisplayState { brightness: brightness_to_restore });
        }
    }

    if matches!(internal.disable(), Err(InternalDisplayError::BrightnessFailed)) {
        clear_pending_display_state(shared_state);
        persist_recovery_state(shared_state, recovery_cache_dir);
        logging::error!("failed to dim display");
        return;
    }

    logging::info!("dimming display to 0.0");

    capture_and_disable_keyboard_backlight_state(shared_state);

    capture_and_disable_external_display_state(shared_state);
    start_caffeinate(shared_state);
    persist_recovery_state(shared_state, recovery_cache_dir);
}

fn clear_pending_display_state(shared_state: &SharedMonitorState) {
    let mut state = lock_state(shared_state);
    state.internal_display_state = None;
    state.external_display_state = None;
    state.keyboard_backlight_state = None;
}

fn capture_and_disable_keyboard_backlight_state(shared_state: &SharedMonitorState) -> bool {
    let keyboard = KeyboardBacklight;
    if lock_state(shared_state).keyboard_backlight_state.is_none() {
        let Some(current_state) = keyboard.get_state() else {
            logging::error!("failed to read keyboard backlight");
            return false;
        };

        let mut state = lock_state(shared_state);
        if state.keyboard_backlight_state.is_none() {
            state.keyboard_backlight_state =
                Some(KeyboardBacklightState { brightness: current_state.brightness });
        }
    }

    disable_keyboard_backlight(shared_state)
}

fn disable_keyboard_backlight(shared_state: &SharedMonitorState) -> bool {
    let keyboard = KeyboardBacklight;
    match keyboard.disable() {
        Ok(()) => {
            logging::info!("dimming keyboard backlight to 0.0");
            true
        }
        Err(KeyboardBacklightError::AlreadyDisabled) => false,
        Err(error) => {
            logging::error!("failed to dim keyboard backlight: {error}");
            let mut state = lock_state(shared_state);
            state.keyboard_backlight_state = None;
            false
        }
    }
}

fn brightness_snapshot_for_dim(state: &mut MonitorState, current_brightness: f32) -> f32 {
    if current_brightness > 0.0 {
        state.last_nonzero_brightness = current_brightness;
        return current_brightness;
    }

    if state.last_nonzero_brightness > 0.0 {
        logging::info!(
            "brightness is {:.2}; using last known value {:.2} for restore",
            current_brightness,
            state.last_nonzero_brightness
        );
        return state.last_nonzero_brightness;
    }

    current_brightness
}

#[cfg(test)]
mod tests {
    use super::super::state::MonitorState;
    use super::brightness_snapshot_for_dim;

    #[test]
    #[allow(clippy::float_cmp)]
    fn dimming_uses_current_nonzero_brightness_for_restore() {
        let mut state = MonitorState::new();
        assert_eq!(brightness_snapshot_for_dim(&mut state, 0.42), 0.42);
        assert_eq!(state.last_nonzero_brightness, 0.42);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn dimming_reuses_last_nonzero_brightness_when_current_is_zero() {
        let mut state = MonitorState::new();
        state.last_nonzero_brightness = 0.64;

        assert_eq!(brightness_snapshot_for_dim(&mut state, 0.0), 0.64);
        assert_eq!(state.last_nonzero_brightness, 0.64);
    }
}
