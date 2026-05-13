use std::{
    ffi::c_void,
    marker::PhantomData,
    rc::Rc,
    sync::mpsc::{self, SyncSender},
    thread::{self, JoinHandle},
};
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
    #[error("failed to spawn power notifier")]
    StartFailed(#[from] std::io::Error),
    #[error("failed to register power notifier")]
    RegisterFailed,
    #[error("failed to receive power notifier startup status")]
    StartupStatusFailed,
}

unsafe extern "C" {
    fn CaffeinateStart() -> u8;
    fn CaffeinateStop() -> u8;

    fn PowerObserverRunLoop(
        will_sleep: extern "C" fn(*mut c_void),
        did_wake: extern "C" fn(*mut c_void),
        context: *mut c_void,
        startup_callback: extern "C" fn(u8, *mut c_void),
        startup_context: *mut c_void,
    ) -> u8;
}

#[derive(Debug, Default)]
pub struct Caffeinate {
    _not_thread_safe: PhantomData<Rc<()>>,
}

impl Caffeinate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self) -> Result<(), CaffeinateError> {
        match unsafe { CaffeinateStart() } {
            0 => Ok(()),
            2 => Err(CaffeinateError::AlreadyActive),
            _ => Err(CaffeinateError::StartFailed),
        }
    }

    pub fn stop(&mut self) -> Result<(), CaffeinateError> {
        match unsafe { CaffeinateStop() } {
            0 => Ok(()),
            3 => Err(CaffeinateError::NotActive),
            _ => Err(CaffeinateError::StopFailed),
        }
    }
}

pub struct PowerObserver {
    context: *mut c_void,
    thread: Option<JoinHandle<()>>,
}

impl Default for PowerObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerObserver {
    pub fn new() -> Self {
        Self { context: std::ptr::null_mut(), thread: None }
    }

    pub fn start(
        &mut self,
        will_sleep: extern "C" fn(*mut c_void),
        did_wake: extern "C" fn(*mut c_void),
    ) -> Result<(), PowerNotifierError> {
        let context = self.context as usize;
        let (startup_tx, startup_rx) = mpsc::sync_channel::<u8>(1);
        let thread =
            thread::Builder::new().name("lidoff.power".to_owned()).spawn(move || {
                let startup_context = (&raw const startup_tx).cast_mut().cast::<c_void>();
                unsafe {
                    PowerObserverRunLoop(
                        will_sleep,
                        did_wake,
                        context as *mut c_void,
                        handle_power_observer_startup,
                        startup_context,
                    );
                }
            })?;

        if startup_rx.recv().map_err(|_| PowerNotifierError::StartupStatusFailed)? != 0 {
            return Err(PowerNotifierError::RegisterFailed);
        }

        self.thread = Some(thread);
        Ok(())
    }
}

impl Drop for PowerObserver {
    fn drop(&mut self) {
        drop(self.thread.take());
    }
}

extern "C" fn handle_power_observer_startup(status: u8, context: *mut c_void) {
    let startup_tx = unsafe { &*context.cast::<SyncSender<u8>>() };
    let _ = startup_tx.send(status);
}
