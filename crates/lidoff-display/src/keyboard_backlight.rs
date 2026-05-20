use std::marker::PhantomData;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DisplayController,
    shim::{keyboard_backlight_get, keyboard_backlight_set},
};

#[derive(Debug, Default)]
pub struct KeyboardBacklight {
    _not_thread_safe: PhantomData<Rc<()>>,
}

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

    #[error("failed to get keyboard backlight")]
    GetBrightnessFailed,
}

impl DisplayController for KeyboardBacklight {
    type State = KeyboardBacklightState;
    type Error = KeyboardBacklightError;

    fn is_disabled(&mut self) -> bool {
        keyboard_backlight_get() == 0.0
    }

    fn disable(&mut self) -> Result<(), Self::Error> {
        if self.is_disabled() {
            return Err(KeyboardBacklightError::AlreadyDisabled);
        }

        if !keyboard_backlight_set(0.0) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }
        Ok(())
    }

    fn get_state(&mut self) -> Result<Self::State, Self::Error> {
        let brightness = keyboard_backlight_get();
        if brightness < 0.0 {
            return Err(KeyboardBacklightError::GetBrightnessFailed);
        }

        Ok(KeyboardBacklightState { brightness })
    }

    fn restore_state(&mut self, state: &Self::State) -> Result<(), Self::Error> {
        if !keyboard_backlight_set(state.brightness.clamp(0.0, 1.0)) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }

        Ok(())
    }
}

impl KeyboardBacklight {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn disable_from_state(
        &mut self,
        state: KeyboardBacklightState,
    ) -> Result<(), KeyboardBacklightError> {
        if state.brightness == 0.0 {
            return Err(KeyboardBacklightError::AlreadyDisabled);
        }

        if !keyboard_backlight_set(0.0) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }

        Ok(())
    }
}
