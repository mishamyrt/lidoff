use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DisplayController,
    shim::{keyboard_backlight_get, keyboard_backlight_set},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardBacklight;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KeyboardBacklightState {
    pub brightness: f32,
}

#[derive(Error, Debug)]
pub enum KeyboardBacklightError {
    #[error("already disabled")]
    AlreadyDisabled,

    #[error("failed to set keyboard backlight")]
    BrightnessFailed,
}

impl DisplayController for KeyboardBacklight {
    type State = KeyboardBacklightState;
    type Error = KeyboardBacklightError;

    fn is_disabled(&self) -> bool {
        keyboard_backlight_get() == 0.0
    }

    fn disable(&self) -> Result<(), Self::Error> {
        if self.is_disabled() {
            return Err(KeyboardBacklightError::AlreadyDisabled);
        }

        if !keyboard_backlight_set(0.0) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }
        Ok(())
    }

    fn get_state(&self) -> Option<Self::State> {
        let brightness = keyboard_backlight_get();
        (brightness >= 0.0).then_some(KeyboardBacklightState { brightness })
    }

    fn restore_state(&self, state: Self::State) -> Result<(), Self::Error> {
        if !keyboard_backlight_set(state.brightness.clamp(0.0, 1.0)) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }

        Ok(())
    }
}
