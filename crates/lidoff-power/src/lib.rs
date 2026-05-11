use std::{ffi::c_void, thread};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CaffeinateError {
    #[error("caffeinate is already active")]
    AlreadyActive,
    #[error("caffeinate is not active")]
    NotActive,
    #[error("failed to start caffeinate")]
    StartFailed,
    #[error("failed to stop caffeinate")]
    StopFailed,
}

#[derive(Error, Debug)]
pub enum PowerNotifierError {
    #[error("failed to start power notifier")]
    StartFailed(#[from] std::io::Error),
}

unsafe extern "C" {
    fn CaffeinateStart() -> u8;
    fn CaffeinateStop() -> u8;

    fn PowerObserverRunLoop(
        will_sleep: extern "C" fn(*mut c_void),
        did_wake: extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> u8;
}

pub fn caffeinate_start() -> Result<(), CaffeinateError> {
    match unsafe { CaffeinateStart() } {
        0 => Ok(()),
        2 => Err(CaffeinateError::AlreadyActive),
        _ => Err(CaffeinateError::StartFailed),
    }
}

pub fn caffeinate_stop() -> Result<(), CaffeinateError> {
    match unsafe { CaffeinateStop() } {
        0 => Ok(()),
        2 => Err(CaffeinateError::NotActive),
        _ => Err(CaffeinateError::StopFailed),
    }
}

pub struct PowerObserver {
    context: *mut c_void,
}

impl Default for PowerObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerObserver {
    pub fn new() -> Self {
        Self {
            context: std::ptr::null_mut(),
        }
    }

    pub fn start(
        &mut self,
        will_sleep: extern "C" fn(*mut c_void),
        did_wake: extern "C" fn(*mut c_void),
    ) -> Result<(), PowerNotifierError> {
        let context = self.context as usize;
        thread::Builder::new()
            .name("lidoff.power".to_owned())
            .spawn(move || unsafe {
                if PowerObserverRunLoop(will_sleep, did_wake, context as *mut c_void) != 0 {
                    return Err(PowerNotifierError::StartFailed);
                }
                Ok(())
            })
            .map_err(PowerNotifierError::StartFailed)
            .map(|_| ())
    }
}
