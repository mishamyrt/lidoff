mod logging;
mod monitor;
mod recovery_state;

use std::ffi::c_int;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub use monitor::{
    MONITOR_DEFAULT_INTERVAL_MS, MONITOR_DEFAULT_THRESHOLD, MONITOR_FULL_CLOSE_ANGLE,
};

static SHOULD_RUN: AtomicBool = AtomicBool::new(true);

const SIGHUP: c_int = 1;
const SIGINT: c_int = 2;
const SIGQUIT: c_int = 3;
const SIGTERM: c_int = 15;

unsafe extern "C" {
    fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> usize;
}

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub threshold: u32,
    pub interval_ms: u64,
    pub verbose: bool,
    pub recovery_cache_dir: PathBuf,
}

pub fn run(config: &DaemonConfig) -> bool {
    logging::set_verbose(config.verbose);
    install_signal_handlers();

    let Ok(mut lid_sensor) = lidoff_lidsensor::LidSensor::new() else {
        logging::error!("failed to initialize lid sensor");
        return false;
    };

    let monitor_config = monitor::MonitorConfig {
        threshold: config.threshold,
        interval_ms: config.interval_ms,
        recovery_cache_dir: config.recovery_cache_dir.clone(),
    };
    monitor::run(&monitor_config, &SHOULD_RUN, &mut lid_sensor);
    true
}

fn install_signal_handlers() {
    unsafe {
        signal(SIGINT, signal_handler);
        signal(SIGTERM, signal_handler);
        signal(SIGHUP, signal_handler);
        signal(SIGQUIT, signal_handler);
    }
}

extern "C" fn signal_handler(_signal: c_int) {
    SHOULD_RUN.store(false, Ordering::Relaxed);
}
