mod effects;
mod persistence;
mod power_events;
mod state;
mod transitions;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lidoff_lidsensor::LidSensor;
use lidoff_power::PowerObserver;

use self::effects::{execute_monitor_action, restore_display_state};
use self::persistence::{persist_recovery_state, recover_state_if_needed};
use self::power_events::{handle_did_wake, handle_will_sleep, set_power_state};
use self::state::{MonitorAction, MonitorState, lock_state};
use self::transitions::{
    handle_fully_closed_locked, handle_open_locked, handle_partially_closed_locked,
    lid_state_for_angle,
};
use crate::logging;

pub const MONITOR_DEFAULT_INTERVAL_MS: u64 = 300;
pub const MONITOR_DEFAULT_THRESHOLD: u32 = 30;
pub const MONITOR_FULL_CLOSE_ANGLE: u32 = 5;

const MONITOR_PARTIAL_STABILITY_SAMPLES: i32 = 2;
const MONITOR_POST_CLOSE_GRACE_SECONDS: f64 = 1.0;
const MONITOR_POST_OPEN_GRACE_SECONDS: f64 = 1.0;
const MONITOR_POST_OPEN_RESTORE_SECONDS: f64 = 2.0;
const MONITOR_POST_WAKE_GRACE_SECONDS: f64 = 1.0;

#[derive(Clone, Debug)]
pub(crate) struct MonitorConfig {
    pub threshold: u32,
    pub interval_ms: u64,
    pub recovery_cache_dir: PathBuf,
}

pub(crate) fn run(
    config: &MonitorConfig,
    should_run: &AtomicBool,
    lid_sensor: &mut LidSensor,
) {
    let shared_state = Arc::new(Mutex::new(MonitorState::new()));
    set_power_state(shared_state.clone(), config.recovery_cache_dir.clone());

    recover_state_if_needed(&shared_state, &config.recovery_cache_dir);

    let mut observer = PowerObserver::new();
    if let Err(e) = observer.start(handle_will_sleep, handle_did_wake) {
        logging::error!("failed to start power observer: {e}");
        return;
    }

    let interval = Duration::from_millis(config.interval_ms);

    while should_run.load(Ordering::Relaxed) {
        let angle = match lid_sensor.get_angle() {
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
                    state::LidState::FullyClosed => {
                        handle_fully_closed_locked(&mut state, now, state_changed)
                    }
                    state::LidState::PartiallyClosed => {
                        handle_partially_closed_locked(&mut state, angle, now)
                    }
                    state::LidState::Open => {
                        handle_open_locked(&mut state, now, state_changed)
                    }
                };
                state.last_angle = Some(angle);
                state.last_lid_state = Some(lid_state);
                action
            }
        };
        execute_monitor_action(&shared_state, action, &config.recovery_cache_dir);

        thread::sleep(interval);
    }

    restore_display_state(&shared_state, false, true);
    persist_recovery_state(&shared_state, &config.recovery_cache_dir);
}
