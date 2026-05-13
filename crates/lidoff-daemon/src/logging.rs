use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_verbose(enabled: bool) {
    VERBOSE_ENABLED.store(enabled, Ordering::Relaxed);
}

pub(crate) fn verbose_enabled() -> bool {
    VERBOSE_ENABLED.load(Ordering::Relaxed)
}

#[allow(clippy::print_stdout)]
pub(crate) fn info_args(args: fmt::Arguments<'_>) {
    println!("[info]: {args}");
}

#[allow(clippy::print_stderr)]
pub(crate) fn error_args(args: fmt::Arguments<'_>) {
    eprintln!("[error]: {args}");
}

#[allow(clippy::print_stdout)]
pub(crate) fn debug_args(args: fmt::Arguments<'_>) {
    println!("[debug]: {args}");
}

macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logging::info_args(format_args!($($arg)*))
    };
}

macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logging::error_args(format_args!($($arg)*))
    };
}

macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::logging::verbose_enabled() {
            $crate::logging::debug_args(format_args!($($arg)*));
        }
    };
}

pub(crate) use debug;
pub(crate) use error;
pub(crate) use info;
