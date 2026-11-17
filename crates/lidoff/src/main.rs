use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use lidoff_daemon::{
    DaemonConfig, MONITOR_DEFAULT_INTERVAL_MS, MONITOR_DEFAULT_THRESHOLD,
    MONITOR_FULL_CLOSE_ANGLE,
};

#[derive(Parser, Debug)]
#[command(
    name = "lidoff",
    version,
    about = "Monitor the MacBook lid angle in the foreground",
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
}

fn behavior_help() -> String {
    format!(
        "Behavior:\n  angle < {MONITOR_FULL_CLOSE_ANGLE}: fully closed, restore brightness values, unlock cursor, end caffeinate\n  angle < threshold: save brightness values, set them to 0, lock cursor, start caffeinate\n  angle >= threshold: restore saved brightness values, unlock cursor, end caffeinate"
    )
}

#[allow(clippy::print_stderr)]
fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("lidoff: home directory not found");
        return ExitCode::FAILURE;
    };
    let config = DaemonConfig {
        threshold: cli.threshold,
        interval_ms: cli.interval_ms,
        verbose: cli.verbose,
        recovery_cache_dir: PathBuf::from(home).join("Library/Caches/co.myrt.lidoff"),
    };

    if lidoff_daemon::run(&config) {
        ExitCode::SUCCESS
    } else {
        eprintln!("lidoff: failed to run monitor");
        ExitCode::FAILURE
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
                assert!(Cli::try_parse_from(["lidoff", flag, value]).is_ok());
            }
            for value in invalid {
                assert!(Cli::try_parse_from(["lidoff", flag, value]).is_err());
            }
        }
    }
}
