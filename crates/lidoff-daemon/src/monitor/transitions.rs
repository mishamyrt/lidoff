use std::time::{Duration, Instant};

use super::state::{LidState, MonitorAction, MonitorState};
use super::{
    MONITOR_FULL_CLOSE_ANGLE, MONITOR_PARTIAL_STABILITY_SAMPLES,
    MONITOR_POST_CLOSE_GRACE_SECONDS, MONITOR_POST_OPEN_GRACE_SECONDS,
    MONITOR_POST_OPEN_RESTORE_SECONDS, MONITOR_POST_WAKE_GRACE_SECONDS,
};

pub(super) fn lid_state_for_angle(angle: u32, threshold: u32) -> LidState {
    if angle < MONITOR_FULL_CLOSE_ANGLE {
        return LidState::FullyClosed;
    }
    if angle < threshold {
        return LidState::PartiallyClosed;
    }
    LidState::Open
}

pub(super) fn handle_fully_closed_locked(
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

pub(super) fn handle_partially_closed_locked(
    state: &mut MonitorState,
    angle: u32,
    now: Instant,
) -> MonitorAction {
    if partial_dimming_suppression_reason(state, now).is_some() {
        state.below_threshold_streak = 0;
        return MonitorAction::None;
    }

    if state.internal_display_state.is_some() || state.keyboard_backlight_state.is_some() {
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

pub(super) fn handle_open_locked(
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
        || state.keyboard_backlight_state.is_some()
        || state.caffeinate_active
}

#[cfg(test)]
mod tests {
    use super::{
        handle_open_locked, lid_state_for_angle, maybe_start_open_restore_hold,
        partial_dimming_suppressed, partial_grace_active,
        should_clear_internal_restore_on_open, should_prepare_fully_closed,
        should_restore_on_open,
    };
    use crate::monitor::{MONITOR_DEFAULT_THRESHOLD, MONITOR_FULL_CLOSE_ANGLE};
    use lidoff_display::{InternalDisplayState, KeyboardBacklightState};
    use std::time::{Duration, Instant};

    use super::super::state::{LidState, MonitorState};

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
    fn open_with_pending_keyboard_backlight_requires_restore() {
        let mut state = MonitorState::new();
        state.keyboard_backlight_state = Some(KeyboardBacklightState { brightness: 0.42 });

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
