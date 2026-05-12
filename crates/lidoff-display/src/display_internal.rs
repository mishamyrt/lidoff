use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DisplayController,
    coregraphics::{brightness_get, brightness_set},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct InternalDisplay;

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
}

impl DisplayController for InternalDisplay {
    type State = InternalDisplayState;
    type Error = InternalDisplayError;

    fn is_disabled(&self) -> bool {
        brightness_get() == 0.0
    }

    fn disable(&self) -> Result<(), Self::Error> {
        if self.is_disabled() {
            return Err(InternalDisplayError::AlreadyDisabled);
        }

        if !brightness_set(0.0) {
            return Err(InternalDisplayError::BrightnessFailed);
        }
        Ok(())
    }

    fn get_state(&self) -> Option<Self::State> {
        let brightness = brightness_get();
        (brightness >= 0.0).then_some(InternalDisplayState { brightness })
    }

    fn restore_state(&self, state: Self::State) -> Result<(), Self::Error> {
        let brightness = state.brightness.clamp(0.0, 1.0);

        if !brightness_set(brightness) {
            return Err(InternalDisplayError::BrightnessFailed);
        }

        Ok(())
    }
}
