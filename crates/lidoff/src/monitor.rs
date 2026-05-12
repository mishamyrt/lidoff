use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lidoff_display::{
    DisplayController, ExternalDisplayError, ExternalDisplayState, ExternalDisplays,
    InternalDisplay, InternalDisplayError, InternalDisplayState,
};
use lidoff_power::{CaffeinateError, PowerObserver, caffeinate_start, caffeinate_stop};

use crate::logging;
use crate::recovery_state::{self, RecoveryStateData};

pub const MONITOR_DEFAULT_THRESHOLD: i32 = 30;
pub const MONITOR_DEFAULT_INTERVAL_MS: i32 = 300;
pub const MONITOR_FULL_CLOSE_ANGLE: i32 = 10;
pub const MONITOR_PARTIAL_STABILITY_SAMPLES: i32 = 2;
pub const MONITOR_POST_CLOSE_GRACE_SECONDS: f64 = 1.0;
pub const MONITOR_POST_WAKE_GRACE_SECONDS: f64 = 1.0;

static POWER_STATE: OnceLock<Arc<Mutex<MonitorState>>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LidState {
    FullyClosed,
    PartiallyClosed,
    Open,
}

#[derive(Clone, Copy, Debug)]
pub struct MonitorConfig {
    pub threshold: i32,
    pub interval_ms: i32,
}

#[derive(Debug)]
struct MonitorState {
    last_nonzero_brightness: f32,
    internal_display_state: Option<InternalDisplayState>,
    external_display_state: Option<ExternalDisplayState>,
    caffeinate_active: bool,
    last_angle: i32,
    below_threshold_streak: i32,
    last_full_close_at: f64,
    last_wake_at: f64,
    system_sleeping: bool,
}

impl MonitorState {
    fn new() -> Self {
        Self {
            last_nonzero_brightness: -1.0,
            internal_display_state: None,
            external_display_state: None,
            caffeinate_active: false,
            last_angle: -1,
            below_threshold_streak: 0,
            last_full_close_at: 0.0,
            last_wake_at: 0.0,
            system_sleeping: false,
        }
    }
}

pub fn run(config: &MonitorConfig, should_run: &AtomicBool) {
    let shared_state = Arc::new(Mutex::new(MonitorState::new()));
    let _ = POWER_STATE.set(shared_state.clone());

    {
        let mut state = lock_state(&shared_state);
        recover_state_if_needed(&mut state);
    }

    let mut observer = PowerObserver::new();
    if let Err(e) = observer.start(handle_will_sleep, handle_did_wake) {
        logging::error(format!("failed to start power observer: {}", e));
        return;
    }

    let interval = Duration::from_millis(config.interval_ms as u64);

    while should_run.load(Ordering::Relaxed) {
        let angle = match lidoff_lidsensor::get_angle() {
            Ok(angle) => angle,
            Err(e) => {
                logging::error(format!("failed to get lid angle: {}", e));
                thread::sleep(interval);
                continue;
            }
        };

        logging::debug(format!("angle {angle}°"));
        let now = current_time_seconds();

        {
            let mut state = lock_state(&shared_state);
            if !state.system_sleeping {
                match lid_state_for_angle(angle, config.threshold) {
                    LidState::FullyClosed => handle_fully_closed_locked(&mut state, now),
                    LidState::PartiallyClosed => {
                        handle_partially_closed_locked(&mut state, angle, now)
                    }
                    LidState::Open => handle_open_locked(&mut state),
                }
                state.last_angle = angle;
            }
        }

        thread::sleep(interval);
    }

    let mut state = lock_state(&shared_state);
    restore_display_state_locked(&mut state, false);
    persist_recovery_state_locked(&state);
}

fn lid_state_for_angle(angle: i32, threshold: i32) -> LidState {
    if angle < MONITOR_FULL_CLOSE_ANGLE {
        return LidState::FullyClosed;
    }
    if angle < threshold {
        return LidState::PartiallyClosed;
    }
    LidState::Open
}

fn persist_recovery_state_locked(state: &MonitorState) {
    let recovery_state = RecoveryStateData {
        internal_display_state: state.internal_display_state,
        external_display_state: state.external_display_state.clone(),
    };

    if recovery_state.internal_display_state.is_some()
        || recovery_state.external_display_state.is_some()
    {
        if !recovery_state::save(&recovery_state) {
            logging::error("failed to persist recovery state");
        }
    } else {
        recovery_state::clear();
    }
}

fn recover_state_if_needed(state: &mut MonitorState) {
    let Some(recovery_state) = recovery_state::load() else {
        return;
    };

    logging::info("recovery state detected, attempting restore");
    state.internal_display_state = recovery_state.internal_display_state;
    state.external_display_state = recovery_state.external_display_state;
    restore_display_state_locked(state, true);
    persist_recovery_state_locked(state);
}

fn restore_display_state_locked(state: &mut MonitorState, log_restore: bool) {
    restore_external_display_state_locked(state);
    restore_internal_display_state_locked(state, log_restore);
    stop_caffeinate_locked(state);
}

fn prepare_display_state_for_sleep_locked(state: &mut MonitorState, log_restore: bool) {
    apply_internal_display_state_locked(
        state,
        false,
        log_restore,
        "preparing sleep brightness",
        "failed to prepare brightness before sleep",
    );
    stop_caffeinate_locked(state);
}

fn start_caffeinate_locked(state: &mut MonitorState) {
    if state.caffeinate_active {
        return;
    }

    match caffeinate_start() {
        Ok(()) | Err(CaffeinateError::AlreadyActive) => {
            state.caffeinate_active = true;
        }
        Err(error) => logging::error(format!("failed to start caffeinate session: {}", error)),
    }
}

fn stop_caffeinate_locked(state: &mut MonitorState) {
    if !state.caffeinate_active {
        return;
    }

    match caffeinate_stop() {
        Ok(()) | Err(CaffeinateError::NotActive) => {
            state.caffeinate_active = false;
        }
        Err(error) => logging::error(format!("failed to stop caffeinate session: {}", error)),
    }
}

fn restore_external_display_state_locked(state: &mut MonitorState) {
    let Some(saved_state) = state.external_display_state.clone() else {
        return;
    };

    let external = ExternalDisplays;
    match external.restore_state(saved_state) {
        Ok(()) => {
            logging::info("restored external displays");
            state.external_display_state = None;
        }
        Err(error) => logging::error(format!("external display restore incomplete: {error}")),
    }
}

fn restore_internal_display_state_locked(state: &mut MonitorState, log_restore: bool) {
    apply_internal_display_state_locked(
        state,
        true,
        log_restore,
        "restoring brightness",
        "failed to restore brightness",
    );
}

fn apply_internal_display_state_locked(
    state: &mut MonitorState,
    clear_after_restore: bool,
    log_restore: bool,
    action: &str,
    error: &str,
) {
    let Some(saved_state) = state.internal_display_state else {
        return;
    };

    if log_restore {
        logging::info(format!("{action} to {:.2}", saved_state.brightness));
    }

    let internal = InternalDisplay;
    if internal.restore_state(saved_state).is_ok() {
        state.last_nonzero_brightness = saved_state.brightness;
        if clear_after_restore {
            state.internal_display_state = None;
        }
    } else if log_restore {
        logging::error(error);
    }
}

fn capture_and_disable_external_display_state_locked(state: &mut MonitorState) -> bool {
    let external = ExternalDisplays;
    if state.external_display_state.is_none() {
        let Some(saved_state) = external.get_state() else {
            logging::error("failed to capture external display state");
            return false;
        };

        state.external_display_state = Some(saved_state);
    }

    match external.disable() {
        Ok(()) | Err(ExternalDisplayError::AlreadyDisabled) => true,
        Err(error) => {
            logging::error(format!("external display disable failed: {error}"));
            false
        }
    }
}

fn handle_fully_closed_locked(state: &mut MonitorState, now: f64) {
    state.last_full_close_at = now;
    state.below_threshold_streak = 0;

    prepare_display_state_for_sleep_locked(state, true);
    persist_recovery_state_locked(state);
}

fn handle_partially_closed_locked(state: &mut MonitorState, angle: i32, now: f64) {
    let since_close = if state.last_full_close_at > 0.0 {
        now - state.last_full_close_at
    } else {
        MONITOR_POST_CLOSE_GRACE_SECONDS
    };
    let since_wake = if state.last_wake_at > 0.0 {
        now - state.last_wake_at
    } else {
        MONITOR_POST_WAKE_GRACE_SECONDS
    };
    let grace_active = since_close < MONITOR_POST_CLOSE_GRACE_SECONDS
        || since_wake < MONITOR_POST_WAKE_GRACE_SECONDS;

    if state.internal_display_state.is_some() {
        state.below_threshold_streak = 0;
        let mut changed = false;
        let internal = InternalDisplay;
        if !internal.is_disabled() {
            if matches!(internal.disable(), Err(InternalDisplayError::BrightnessFailed)) {
                state.internal_display_state = None;
                state.external_display_state = None;
                persist_recovery_state_locked(state);
                logging::error("failed to dim display");
                return;
            }

            logging::info("dimming display to 0.0");
            changed = true;
        }

        let external = ExternalDisplays;
        if state.external_display_state.is_none()
            && !external.is_disabled()
            && capture_and_disable_external_display_state_locked(state)
        {
            changed = true;
        }

        if !state.caffeinate_active {
            start_caffeinate_locked(state);
            changed = true;
        }

        if changed {
            persist_recovery_state_locked(state);
        }
        return;
    }

    if grace_active {
        state.below_threshold_streak = 0;
        return;
    }

    let not_opening = if state.last_angle == -1 { true } else { angle <= state.last_angle };
    if not_opening {
        state.below_threshold_streak += 1;
    } else {
        state.below_threshold_streak = 0;
    }

    if state.below_threshold_streak >= MONITOR_PARTIAL_STABILITY_SAMPLES {
        let internal = InternalDisplay;
        let Some(current_state) = internal.get_state() else {
            logging::error("failed to read brightness");
            return;
        };

        let brightness_to_restore =
            brightness_snapshot_for_dim(state, current_state.brightness);
        state.internal_display_state =
            Some(InternalDisplayState { brightness: brightness_to_restore });

        if matches!(internal.disable(), Err(InternalDisplayError::BrightnessFailed)) {
            state.internal_display_state = None;
            state.external_display_state = None;
            persist_recovery_state_locked(state);
            logging::error("failed to dim display");
            return;
        }

        logging::info("dimming display to 0.0");

        if capture_and_disable_external_display_state_locked(state) {
            persist_recovery_state_locked(state);
        }

        start_caffeinate_locked(state);

        persist_recovery_state_locked(state);
    }
}

fn brightness_snapshot_for_dim(state: &mut MonitorState, current_brightness: f32) -> f32 {
    if current_brightness > 0.0 {
        state.last_nonzero_brightness = current_brightness;
        return current_brightness;
    }

    if state.last_nonzero_brightness > 0.0 {
        logging::info(format!(
            "brightness is {:.2}; using last known value {:.2} for restore",
            current_brightness, state.last_nonzero_brightness
        ));
        return state.last_nonzero_brightness;
    }

    current_brightness
}

fn handle_open_locked(state: &mut MonitorState) {
    state.below_threshold_streak = 0;
    restore_display_state_locked(state, true);
    persist_recovery_state_locked(state);
}

extern "C" fn handle_will_sleep(_context: *mut std::ffi::c_void) {
    let Some(shared_state) = POWER_STATE.get() else {
        return;
    };

    let now = current_time_seconds();
    let mut state = lock_state(shared_state);
    state.system_sleeping = true;
    state.last_full_close_at = now;
    state.last_wake_at = 0.0;
    state.last_angle = -1;
    state.below_threshold_streak = 0;
    prepare_display_state_for_sleep_locked(&mut state, false);
    persist_recovery_state_locked(&state);
}

extern "C" fn handle_did_wake(_context: *mut std::ffi::c_void) {
    let Some(shared_state) = POWER_STATE.get() else {
        return;
    };

    let now = current_time_seconds();
    let mut state = lock_state(shared_state);
    state.system_sleeping = false;
    state.last_wake_at = now;
    state.last_full_close_at = now;
    state.last_angle = -1;
    state.below_threshold_streak = 0;
}

fn lock_state(
    shared_state: &Arc<Mutex<MonitorState>>,
) -> std::sync::MutexGuard<'_, MonitorState> {
    match shared_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn current_time_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::{
        LidState, MONITOR_DEFAULT_THRESHOLD, MONITOR_FULL_CLOSE_ANGLE, MonitorState,
        brightness_snapshot_for_dim, lid_state_for_angle,
    };

    #[test]
    fn lid_state_tracks_ranges() {
        assert_eq!(
            lid_state_for_angle(MONITOR_FULL_CLOSE_ANGLE - 1, MONITOR_DEFAULT_THRESHOLD),
            LidState::FullyClosed
        );
        assert_eq!(
            lid_state_for_angle(MONITOR_DEFAULT_THRESHOLD - 1, MONITOR_DEFAULT_THRESHOLD),
            LidState::PartiallyClosed
        );
        assert_eq!(
            lid_state_for_angle(MONITOR_DEFAULT_THRESHOLD, MONITOR_DEFAULT_THRESHOLD),
            LidState::Open
        );
    }

    #[test]
    fn dimming_uses_current_nonzero_brightness_for_restore() {
        let mut state = MonitorState::new();
        assert_eq!(brightness_snapshot_for_dim(&mut state, 0.42), 0.42);
        assert_eq!(state.last_nonzero_brightness, 0.42);
    }

    #[test]
    fn dimming_reuses_last_nonzero_brightness_when_current_is_zero() {
        let mut state = MonitorState::new();
        state.last_nonzero_brightness = 0.64;

        assert_eq!(brightness_snapshot_for_dim(&mut state, 0.0), 0.64);
        assert_eq!(state.last_nonzero_brightness, 0.64);
    }
}
