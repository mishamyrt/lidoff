mod args;
mod launch_agent;

use args::{parse, print_usage};

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
        return i32::from(launch_agent::uninstall());
    }

    if parsed.do_install {
        return i32::from(launch_agent::install(parsed.threshold));
    }

    let config = lidoff_daemon::DaemonConfig {
        threshold: parsed.threshold,
        interval_ms: parsed.interval_ms,
        verbose: parsed.verbose,
    };

    i32::from(lidoff_daemon::run(&config))
}
