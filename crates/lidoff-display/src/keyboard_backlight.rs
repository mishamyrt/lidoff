use std::marker::PhantomData;
use std::rc::Rc;

use rkyv::{Archive, Deserialize, Serialize};
use thiserror::Error;

use crate::shim::{keyboard_backlight_get, keyboard_backlight_set};

#[derive(Debug, Default)]
pub struct KeyboardBacklight {
    _not_thread_safe: PhantomData<Rc<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Archive, Serialize, Deserialize)]
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

impl KeyboardBacklight {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_disabled(&mut self) -> bool {
        keyboard_backlight_get() == 0.0
    }

    pub fn disable(&mut self) -> Result<(), KeyboardBacklightError> {
        if self.is_disabled() {
            return Err(KeyboardBacklightError::AlreadyDisabled);
        }

        if !keyboard_backlight_set(0.0) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }
        Ok(())
    }

    pub fn get_state(&mut self) -> Result<KeyboardBacklightState, KeyboardBacklightError> {
        let brightness = keyboard_backlight_get();
        if brightness < 0.0 {
            return Err(KeyboardBacklightError::GetBrightnessFailed);
        }

        Ok(KeyboardBacklightState { brightness })
    }

    pub fn restore_state(
        &mut self,
        state: &KeyboardBacklightState,
    ) -> Result<(), KeyboardBacklightError> {
        if !keyboard_backlight_set(state.brightness.clamp(0.0, 1.0)) {
            return Err(KeyboardBacklightError::BrightnessFailed);
        }

        Ok(())
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
