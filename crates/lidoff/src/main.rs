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
        value_parser = clap::value_parser!(u32).range(10..=60),
        value_name = "degrees",
        help = "Lid angle threshold in range 10-60 degrees"
    )]
    threshold: u32,

    #[arg(
        short = 'i',
        long = "interval",
        default_value_t = MONITOR_DEFAULT_INTERVAL_MS,
        value_parser = clap::value_parser!(u64).range(50..=5_000),
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

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn cli_validates_option_ranges() {
        for (flag, valid, invalid) in [
            ("--threshold", ["10", "30", "60"], ["9", "61", "-1", "text"]),
            ("--interval", ["50", "300", "5000"], ["49", "5001", "-1", "text"]),
        ] {
            for value in valid {
                assert!(Cli::try_parse_from(["lidoff", flag, value, "run"]).is_ok());
            }
            for value in invalid {
                assert!(Cli::try_parse_from(["lidoff", flag, value, "run"]).is_err());
            }
        }
    }
}
