use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use lidoff_display::{
    Cursor, CursorError, ExternalDisplayDisableResult, ExternalDisplayError, ExternalDisplays,
    InternalDisplay, InternalDisplayError, InternalDisplayState, KeyboardBacklight,
    KeyboardBacklightError, KeyboardBacklightState,
};
use lidoff_power::{Caffeinate, CaffeinateError};

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
    if matches!(action, MonitorAction::StartPartialDim | MonitorAction::ResumePartialDim)
        && lock_state(shared_state).system_sleeping
    {
        return;
    }
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
    unlock_cursor();
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
    unlock_cursor();
    restore_external_display_state(shared_state);
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
        logging::debug!("restoring keyboard backlight to {:.2}", saved_state.brightness);
    }

    let mut keyboard = KeyboardBacklight::new();
    if keyboard.restore_state(&saved_state).is_ok() {
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

    let mut caffeinate = Caffeinate::new();
    let active = match caffeinate.start() {
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

    let mut caffeinate = Caffeinate::new();
    let inactive = match caffeinate.stop() {
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
    let Some(saved_state) = lock_state(shared_state).external_display_state.take() else {
        return false;
    };

    let mut external = ExternalDisplays::new();
    match external.restore_state(&saved_state) {
        Ok(()) => {
            logging::debug!("restored external displays");
            true
        }
        Err(error) => {
            logging::error!("external display restore incomplete: {error}");
            lock_state(shared_state).external_display_state = Some(saved_state);
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
        logging::debug!("{action} to {:.2}", saved_state.brightness);
    }

    let mut internal = InternalDisplay::new();
    if internal.restore_state(&saved_state).is_ok() {
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
    let mut external = ExternalDisplays::new();
    let disable_result = match external.capture_and_disable() {
        Ok(result) => result,
        Err(ExternalDisplayError::AlreadyDisabled) => return true,
        Err(error) => {
            logging::error!("external display disable failed: {error}");
            return false;
        }
    };

    let ExternalDisplayDisableResult { state: saved_state, disabled, failed } = disable_result;

    {
        let mut state = lock_state(shared_state);
        if let Some(pending) = state.external_display_state.as_mut() {
            for id in saved_state.skylight_display_ids {
                if !pending.skylight_display_ids.contains(&id) {
                    pending.skylight_display_ids.push(id);
                }
            }
        } else {
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

    true
}

fn resume_partial_dim(shared_state: &SharedMonitorState, recovery_cache_dir: &Path) {
    let mut changed = false;
    let mut internal = InternalDisplay::new();
    if !internal.is_disabled() {
        if matches!(internal.disable(), Err(InternalDisplayError::BrightnessFailed)) {
            clear_pending_display_state(shared_state);
            persist_recovery_state(shared_state, recovery_cache_dir);
            logging::error!("failed to dim display");
            return;
        }

        logging::debug!("dimming display to 0.0");
        changed = true;
    }

    if disable_keyboard_backlight(shared_state) {
        changed = true;
    }

    if lock_cursor() {
        changed = true;
    }

    let mut external = ExternalDisplays::new();
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
    let mut internal = InternalDisplay::new();
    let current_state = match internal.get_state() {
        Ok(state) => state,
        Err(error) => {
            logging::error!("failed to read brightness: {error}");
            return;
        }
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

    if matches!(
        internal.disable_from_state(current_state),
        Err(InternalDisplayError::BrightnessFailed)
    ) {
        clear_pending_display_state(shared_state);
        persist_recovery_state(shared_state, recovery_cache_dir);
        logging::error!("failed to dim display");
        return;
    }

    logging::debug!("dimming display to 0.0");

    capture_and_disable_keyboard_backlight_state(shared_state);
    capture_and_disable_external_display_state(shared_state);
    start_caffeinate(shared_state);
    persist_recovery_state(shared_state, recovery_cache_dir);
    lock_cursor();
}

fn clear_pending_display_state(shared_state: &SharedMonitorState) {
    let mut state = lock_state(shared_state);
    state.internal_display_state = None;
    state.external_display_state = None;
    state.keyboard_backlight_state = None;
}

fn capture_and_disable_keyboard_backlight_state(shared_state: &SharedMonitorState) -> bool {
    let mut keyboard = KeyboardBacklight::new();
    if lock_state(shared_state).keyboard_backlight_state.is_none() {
        let current_state = match keyboard.get_state() {
            Ok(state) => state,
            Err(error) => {
                logging::error!("failed to read keyboard backlight: {error}");
                return false;
            }
        };

        let mut state = lock_state(shared_state);
        if state.keyboard_backlight_state.is_none() {
            state.keyboard_backlight_state =
                Some(KeyboardBacklightState { brightness: current_state.brightness });
        }

        return handle_keyboard_backlight_disable_result(
            shared_state,
            keyboard.disable_from_state(current_state),
        );
    }

    disable_keyboard_backlight(shared_state)
}

fn disable_keyboard_backlight(shared_state: &SharedMonitorState) -> bool {
    let mut keyboard = KeyboardBacklight::new();
    handle_keyboard_backlight_disable_result(shared_state, keyboard.disable())
}

fn handle_keyboard_backlight_disable_result(
    shared_state: &SharedMonitorState,
    result: Result<(), KeyboardBacklightError>,
) -> bool {
    match result {
        Ok(()) => {
            logging::debug!("dimming keyboard backlight to 0.0");
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

fn lock_cursor() -> bool {
    let mut cursor = Cursor::new();
    match cursor.lock() {
        Ok(()) => {
            logging::debug!("locked cursor movement");
            true
        }
        Err(CursorError::AlreadyLocked) => false,
        Err(error) => {
            logging::error!("failed to lock cursor movement: {error}");
            false
        }
    }
}

fn unlock_cursor() -> bool {
    let mut cursor = Cursor::new();
    match cursor.unlock() {
        Ok(()) => {
            logging::debug!("unlocked cursor movement");
            true
        }
        Err(CursorError::AlreadyUnlocked) => false,
        Err(error) => {
            logging::error!("failed to unlock cursor movement: {error}");
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
        logging::debug!(
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::super::state::MonitorState;
    use super::{
        MonitorAction, brightness_snapshot_for_dim,
        capture_and_disable_external_display_state, execute_monitor_action,
    };

    static BRIGHTNESS_CALLS: AtomicUsize = AtomicUsize::new(0);
    static EXTERNAL_RESTORE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static FAIL_EXTERNAL_RESTORE: AtomicBool = AtomicBool::new(true);

    #[unsafe(export_name = "DisplaysListOnline")]
    extern "C" fn displays_list_online(
        display_ids: *mut u32,
        capacity: usize,
        count_out: *mut usize,
    ) -> u8 {
        assert!(capacity >= 2);
        unsafe {
            *display_ids = 1;
            *display_ids.add(1) = 2;
            *count_out = 2;
        }
        1
    }

    #[unsafe(export_name = "DisplayIsBuiltin")]
    extern "C" fn display_is_builtin(display_id: u32) -> u8 {
        u8::from(display_id == 1)
    }

    #[unsafe(export_name = "BrightnessGet")]
    extern "C" fn brightness_get() -> f32 {
        BRIGHTNESS_CALLS.fetch_add(1, Ordering::Relaxed);
        -1.0
    }

    #[unsafe(export_name = "BrightnessSet")]
    extern "C" fn brightness_set(_brightness: f32) -> u8 {
        BRIGHTNESS_CALLS.fetch_add(1, Ordering::Relaxed);
        0
    }

    #[unsafe(export_name = "CursorIsLocked")]
    extern "C" fn cursor_is_locked() -> u8 {
        0
    }

    #[unsafe(export_name = "CursorLock")]
    extern "C" fn cursor_lock() -> u8 {
        1
    }

    #[unsafe(export_name = "CursorUnlock")]
    extern "C" fn cursor_unlock() -> u8 {
        1
    }

    #[unsafe(export_name = "SkylightFinalize")]
    extern "C" fn skylight_finalize() -> u8 {
        1
    }

    #[unsafe(export_name = "SkylightDisableDisplay")]
    extern "C" fn skylight_disable_display(_display_id: u32) -> u8 {
        1
    }

    #[unsafe(export_name = "SkylightRestoreFromState")]
    extern "C" fn skylight_restore_from_state(
        _display_ids: *const u32,
        count: usize,
    ) -> usize {
        EXTERNAL_RESTORE_CALLS.fetch_add(1, Ordering::Relaxed);
        if FAIL_EXTERNAL_RESTORE.load(Ordering::Relaxed) { count - 1 } else { count }
    }

    #[test]
    fn fully_closed_restores_external_displays_and_retries_failures() {
        use super::super::transitions::handle_fully_closed_locked;
        use lidoff_display::ExternalDisplayState;

        let shared_state = Arc::new(Mutex::new(MonitorState::new()));
        shared_state.lock().unwrap().external_display_state =
            Some(ExternalDisplayState { skylight_display_ids: vec![3, 6].into() });
        assert!(capture_and_disable_external_display_state(&shared_state));
        let saved_state = ExternalDisplayState { skylight_display_ids: vec![3, 6, 2].into() };
        assert_eq!(
            shared_state.lock().unwrap().external_display_state,
            Some(saved_state.clone())
        );
        let cache_dir = std::env::temp_dir()
            .join(format!("lidoff-external-sleep-test-{}", std::process::id()));

        execute_monitor_action(
            &shared_state,
            MonitorAction::PrepareDisplayStateForSleep { log_restore: false },
            &cache_dir,
        );
        assert_eq!(EXTERNAL_RESTORE_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(shared_state.lock().unwrap().external_display_state, Some(saved_state));

        FAIL_EXTERNAL_RESTORE.store(false, Ordering::Relaxed);
        let action = handle_fully_closed_locked(
            &mut shared_state.lock().unwrap(),
            std::time::Instant::now(),
            false,
        );
        assert!(matches!(action, MonitorAction::PrepareDisplayStateForSleep { .. }));
        execute_monitor_action(&shared_state, action, &cache_dir);

        assert_eq!(EXTERNAL_RESTORE_CALLS.load(Ordering::Relaxed), 2);
        assert!(shared_state.lock().unwrap().external_display_state.is_none());
        assert!(!cache_dir.join("state.bin").exists());
        std::fs::remove_dir_all(cache_dir).unwrap();
    }

    #[test]
    fn queued_dimming_does_not_run_while_sleeping() {
        let shared_state = Arc::new(Mutex::new(MonitorState::new()));
        shared_state.lock().unwrap().system_sleeping = true;
        let cache_dir =
            std::env::temp_dir().join(format!("lidoff-sleep-test-{}", std::process::id()));

        for action in [MonitorAction::StartPartialDim, MonitorAction::ResumePartialDim] {
            execute_monitor_action(&shared_state, action, &cache_dir);
        }

        assert_eq!(BRIGHTNESS_CALLS.load(Ordering::Relaxed), 0);
        assert!(!shared_state.lock().unwrap().caffeinate_active);
        assert!(!cache_dir.exists());
    }

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
