use std::marker::PhantomData;
use std::rc::Rc;

use rkyv::{Archive, Deserialize, Serialize};
use thiserror::Error;

use crate::shim::{brightness_get, brightness_set};

#[derive(Debug, Default)]
pub struct InternalDisplay {
    _not_thread_safe: PhantomData<Rc<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Archive, Serialize, Deserialize)]
pub struct InternalDisplayState {
    pub brightness: f32,
}

#[derive(Error, Debug)]
pub enum InternalDisplayError {
    #[error("already disabled")]
    AlreadyDisabled,

    #[error("failed to set brightness")]
    BrightnessFailed,

    #[error("failed to get brightness")]
    GetBrightnessFailed,
}

impl InternalDisplay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_disabled(&mut self) -> bool {
        brightness_get() == 0.0
    }

    pub fn disable(&mut self) -> Result<(), InternalDisplayError> {
        if self.is_disabled() {
            return Err(InternalDisplayError::AlreadyDisabled);
        }

        if !brightness_set(0.0) {
            return Err(InternalDisplayError::BrightnessFailed);
        }
        Ok(())
    }

    pub fn get_state(&mut self) -> Result<InternalDisplayState, InternalDisplayError> {
        let brightness = brightness_get();
        if brightness < 0.0 {
            return Err(InternalDisplayError::GetBrightnessFailed);
        }

        Ok(InternalDisplayState { brightness })
    }

    pub fn restore_state(
        &mut self,
        state: &InternalDisplayState,
    ) -> Result<(), InternalDisplayError> {
        let brightness = state.brightness.clamp(0.0, 1.0);

        if !brightness_set(brightness) {
            return Err(InternalDisplayError::BrightnessFailed);
        }

        Ok(())
    }

    pub fn disable_from_state(
        &mut self,
        state: InternalDisplayState,
    ) -> Result<(), InternalDisplayError> {
        if state.brightness == 0.0 {
            return Err(InternalDisplayError::AlreadyDisabled);
        }

        if !brightness_set(0.0) {
            return Err(InternalDisplayError::BrightnessFailed);
        }

        Ok(())
    }
}
