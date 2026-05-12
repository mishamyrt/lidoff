use lidoff_daemon::{
    MONITOR_DEFAULT_INTERVAL_MS, MONITOR_DEFAULT_THRESHOLD, MONITOR_FULL_CLOSE_ANGLE,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParsedArgs {
    pub threshold: u32,
    pub interval_ms: u64,
    pub do_install: bool,
    pub do_uninstall: bool,
    pub verbose: bool,
}

impl Default for ParsedArgs {
    fn default() -> Self {
        Self {
            threshold: MONITOR_DEFAULT_THRESHOLD,
            interval_ms: MONITOR_DEFAULT_INTERVAL_MS,
            do_install: false,
            do_uninstall: false,
            verbose: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParseError {
    pub message: String,
    pub print_usage: bool,
}

pub(crate) fn parse<I>(args: I) -> Result<ParsedArgs, ParseError>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = ParsedArgs::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--enable" => parsed.do_install = true,
            "--disable" => parsed.do_uninstall = true,
            "-v" | "--verbose" => parsed.verbose = true,
            "-t" | "--threshold" => {
                let Some(value) = args.next() else {
                    return Err(ParseError {
                        message: format!("unknown option: {arg}"),
                        print_usage: true,
                    });
                };
                let threshold = value.parse::<u32>().map_err(|_| ParseError {
                    message: format!("invalid threshold: {value} (0-180)"),
                    print_usage: false,
                })?;
                if !(0..=180).contains(&threshold) {
                    return Err(ParseError {
                        message: format!("invalid threshold: {threshold} (0-180)"),
                        print_usage: false,
                    });
                }
                parsed.threshold = threshold;
            }
            "-i" | "--interval" => {
                let Some(value) = args.next() else {
                    return Err(ParseError {
                        message: format!("unknown option: {arg}"),
                        print_usage: true,
                    });
                };
                let interval_ms = value.parse::<u64>().map_err(|_| ParseError {
                    message: format!("invalid interval: {value} (100-10000)"),
                    print_usage: false,
                })?;
                if !(100..=10_000).contains(&interval_ms) {
                    return Err(ParseError {
                        message: format!("invalid interval: {interval_ms} (100-10000)"),
                        print_usage: false,
                    });
                }
                parsed.interval_ms = interval_ms;
            }
            _ => {
                return Err(ParseError {
                    message: format!("unknown option: {arg}"),
                    print_usage: true,
                });
            }
        }
    }

    if parsed.do_install && parsed.do_uninstall {
        return Err(ParseError {
            message: "--enable and --disable cannot be used together".to_owned(),
            print_usage: true,
        });
    }

    Ok(parsed)
}

#[allow(clippy::print_stdout)]
pub(crate) fn print_usage(program_name: &str) {
    println!("lidoff - MacBook lid angle brightness daemon\n");
    println!("Usage:");
    println!("  {program_name} [-t threshold] [-i interval]  Run daemon");
    println!("  {program_name} --enable [-t threshold]      Install as LaunchAgent");
    println!("  {program_name} --disable                   Remove LaunchAgent");
    println!("  {program_name} --help                        Show this help\n");
    println!("  {program_name} --version                     Show version\n");
    println!("Options:");
    println!(
        "  -t, --threshold <degrees>   Lid angle threshold (default: {MONITOR_DEFAULT_THRESHOLD})"
    );
    println!(
        "  -i, --interval <ms>         Polling interval in ms (default: {MONITOR_DEFAULT_INTERVAL_MS})"
    );
    println!("  -v, --verbose               Log current lid angle\n");
    println!("Behavior:");
    println!(
        "  angle < {MONITOR_FULL_CLOSE_ANGLE}: fully closed, restore brightness and end caffeinate"
    );
    println!("  angle < threshold: save brightness, set to 0, start caffeinate");
    println!("  angle >= threshold: restore saved brightness, end caffeinate");
}

#[cfg(test)]
mod tests {
    use super::{ParsedArgs, parse};

    #[test]
    fn parses_daemon_flags() {
        let parsed = parse([
            "-t".to_owned(),
            "42".to_owned(),
            "-i".to_owned(),
            "500".to_owned(),
            "--verbose".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            parsed,
            ParsedArgs {
                threshold: 42,
                interval_ms: 500,
                do_install: false,
                do_uninstall: false,
                verbose: true,
            }
        );
    }

    #[test]
    fn rejects_invalid_threshold() {
        let error = parse(["--threshold".to_owned(), "181".to_owned()]).unwrap_err();
        assert_eq!(error.message, "invalid threshold: 181 (0-180)");
        assert!(!error.print_usage);
    }

    #[test]
    fn rejects_unparseable_threshold() {
        let error = parse(["--threshold".to_owned(), "abc".to_owned()]).unwrap_err();
        assert_eq!(error.message, "invalid threshold: abc (0-180)");
        assert!(!error.print_usage);
    }

    #[test]
    fn rejects_unparseable_interval() {
        let error = parse(["--interval".to_owned(), "abc".to_owned()]).unwrap_err();
        assert_eq!(error.message, "invalid interval: abc (100-10000)");
        assert!(!error.print_usage);
    }

    #[test]
    fn rejects_install_and_uninstall_together() {
        let error = parse(["--enable".to_owned(), "--disable".to_owned()]).unwrap_err();
        assert_eq!(error.message, "--enable and --disable cannot be used together");
        assert!(error.print_usage);
    }
}
