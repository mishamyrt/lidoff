use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(enabled: bool) {
    VERBOSE_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn info(message: impl AsRef<str>) {
    println!("lidoff[info]: {}", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    eprintln!("lidoff[error]: {}", message.as_ref());
}

pub fn debug(message: impl AsRef<str>) {
    if VERBOSE_ENABLED.load(Ordering::Relaxed) {
        println!("lidoff[debug]: {}", message.as_ref());
    }
}
