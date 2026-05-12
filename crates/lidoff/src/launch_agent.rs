use std::fs;
use std::io;
use std::path::PathBuf;

use lunchd::KeepAlive;
use lunchd::LaunchAgent;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum LaunchAgentError {
    #[error("failed to resolve executable path")]
    Resolve(#[from] io::Error),

    #[error("failed to build launch agent")]
    Build(#[from] lunchd::LaunchAgentBuilderError),

    #[error("failed to install launch agent")]
    Install(#[source] lunchd::AgentError),

    #[error("failed to uninstall launch agent")]
    Uninstall(#[source] lunchd::AgentError),
}

const LAUNCH_AGENT_LABEL: &str = "co.myrt.lidoff";

struct AgentParams {
    threshold: u32,
    interval: u64,
}

fn build_agent(params: Option<AgentParams>) -> Result<LaunchAgent, LaunchAgentError> {
    let bin_path = resolve_executable_path()?;
    let (threshold, interval) = match params {
        Some(params) => (params.threshold, params.interval),
        None => (0, 0),
    };
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

fn resolve_executable_path() -> io::Result<PathBuf> {
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

pub(crate) fn install(threshold: u32, interval: u64) -> Result<(), LaunchAgentError> {
    let agent = build_agent(Some(AgentParams { threshold, interval }))?;
    agent.install().map_err(LaunchAgentError::Install)
}

pub(crate) fn uninstall() -> Result<(), LaunchAgentError> {
    let agent = build_agent(None)?;
    agent.install().map_err(LaunchAgentError::Uninstall)
}
