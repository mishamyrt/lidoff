mod display_external;
mod display_internal;
mod keyboard_backlight;
mod shim;

pub use display_external::{
    ExternalDisplayDisableResult, ExternalDisplayError, ExternalDisplayState, ExternalDisplays,
};
pub use display_internal::{InternalDisplay, InternalDisplayError, InternalDisplayState};
pub use keyboard_backlight::{
    KeyboardBacklight, KeyboardBacklightError, KeyboardBacklightState,
};

/// A trait for controlling the display.
pub trait DisplayController {
    /// The state of the display.
    type State;

    /// The error type of the display controller operations.
    type Error;

    /// Returns `true` if the display is currently disabled.
    fn is_disabled(&self) -> bool;

    /// Disables the display.
    fn disable(&self) -> Result<(), Self::Error>;

    /// Returns the current state of the display.
    fn get_state(&self) -> Option<Self::State>;

    /// Restores the display to the given state.
    fn restore_state(&self, state: Self::State) -> Result<(), Self::Error>;
}
