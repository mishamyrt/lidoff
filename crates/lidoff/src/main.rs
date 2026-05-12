mod args;
mod launch_agent;

use args::{parse, print_usage};
use std::path::PathBuf;

const CACHE_RELATIVE_PATH: &str = "Library/Caches/co.myrt.lidoff";

fn main() {
    let exit_code = run();
    std::process::exit(exit_code);
}

#[allow(clippy::print_stdout)]
#[allow(clippy::print_stderr)]
fn run() -> i32 {
    let program_name = std::env::args().next().unwrap_or_else(|| "lidoff".to_owned());
    let mut raw_args = std::env::args().skip(1);

    if raw_args.any(|arg| arg == "--help" || arg == "-h") {
        print_usage(&program_name);
        return 0;
    }

    let mut raw_args = std::env::args().skip(1);
    if raw_args.any(|arg| arg == "--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return 0;
    }

    let parsed = match parse(std::env::args().skip(1)) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("lidoff: {}", error.message);
            if error.print_usage {
                print_usage(&program_name);
            }
            return 1;
        }
    };

    if parsed.do_uninstall {
        if let Err(error) = launch_agent::uninstall() {
            eprintln!("failed to uninstall launch agent: {error}");
            return 1;
        }
        return 0;
    }

    if parsed.do_install {
        if let Err(error) = launch_agent::install(parsed.threshold, parsed.interval_ms) {
            eprintln!("failed to install launch agent: {error}");
            return 1;
        }
        return 0;
    }

    let recovery_cache_dir = match recovery_cache_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("lidoff: {error}");
            return 1;
        }
    };

    let config = lidoff_daemon::DaemonConfig {
        threshold: parsed.threshold,
        interval_ms: parsed.interval_ms,
        verbose: parsed.verbose,
        recovery_cache_dir,
    };

    i32::from(lidoff_daemon::run(&config))
}

fn recovery_cache_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_owned())?;
    Ok(home.join(CACHE_RELATIVE_PATH))
}
