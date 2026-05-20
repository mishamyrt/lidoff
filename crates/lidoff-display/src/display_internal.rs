use std::marker::PhantomData;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DisplayController,
    shim::{brightness_get, brightness_set},
};

#[derive(Debug, Default)]
pub struct InternalDisplay {
    _not_thread_safe: PhantomData<Rc<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

impl DisplayController for InternalDisplay {
    type State = InternalDisplayState;
    type Error = InternalDisplayError;

    fn is_disabled(&mut self) -> bool {
        brightness_get() == 0.0
    }

    fn disable(&mut self) -> Result<(), Self::Error> {
        if self.is_disabled() {
            return Err(InternalDisplayError::AlreadyDisabled);
        }

        if !brightness_set(0.0) {
            return Err(InternalDisplayError::BrightnessFailed);
        }
        Ok(())
    }

    fn get_state(&mut self) -> Result<Self::State, Self::Error> {
        let brightness = brightness_get();
        if brightness < 0.0 {
            return Err(InternalDisplayError::GetBrightnessFailed);
        }

        Ok(InternalDisplayState { brightness })
    }

    fn restore_state(&mut self, state: &Self::State) -> Result<(), Self::Error> {
        let brightness = state.brightness.clamp(0.0, 1.0);

        if !brightness_set(brightness) {
            return Err(InternalDisplayError::BrightnessFailed);
        }

        Ok(())
    }
}

impl InternalDisplay {
    pub fn new() -> Self {
        Self::default()
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
