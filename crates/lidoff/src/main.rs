mod lidoff;

use std::error::Error;

use clap::{Parser, Subcommand};
use lidoff_daemon::{
    MONITOR_DEFAULT_INTERVAL_MS, MONITOR_DEFAULT_THRESHOLD, MONITOR_FULL_CLOSE_ANGLE,
};

use crate::lidoff::{CommandOutcome, Lidoff};

#[derive(Parser, Debug)]
#[command(
    name = "lidoff",
    version,
    about = "MacBook lid angle display, keyboard, and cursor daemon",
    long_about = None,
    after_help = behavior_help()
)]
struct Cli {
    #[arg(
        short = 't',
        long = "threshold",
        default_value_t = MONITOR_DEFAULT_THRESHOLD,
        value_parser = parse_threshold,
        value_name = "degrees",
        help = "Lid angle threshold in range 10-60 degrees"
    )]
    threshold: u32,

    #[arg(
        short = 'i',
        long = "interval",
        default_value_t = MONITOR_DEFAULT_INTERVAL_MS,
        value_parser = parse_interval,
        value_name = "ms",
        help = "Polling interval in ms in range 50-5000 ms"
    )]
    interval_ms: u64,

    #[arg(short = 'v', long = "verbose", help = "Log current lid angle")]
    verbose: bool,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand, Clone)]
enum Command {
    /// Install service as launch agent
    Install,
    /// Uninstall service's launch agent
    Uninstall,
    /// Check service status
    Status,
    /// Start the monitor in the foreground
    Run,
}

fn parse_threshold(value: &str) -> Result<u32, String> {
    let threshold =
        value.parse::<u32>().map_err(|_| format!("invalid threshold: {value} (10-60)"))?;
    if !(10..=60).contains(&threshold) {
        return Err(format!("invalid threshold: {threshold} (10-60)"));
    }
    Ok(threshold)
}

fn parse_interval(value: &str) -> Result<u64, String> {
    let interval_ms =
        value.parse::<u64>().map_err(|_| format!("invalid interval: {value} (50-5000)"))?;
    if !(50..=5_000).contains(&interval_ms) {
        return Err(format!("invalid interval: {interval_ms} (50-5000)"));
    }
    Ok(interval_ms)
}

fn behavior_help() -> String {
    format!(
        "Behavior:\n  angle < {MONITOR_FULL_CLOSE_ANGLE}: fully closed, restore brightness values, unlock cursor, end caffeinate\n  angle < threshold: save brightness values, set them to 0, lock cursor, start caffeinate\n  angle >= threshold: restore saved brightness values, unlock cursor, end caffeinate"
    )
}

#[allow(clippy::print_stderr, clippy::print_stdout)]
fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let lidoff = Lidoff::new(cli.threshold, cli.interval_ms, cli.verbose);

    let result = match cli.command {
        Command::Install => lidoff.install(),
        Command::Uninstall => lidoff.uninstall(),
        Command::Run => lidoff.run_monitor(),
        Command::Status => lidoff.get_status(),
    };

    match result {
        Ok(CommandOutcome::Silent) => std::process::ExitCode::SUCCESS,
        Ok(CommandOutcome::Message(message)) => {
            println!("{message}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("lidoff: {error}");
            if let Some(source_err) = error.source() {
                eprintln!("{source_err}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}
