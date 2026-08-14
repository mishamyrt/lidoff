use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

use crate::shim::{cursor_is_locked, cursor_lock, cursor_unlock};

#[derive(Debug, Default)]
pub struct Cursor {
    _not_thread_safe: PhantomData<Rc<()>>,
}

#[derive(Error, Debug)]
pub enum CursorError {
    #[error("already locked")]
    AlreadyLocked,

    #[error("already unlocked")]
    AlreadyUnlocked,

    #[error("failed to lock cursor")]
    LockFailed,

    #[error("failed to unlock cursor")]
    UnlockFailed,
}

impl Cursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_locked(&mut self) -> bool {
        cursor_is_locked()
    }

    pub fn lock(&mut self) -> Result<(), CursorError> {
        if self.is_locked() {
            return Err(CursorError::AlreadyLocked);
        }
        if !cursor_lock() {
            return Err(CursorError::LockFailed);
        }
        Ok(())
    }

    pub fn unlock(&mut self) -> Result<(), CursorError> {
        let was_locked = self.is_locked();
        // Always reconnect at the WindowServer level so recovery after a daemon restart
        // does not depend on this process-local flag.
        if !cursor_unlock() {
            return Err(CursorError::UnlockFailed);
        }
        if !was_locked {
            return Err(CursorError::AlreadyUnlocked);
        }
        Ok(())
    }
}
