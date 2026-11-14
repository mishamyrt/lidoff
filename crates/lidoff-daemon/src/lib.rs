mod logging;
mod monitor;
mod recovery_state;

use std::ffi::c_int;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
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
    let _instance_lock = match lock_instance(&config.recovery_cache_dir) {
        Ok(lock) => lock,
        Err(error) => {
            logging::error!("failed to lock daemon instance (is lidoff running?): {error}");
            return false;
        }
    };
    install_signal_handlers();

    logging::info!(
        "starting lidoff daemon (threshold={}°, interval={}ms)",
        config.threshold,
        config.interval_ms
    );

    monitor::run(config, &SHOULD_RUN)
}

fn lock_instance(cache_dir: &Path) -> io::Result<File> {
    fs::create_dir_all(cache_dir)?;
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(cache_dir.join("daemon.lock"))?;
    file.try_lock()?;
    Ok(file)
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

#[cfg(test)]
mod tests {
    use super::lock_instance;
    use std::{env, fs, path::Path, process::Command, time::SystemTime};

    #[test]
    fn instance_lock_excludes_other_processes_until_drop() {
        const CACHE_ENV: &str = "LIDOFF_TEST_INSTANCE_CACHE";
        if let Some(cache_dir) = env::var_os(CACHE_ENV) {
            assert_eq!(
                lock_instance(Path::new(&cache_dir)).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            return;
        }
        let id = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let cache_dir = env::temp_dir().join(format!("lidoff-lock-{id}"));
        let lock = lock_instance(&cache_dir).unwrap();
        let output = Command::new(env::current_exe().unwrap())
            .args(["--exact", "tests::instance_lock_excludes_other_processes_until_drop"])
            .env(CACHE_ENV, &cache_dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        drop(lock);
        drop(lock_instance(&cache_dir).unwrap());
        fs::remove_dir_all(cache_dir).unwrap();
    }
}
