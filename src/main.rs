mod args;
mod external_display;
mod launch_agent;
mod lid_sensor;
mod logging;
mod monitor;
mod platform;
mod recovery_state;

use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

use args::{parse, print_usage};
use monitor::MonitorConfig;

static SHOULD_RUN: AtomicBool = AtomicBool::new(true);

const SIGHUP: c_int = 1;
const SIGINT: c_int = 2;
const SIGQUIT: c_int = 3;
const SIGTERM: c_int = 15;

unsafe extern "C" {
    fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> usize;
}

fn main() {
    let exit_code = run();
    std::process::exit(exit_code);
}

fn run() -> i32 {
    let program_name = std::env::args()
        .next()
        .unwrap_or_else(|| "lidoff".to_owned());
    let mut raw_args = std::env::args().skip(1).peekable();

    if raw_args.any(|arg| arg == "--help" || arg == "-h") {
        print_usage(&program_name);
        return 0;
    }

    let mut raw_args = std::env::args().skip(1).peekable();
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

    logging::set_verbose(parsed.verbose);

    if parsed.do_uninstall {
        return if launch_agent::uninstall() { 0 } else { 1 };
    }

    if parsed.do_install {
        return if launch_agent::install(parsed.threshold) {
            0
        } else {
            1
        };
    }

    install_signal_handlers();

    if !platform::lid_sensor_init() {
        logging::error("failed to initialize lid sensor");
        return 1;
    }

    let config = MonitorConfig {
        threshold: parsed.threshold,
        interval_ms: parsed.interval_ms,
    };
    monitor::run(&config, &SHOULD_RUN);
    platform::lid_sensor_close();
    0
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
