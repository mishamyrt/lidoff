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

    #[error("home directory not found")]
    HomeNotFound,

    #[error("failed to run monitor")]
    RunMonitor,
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

    pub(crate) fn install(&self) -> Result<()> {
        let agent = self.launch_agent()?;
        agent.install().map_err(LidoffError::Install)
    }

    pub(crate) fn uninstall(&self) -> Result<()> {
        let agent = self.launch_agent()?;
        agent.uninstall().map_err(LidoffError::Uninstall)
    }

    pub(crate) fn run_monitor(&self) -> Result<()> {
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

        Ok(())
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
    let real_path = match fs::read_link(&exec_path) {
        Ok(path) => path,
        Err(_) => exec_path,
    };

    if real_path.is_absolute() {
        Ok(real_path)
    } else {
        Ok(std::env::current_dir()?.join(real_path))
    }
}

fn build_agent(threshold: u32, interval: u64, bin_path: &Path) -> Result<LaunchAgent> {
    let agent = LaunchAgent::builder(LAUNCH_AGENT_LABEL)
        .arg(bin_path.to_string_lossy())
        .arg("--threshold".to_string())
        .arg(threshold.to_string())
        .arg("--interval".to_string())
        .arg(interval.to_string())
        .keep_alive(KeepAlive::Always)
        .run_at_load(true)
        .build()?;

    Ok(agent)
}
