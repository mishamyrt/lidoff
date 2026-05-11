use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lidoff_display::{
    self as external_display, ExternalDisplayDisableResult, ExternalDisplayRestoreResult,
};
use lidoff_power::{CaffeinateError, PowerObserver, caffeinate_start, caffeinate_stop};

use crate::logging;
use crate::platform::{self, LID_ANGLE_ERROR};
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
    Unknown,
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
    saved_brightness: f32,
    last_nonzero_brightness: f32,
    brightness_lowered: bool,
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
            saved_brightness: -1.0,
            last_nonzero_brightness: -1.0,
            brightness_lowered: false,
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
        let angle = platform::lid_sensor_get_angle();
        if angle == LID_ANGLE_ERROR {
            thread::sleep(interval);
            continue;
        }

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
                    LidState::Unknown => {}
                }
                state.last_angle = angle;
            }
        }

        thread::sleep(interval);
    }

    let mut state = lock_state(&shared_state);
    let restore_result = external_display::restore();
    log_restore_result(restore_result);
    restore_display_state_locked(&mut state, false);
    persist_recovery_state_locked(&state);
}

fn lid_state_for_angle(angle: i32, threshold: i32) -> LidState {
    if angle == LID_ANGLE_ERROR {
        return LidState::Unknown;
    }
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
        pending_brightness_restore: state.brightness_lowered && state.saved_brightness >= 0.0,
        saved_brightness: state.saved_brightness,
        pending_external_restore: external_display::are_disabled(),
    };

    let external_state = if recovery_state.pending_external_restore {
        external_display::copy_state()
    } else {
        None
    };

    if recovery_state.pending_brightness_restore || recovery_state.pending_external_restore {
        if !recovery_state::save(&recovery_state, external_state.as_ref()) {
            logging::error("failed to persist recovery state");
        }
    } else {
        recovery_state::clear();
    }
}

fn recover_state_if_needed(state: &mut MonitorState) {
    let Some((mut recovery_state, external_state)) = recovery_state::load() else {
        return;
    };

    logging::info("recovery state detected, attempting restore");

    if recovery_state.pending_external_restore {
        let result = match external_state.as_ref() {
            Some(saved_state) => external_display::restore_from_state(saved_state),
            None => external_display::restore(),
        };

        if result.ok && result.restored > 0 {
            logging::info(format!("restored {} external displays", result.restored));
            recovery_state.pending_external_restore = false;
        } else if result.ok && external_state.is_none() {
            logging::info("external display recovery requested with no state");
            recovery_state.pending_external_restore = false;
        } else {
            logging::error("failed to restore external displays during recovery");
        }
    }

    if recovery_state.pending_brightness_restore && recovery_state.saved_brightness >= 0.0 {
        if platform::brightness_set(recovery_state.saved_brightness) {
            logging::info(format!(
                "restored brightness to {:.2}",
                recovery_state.saved_brightness
            ));
            state.last_nonzero_brightness = recovery_state.saved_brightness;
            recovery_state.pending_brightness_restore = false;
            recovery_state.saved_brightness = -1.0;
            state.brightness_lowered = false;
            state.saved_brightness = -1.0;
        } else {
            logging::error("failed to restore brightness during recovery");
        }
    }

    if recovery_state.pending_brightness_restore || recovery_state.pending_external_restore {
        let _ = recovery_state::save(&recovery_state, external_state.as_ref());
    } else {
        recovery_state::clear();
    }
}

fn restore_display_state_locked(state: &mut MonitorState, log_restore: bool) {
    if state.brightness_lowered && state.saved_brightness >= 0.0 {
        if log_restore {
            logging::info(format!(
                "restoring brightness to {:.2}",
                state.saved_brightness
            ));
        }

        if platform::brightness_set(state.saved_brightness) {
            state.last_nonzero_brightness = state.saved_brightness;
            state.brightness_lowered = false;
            state.saved_brightness = -1.0;
        } else if log_restore {
            logging::error("failed to restore brightness");
        }
    }

    stop_caffeinate_locked(state);
}

fn prepare_display_state_for_sleep_locked(state: &mut MonitorState, log_restore: bool) {
    if state.brightness_lowered && state.saved_brightness >= 0.0 {
        if log_restore {
            logging::info(format!(
                "preparing sleep brightness to {:.2}",
                state.saved_brightness
            ));
        }

        if !platform::brightness_set(state.saved_brightness) && log_restore {
            logging::error("failed to prepare brightness before sleep");
        }
    }

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

fn log_disable_result(result: ExternalDisplayDisableResult) {
    if result.already_disabled {
        return;
    }

    if !result.ok {
        logging::error("external display disable failed");
        return;
    }

    if result.failed > 0 {
        logging::error(format!(
            "external display disable failed for {} displays",
            result.failed
        ));
    } else if result.total_external > 0 && result.disabled == 0 {
        logging::info("no external displays were disabled");
    }
}

fn log_restore_result(result: ExternalDisplayRestoreResult) {
    if !result.had_backups {
        return;
    }

    if result.ok {
        if result.restored > 0 {
            logging::info(format!("restored {} external displays", result.restored));
        } else {
            logging::info("external display restore requested with no restored targets");
        }
        return;
    }

    logging::error(format!(
        "external display restore incomplete (restored {})",
        result.restored
    ));
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

    if state.brightness_lowered {
        state.below_threshold_streak = 0;
        if !external_display::are_disabled() {
            let disable_result = external_display::disable();
            log_disable_result(disable_result);
            persist_recovery_state_locked(state);
        }
        return;
    }

    if grace_active {
        state.below_threshold_streak = 0;
        return;
    }

    let not_opening = if state.last_angle == -1 {
        true
    } else {
        angle <= state.last_angle
    };
    if not_opening {
        state.below_threshold_streak += 1;
    } else {
        state.below_threshold_streak = 0;
    }

    if state.below_threshold_streak >= MONITOR_PARTIAL_STABILITY_SAMPLES {
        let current_brightness = platform::brightness_get();
        if current_brightness < 0.0 {
            logging::error("failed to read brightness");
            return;
        }

        let brightness_to_restore = brightness_snapshot_for_dim(state, current_brightness);
        state.saved_brightness = brightness_to_restore;
        state.brightness_lowered = true;
        persist_recovery_state_locked(state);

        if !platform::brightness_set(0.0) {
            state.saved_brightness = -1.0;
            state.brightness_lowered = false;
            persist_recovery_state_locked(state);
            logging::error("failed to dim display");
            return;
        }

        logging::info("dimming display to 0.0");

        let disable_result = external_display::disable();
        log_disable_result(disable_result);

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
    let restore_result = external_display::restore();
    log_restore_result(restore_result);
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

fn lock_state(shared_state: &Arc<Mutex<MonitorState>>) -> std::sync::MutexGuard<'_, MonitorState> {
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
            lid_state_for_angle(-1, MONITOR_DEFAULT_THRESHOLD),
            LidState::Unknown
        );
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
