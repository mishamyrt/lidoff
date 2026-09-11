use smallvec::SmallVec;

/* Brightness */

unsafe extern "C" {
    fn BrightnessGet() -> f32;
    fn BrightnessSet(brightness: f32) -> u8;
    fn CursorIsLocked() -> u8;
    fn CursorLock() -> u8;
    fn CursorUnlock() -> u8;
    fn KeyboardBacklightGet() -> f32;
    fn KeyboardBacklightSet(brightness: f32) -> u8;
}

/// Returns the current internal display brightness level.
pub(crate) fn brightness_get() -> f32 {
    unsafe { BrightnessGet() }
}

/// Sets the internal display brightness level.
pub(crate) fn brightness_set(brightness: f32) -> bool {
    unsafe { BrightnessSet(brightness) != 0 }
}

/// Returns the current keyboard backlight brightness level.
pub(crate) fn keyboard_backlight_get() -> f32 {
    unsafe { KeyboardBacklightGet() }
}

/// Sets the keyboard backlight brightness level.
pub(crate) fn keyboard_backlight_set(brightness: f32) -> bool {
    unsafe { KeyboardBacklightSet(brightness) != 0 }
}

/// Disconnects mouse movement from the system cursor.
pub(crate) fn cursor_lock() -> bool {
    unsafe { CursorLock() != 0 }
}

/// Reconnects mouse movement to the system cursor.
pub(crate) fn cursor_unlock() -> bool {
    unsafe { CursorUnlock() != 0 }
}

/// Returns whether this process has disconnected mouse movement from the cursor.
pub(crate) fn cursor_is_locked() -> bool {
    unsafe { CursorIsLocked() != 0 }
}

/* Displays */

unsafe extern "C" {
    fn DisplaysListOnline(display_ids: *mut u32, capacity: usize, count_out: *mut usize)
    -> u8;
    fn DisplayIsBuiltin(display_id: u32) -> u8;
}

const MAX_DISPLAY_COUNT: usize = 32;
type DisplayIds = SmallVec<[u32; MAX_DISPLAY_COUNT]>;

/// Returns a list of online display IDs.
pub(crate) fn online_displays() -> Option<DisplayIds> {
    let mut buffer = [0_u32; MAX_DISPLAY_COUNT];
    let mut count = 0_usize;
    let ok = unsafe {
        c_bool(DisplaysListOnline(buffer.as_mut_ptr(), buffer.len(), &raw mut count))
    };
    if !ok || count > buffer.len() {
        return None;
    }

    Some(SmallVec::from_slice(&buffer[..count]))
}

/// Returns whether the display is built-in (i.e. not an external monitor).
pub(crate) fn is_builtin(display_id: u32) -> bool {
    unsafe { c_bool(DisplayIsBuiltin(display_id)) }
}

/* Skylight */

unsafe extern "C" {
    fn SkylightFinalize() -> u8;
    fn SkylightDisableDisplay(display_id: u32) -> u8;
    fn SkylightRestoreFromState(display_ids: *const u32, count: usize) -> usize;
}

/// Finalizes the Skylight session.
pub(crate) fn finalize_skylight() -> bool {
    unsafe { c_bool(SkylightFinalize()) }
}

/// Disables the Skylight display with the given ID.
/// Returns `true` if the display was disabled successfully.
pub(crate) fn disable_skylight_display(display_id: u32) -> bool {
    unsafe { c_bool(SkylightDisableDisplay(display_id)) }
}

/// Restores the Skylight state from the given display IDs.
/// Returns the number of displays that were restored.
pub(crate) fn restore_skylight_state(display_ids: &[u32]) -> usize {
    unsafe { SkylightRestoreFromState(display_ids.as_ptr(), display_ids.len()) }
}

fn c_bool(value: u8) -> bool {
    value != 0
}
