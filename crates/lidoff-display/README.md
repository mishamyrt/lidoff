# lidoff-display

macOS display and keyboard-backlight control used by the daemon.

This crate wraps the native C shims for internal display brightness, external display state, and keyboard backlight into small Rust controllers with serializable state snapshots.

- Brightness values are normalized floats and restored through clamped `0.0..=1.0` values.
- Controllers are marked non-thread-safe because the underlying macOS APIs and shim state are process-global.
- External display disabling tracks only displays successfully captured by Skylight, so partial failures can be restored safely.
- State types implement `Serialize`/`Deserialize` because `lidoff-daemon` persists them for recovery.

## Example

```rust
use lidoff_display::{DisplayController, InternalDisplay};

let mut display = InternalDisplay::new();
let state = display.get_state().expect("read brightness");
display.disable().expect("disable display");
display.restore_state(state).expect("restore display");
```
