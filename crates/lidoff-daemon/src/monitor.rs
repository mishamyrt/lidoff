use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use lidoff_display::{
    DisplayController, ExternalDisplayDisableResult, ExternalDisplayError,
    ExternalDisplayState, ExternalDisplays, InternalDisplay, InternalDisplayError,
    InternalDisplayState,
};
use lidoff_power::{CaffeinateError, PowerObserver, caffeinate_start, caffeinate_stop};

use crate::logging;
use crate::recovery_state::{self, RecoveryStateData};

pub const MONITOR_DEFAULT_INTERVAL_MS: u64 = 300;
pub const MONITOR_DEFAULT_THRESHOLD: u32 = 30;
pub const MONITOR_FULL_CLOSE_ANGLE: u32 = 5;

const MONITOR_PARTIAL_STABILITY_SAMPLES: i32 = 2;
const MONITOR_POST_CLOSE_GRACE_SECONDS: f64 = 1.0;
const MONITOR_POST_OPEN_GRACE_SECONDS: f64 = 1.0;
const MONITOR_POST_OPEN_RESTORE_SECONDS: f64 = 2.0;
const MONITOR_POST_WAKE_GRACE_SECONDS: f64 = 1.0;

static POWER_STATE: OnceLock<Arc<Mutex<MonitorState>>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LidState {
    FullyClosed,
    PartiallyClosed,
    Open,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MonitorConfig {
    pub threshold: u32,
    pub interval_ms: u64,
}

#[derive(Debug)]
struct MonitorState {
    last_nonzero_brightness: f32,
    internal_display_state: Option<InternalDisplayState>,
    external_display_state: Option<ExternalDisplayState>,
    caffeinate_active: bool,
    last_angle: Option<u32>,
    last_lid_state: Option<LidState>,
    below_threshold_streak: i32,
    last_full_close_at: Option<Instant>,
    last_open_at: Option<Instant>,
    last_wake_at: Option<Instant>,
    awaiting_open_after_full_close: bool,
    keep_internal_restore_until: Option<Instant>,
    system_sleeping: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MonitorAction {
    None,
    RestoreDisplayState { log_restore: bool, clear_internal_after_restore: bool },
    PrepareDisplayStateForSleep { log_restore: bool },
    ResumePartialDim,
    StartPartialDim,
}

impl MonitorState {
    fn new() -> Self {
        Self {
            last_nonzero_brightness: -1.0,
            internal_display_state: None,
            external_display_state: None,
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

pub(crate) fn run(config: &MonitorConfig, should_run: &AtomicBool) {
    let shared_state = Arc::new(Mutex::new(MonitorState::new()));
    let _ = POWER_STATE.set(shared_state.clone());

    recover_state_if_needed(&shared_state);

    let mut observer = PowerObserver::new();
    if let Err(e) = observer.start(handle_will_sleep, handle_did_wake) {
        logging::error!("failed to start power observer: {e}");
        return;
    }

    let interval = Duration::from_millis(config.interval_ms);

    while should_run.load(Ordering::Relaxed) {
        let angle = match lidoff_lidsensor::get_angle() {
            Ok(angle) => angle,
            Err(e) => {
                logging::error!("failed to get lid angle: {e}");
                thread::sleep(interval);
                continue;
            }
        };

        logging::debug!("angle {angle}°");
        let now = Instant::now();

        let action = {
            let mut state = lock_state(&shared_state);
            if state.system_sleeping {
                MonitorAction::None
            } else {
                let lid_state = lid_state_for_angle(angle, config.threshold);
                let state_changed = state.last_lid_state != Some(lid_state);
                let action = match lid_state {
                    LidState::FullyClosed => {
                        handle_fully_closed_locked(&mut state, now, state_changed)
                    }
                    LidState::PartiallyClosed => {
                        handle_partially_closed_locked(&mut state, angle, now)
                    }
                    LidState::Open => handle_open_locked(&mut state, now, state_changed),
                };
                state.last_angle = Some(angle);
                state.last_lid_state = Some(lid_state);
                action
            }
        };
        execute_monitor_action(&shared_state, action);

        thread::sleep(interval);
    }

    restore_display_state(&shared_state, false, true);
    persist_recovery_state(&shared_state);
}

fn lid_state_for_angle(angle: u32, threshold: u32) -> LidState {
    if angle < MONITOR_FULL_CLOSE_ANGLE {
        return LidState::FullyClosed;
    }
    if angle < threshold {
        return LidState::PartiallyClosed;
    }
    LidState::Open
}

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

fn persist_recovery_state(shared_state: &Arc<Mutex<MonitorState>>) {
    let recovery_state = {
        let state = lock_state(shared_state);
        recovery_state_data(&state)
    };
    persist_recovery_state_data(&recovery_state);
}

fn recover_state_if_needed(shared_state: &Arc<Mutex<MonitorState>>) {
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

fn execute_monitor_action(shared_state: &Arc<Mutex<MonitorState>>, action: MonitorAction) {
    match action {
        MonitorAction::None => {}
        MonitorAction::RestoreDisplayState { log_restore, clear_internal_after_restore } => {
            restore_display_state(shared_state, log_restore, clear_internal_after_restore);
            persist_recovery_state(shared_state);
        }
        MonitorAction::PrepareDisplayStateForSleep { log_restore } => {
            prepare_display_state_for_sleep(shared_state, log_restore);
            persist_recovery_state(shared_state);
        }
        MonitorAction::ResumePartialDim => resume_partial_dim(shared_state),
        MonitorAction::StartPartialDim => start_partial_dim(shared_state),
    }
}

fn restore_display_state(
    shared_state: &Arc<Mutex<MonitorState>>,
    log_restore: bool,
    clear_internal_after_restore: bool,
) {
    restore_external_display_state(shared_state);
    restore_internal_display_state(shared_state, log_restore, clear_internal_after_restore);
    stop_caffeinate(shared_state);
}

fn prepare_display_state_for_sleep(
    shared_state: &Arc<Mutex<MonitorState>>,
    log_restore: bool,
) {
    apply_internal_display_state(
        shared_state,
        false,
        log_restore,
        "preparing sleep brightness",
        "failed to prepare brightness before sleep",
    );
    stop_caffeinate(shared_state);
}

fn start_caffeinate(shared_state: &Arc<Mutex<MonitorState>>) -> bool {
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

fn stop_caffeinate(shared_state: &Arc<Mutex<MonitorState>>) -> bool {
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

fn restore_external_display_state(shared_state: &Arc<Mutex<MonitorState>>) -> bool {
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
    shared_state: &Arc<Mutex<MonitorState>>,
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
    shared_state: &Arc<Mutex<MonitorState>>,
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

fn capture_and_disable_external_display_state(
    shared_state: &Arc<Mutex<MonitorState>>,
) -> bool {
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

fn handle_fully_closed_locked(
    state: &mut MonitorState,
    now: Instant,
    state_changed: bool,
) -> MonitorAction {
    state.last_full_close_at = Some(now);
    state.awaiting_open_after_full_close = true;
    state.below_threshold_streak = 0;

    if !should_prepare_fully_closed(state, state_changed) {
        return MonitorAction::None;
    }

    MonitorAction::PrepareDisplayStateForSleep { log_restore: true }
}

fn should_prepare_fully_closed(state: &MonitorState, state_changed: bool) -> bool {
    state_changed || state.caffeinate_active
}

fn handle_partially_closed_locked(
    state: &mut MonitorState,
    angle: u32,
    now: Instant,
) -> MonitorAction {
    if partial_dimming_suppression_reason(state, now).is_some() {
        state.below_threshold_streak = 0;
        return MonitorAction::None;
    }

    if state.internal_display_state.is_some() {
        state.below_threshold_streak = 0;
        return MonitorAction::ResumePartialDim;
    }

    let not_opening =
        if let Some(last_angle) = state.last_angle { angle <= last_angle } else { true };
    if not_opening {
        state.below_threshold_streak += 1;
    } else {
        state.below_threshold_streak = 0;
    }

    if state.below_threshold_streak >= MONITOR_PARTIAL_STABILITY_SAMPLES {
        return MonitorAction::StartPartialDim;
    }

    MonitorAction::None
}

fn resume_partial_dim(shared_state: &Arc<Mutex<MonitorState>>) {
    let mut changed = false;
    let internal = InternalDisplay;
    if !internal.is_disabled() {
        if matches!(internal.disable(), Err(InternalDisplayError::BrightnessFailed)) {
            clear_pending_display_state(shared_state);
            persist_recovery_state(shared_state);
            logging::error!("failed to dim display");
            return;
        }

        logging::info!("dimming display to 0.0");
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
        persist_recovery_state(shared_state);
    }
}

fn start_partial_dim(shared_state: &Arc<Mutex<MonitorState>>) {
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
        persist_recovery_state(shared_state);
        logging::error!("failed to dim display");
        return;
    }

    logging::info!("dimming display to 0.0");

    capture_and_disable_external_display_state(shared_state);
    start_caffeinate(shared_state);
    persist_recovery_state(shared_state);
}

fn clear_pending_display_state(shared_state: &Arc<Mutex<MonitorState>>) {
    let mut state = lock_state(shared_state);
    state.internal_display_state = None;
    state.external_display_state = None;
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
fn partial_grace_active(state: &MonitorState, now: Instant) -> bool {
    partial_dimming_suppression_reason(state, now).is_some()
}

#[cfg(test)]
fn partial_dimming_suppressed(state: &MonitorState, now: Instant) -> bool {
    partial_dimming_suppression_reason(state, now).is_some()
}

fn partial_dimming_suppression_reason(
    state: &MonitorState,
    now: Instant,
) -> Option<&'static str> {
    if state.awaiting_open_after_full_close {
        return Some("awaiting open after full close");
    }
    if since(state.last_full_close_at, now)
        < Duration::from_secs_f64(MONITOR_POST_CLOSE_GRACE_SECONDS)
    {
        return Some("post full-close grace");
    }
    if since(state.last_open_at, now)
        < Duration::from_secs_f64(MONITOR_POST_OPEN_GRACE_SECONDS)
    {
        return Some("post open grace");
    }
    if since(state.last_wake_at, now)
        < Duration::from_secs_f64(MONITOR_POST_WAKE_GRACE_SECONDS)
    {
        return Some("post wake grace");
    }
    None
}

fn since(timestamp: Option<Instant>, now: Instant) -> Duration {
    timestamp.map_or(Duration::MAX, |timestamp| now.saturating_duration_since(timestamp))
}

fn handle_open_locked(
    state: &mut MonitorState,
    now: Instant,
    state_changed: bool,
) -> MonitorAction {
    state.below_threshold_streak = 0;
    let opening_after_full_close = state.awaiting_open_after_full_close;
    state.awaiting_open_after_full_close = false;
    if state_changed {
        state.last_open_at = Some(now);
    }
    maybe_start_open_restore_hold(state, now, opening_after_full_close);

    if !should_restore_on_open(state) {
        return MonitorAction::None;
    }

    let clear_internal_after_restore = should_clear_internal_restore_on_open(state, now);
    if clear_internal_after_restore {
        state.keep_internal_restore_until = None;
    }

    MonitorAction::RestoreDisplayState { log_restore: true, clear_internal_after_restore }
}

fn should_clear_internal_restore_on_open(state: &MonitorState, now: Instant) -> bool {
    state.keep_internal_restore_until.is_none_or(|until| now >= until)
}

fn maybe_start_open_restore_hold(
    state: &mut MonitorState,
    now: Instant,
    opening_after_full_close: bool,
) {
    if opening_after_full_close && state.internal_display_state.is_some() {
        state.keep_internal_restore_until =
            Some(now + Duration::from_secs_f64(MONITOR_POST_OPEN_RESTORE_SECONDS));
    }
}

fn should_restore_on_open(state: &MonitorState) -> bool {
    state.internal_display_state.is_some()
        || state.external_display_state.is_some()
        || state.caffeinate_active
}

extern "C" fn handle_will_sleep(_context: *mut std::ffi::c_void) {
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

extern "C" fn handle_did_wake(_context: *mut std::ffi::c_void) {
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

fn lock_state(
    shared_state: &Arc<Mutex<MonitorState>>,
) -> std::sync::MutexGuard<'_, MonitorState> {
    match shared_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LidState, MONITOR_DEFAULT_THRESHOLD, MONITOR_FULL_CLOSE_ANGLE, MonitorState,
        brightness_snapshot_for_dim, handle_open_locked, lid_state_for_angle,
        maybe_start_open_restore_hold, partial_dimming_suppressed, partial_grace_active,
        should_clear_internal_restore_on_open, should_prepare_fully_closed,
        should_restore_on_open,
    };
    use lidoff_display::InternalDisplayState;
    use std::time::{Duration, Instant};

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

    #[test]
    fn open_without_pending_state_does_not_require_restore() {
        let state = MonitorState::new();

        assert!(!should_restore_on_open(&state));
    }

    #[test]
    fn open_with_pending_state_requires_restore() {
        let mut state = MonitorState::new();
        state.internal_display_state = Some(InternalDisplayState { brightness: 0.42 });

        assert!(should_restore_on_open(&state));
    }

    #[test]
    fn fully_closed_prepare_is_edge_triggered() {
        let state = MonitorState::new();

        assert!(should_prepare_fully_closed(&state, true));
        assert!(!should_prepare_fully_closed(&state, false));
    }

    #[test]
    fn fully_closed_prepare_retries_dirty_caffeinate_state() {
        let mut state = MonitorState::new();
        state.caffeinate_active = true;

        assert!(should_prepare_fully_closed(&state, false));
    }

    #[test]
    fn open_transition_starts_partial_grace_period() {
        let mut state = MonitorState::new();
        let now = Instant::now();

        handle_open_locked(&mut state, now, true);

        assert!(partial_grace_active(&state, now + Duration::from_millis(500)));
        assert!(!partial_grace_active(&state, now + Duration::from_secs(1)));
    }

    #[test]
    fn stable_open_does_not_refresh_partial_grace_period() {
        let mut state = MonitorState::new();
        let now = Instant::now();
        state.last_open_at = now.checked_sub(Duration::from_secs(10));

        handle_open_locked(&mut state, now, false);

        assert!(!partial_grace_active(&state, now));
    }

    #[test]
    fn full_close_suppresses_partial_dimming_until_open() {
        let mut state = MonitorState::new();
        let now = Instant::now();
        state.awaiting_open_after_full_close = true;

        assert!(partial_dimming_suppressed(&state, now));

        handle_open_locked(&mut state, now, true);

        assert!(!state.awaiting_open_after_full_close);
        assert!(partial_dimming_suppressed(&state, now + Duration::from_millis(500)));
        assert!(!partial_dimming_suppressed(&state, now + Duration::from_secs(1)));
    }

    #[test]
    fn opening_after_full_close_keeps_internal_restore_temporarily() {
        let mut state = MonitorState::new();
        let now = Instant::now();
        state.awaiting_open_after_full_close = true;
        state.internal_display_state = Some(InternalDisplayState { brightness: 0.42 });

        maybe_start_open_restore_hold(&mut state, now, true);

        assert!(state.internal_display_state.is_some());
        assert!(!should_clear_internal_restore_on_open(&state, now + Duration::from_secs(1)));
        assert!(should_clear_internal_restore_on_open(&state, now + Duration::from_secs(3)));
    }
}
