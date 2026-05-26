use lidoff_daemon::DaemonConfig;
use lunchd::KeepAlive;
use lunchd::LaunchAgent;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

const LAUNCH_AGENT_LABEL: &str = "co.myrt.lidoff";
const CACHE_RELATIVE_PATH: &str = "Library/Caches/co.myrt.lidoff";

#[derive(Error, Debug)]
pub(crate) enum LidoffError {
    #[error("failed to resolve executable path")]
    ResolveBinary(#[from] io::Error),

    #[error("failed to build launch agent")]
    LaunchAgent(#[from] lunchd::LaunchAgentBuilderError),

    #[error("failed to install launch agent")]
    Install(#[source] lunchd::AgentError),

    #[error("failed to uninstall launch agent")]
    Uninstall(#[source] lunchd::AgentError),

    #[error("failed to get launch agent status")]
    Status(#[source] lunchd::AgentError),

    #[error("home directory not found")]
    HomeNotFound,

    #[error("failed to run monitor")]
    RunMonitor,

    #[error("service is already installed")]
    AlreadyInstalled,

    #[error("service is not installed")]
    NotInstalled,
}

pub(crate) enum CommandOutcome {
    Silent,
    Message(&'static str),
}

type Result<T> = std::result::Result<T, LidoffError>;

pub(crate) struct Lidoff {
    threshold: u32,
    interval: u64,
    verbose: bool,
}

impl Lidoff {
    pub(crate) fn new(threshold: u32, interval: u64, verbose: bool) -> Self {
        Self { threshold, interval, verbose }
    }

    pub(crate) fn install(&self) -> Result<CommandOutcome> {
        let agent = self.launch_agent()?;
        if agent.exists() && agent.is_loaded().map_err(LidoffError::Install)? {
            return Err(LidoffError::AlreadyInstalled);
        }

        agent.install().map_err(LidoffError::Install)?;
        Ok(CommandOutcome::Message("service is successfully installed"))
    }

    pub(crate) fn uninstall(&self) -> Result<CommandOutcome> {
        let agent = self.launch_agent()?;
        if !agent.exists() && !agent.is_loaded().map_err(LidoffError::Uninstall)? {
            return Err(LidoffError::NotInstalled);
        }

        agent.uninstall().map_err(LidoffError::Uninstall)?;
        Ok(CommandOutcome::Message("service is successfully uninstalled"))
    }

    pub(crate) fn get_status(&self) -> Result<CommandOutcome> {
        let agent = self.launch_agent()?;
        if agent.exists() && agent.is_loaded().map_err(LidoffError::Status)? {
            Ok(CommandOutcome::Message("service is running"))
        } else if agent.exists() {
            Ok(CommandOutcome::Message("service installed but not running"))
        } else {
            Ok(CommandOutcome::Message("service not installed"))
        }
    }

    pub(crate) fn run_monitor(&self) -> Result<CommandOutcome> {
        let recovery_cache_dir = recovery_cache_dir()?;
        let config = DaemonConfig {
            threshold: self.threshold,
            interval_ms: self.interval,
            verbose: self.verbose,
            recovery_cache_dir,
        };

        if !lidoff_daemon::run(&config) {
            return Err(LidoffError::RunMonitor);
        }

        Ok(CommandOutcome::Silent)
    }

    fn launch_agent(&self) -> Result<LaunchAgent> {
        let bin_path = executable_path()?;
        build_agent(self.threshold, self.interval, &bin_path)
    }
}

fn recovery_cache_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or(LidoffError::HomeNotFound)?;
    Ok(home.join(CACHE_RELATIVE_PATH))
}

fn executable_path() -> io::Result<PathBuf> {
    let exec_path = std::env::current_exe()?;
    resolve_executable_path(&exec_path)
}

fn resolve_executable_path(exec_path: &Path) -> io::Result<PathBuf> {
    let real_path = match fs::read_link(exec_path) {
        Ok(path) if path.is_absolute() => path,
        Ok(path) => exec_path.parent().unwrap_or(Path::new("")).join(path),
        Err(_) => exec_path.to_path_buf(),
    };

    if real_path.is_absolute() {
        fs::canonicalize(real_path)
    } else {
        fs::canonicalize(std::env::current_dir()?.join(real_path))
    }
}

fn build_agent(threshold: u32, interval: u64, bin_path: &Path) -> Result<LaunchAgent> {
    let agent = LaunchAgent::builder(LAUNCH_AGENT_LABEL)
        .arg(bin_path.to_string_lossy())
        .arg("--threshold".to_string())
        .arg(threshold.to_string())
        .arg("--interval".to_string())
        .arg(interval.to_string())
        .arg("run")
        .stdout_path("/tmp/lidoff.stdout")
        .stderr_path("/tmp/lidoff.stderr")
        .keep_alive(KeepAlive::Always)
        .run_at_load(true)
        .build()?;

    Ok(agent)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::resolve_executable_path;

    struct TestDir {
        path: PathBuf,
    }

    static NEXT_TEST_DIR_ID: AtomicU64 = AtomicU64::new(0);

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_TEST_DIR_ID.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = env::temp_dir()
                .join(format!("lidoff-test-{}-{timestamp}-{id}", process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn resolves_relative_symlink_from_symlink_directory() {
        let temp = TestDir::new();
        let homebrew = temp.path().join("Homebrew");
        let bin_dir = homebrew.join("bin");
        let cellar_bin_dir = homebrew.join("Cellar/lidoff/0.4.0/bin");
        let executable = cellar_bin_dir.join("lidoff");
        let symlink_path = bin_dir.join("lidoff");

        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&cellar_bin_dir).unwrap();
        fs::write(&executable, b"lidoff").unwrap();
        symlink("../Cellar/lidoff/0.4.0/bin/lidoff", &symlink_path).unwrap();

        let resolved = resolve_executable_path(&symlink_path).unwrap();

        assert_eq!(resolved, fs::canonicalize(&executable).unwrap());
    }
}
