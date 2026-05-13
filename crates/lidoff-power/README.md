# lidoff-power

Power-management helpers for `lidoff-daemon`.

This crate wraps the native caffeinate and sleep/wake observer shims. It is responsible for keeping the Mac awake while the lid is partially closed and for notifying the daemon when the system is about to sleep or has just woken.

## Details

- `Caffeinate` is non-thread-safe because the native assertion is process-global.
- `PowerObserver::start` waits for native observer registration before returning.
- The observer owns a background run loop thread; callbacks must be `extern "C" fn(*mut c_void)`.

## Example

```rust
let mut caffeinate = lidoff_power::Caffeinate::new();
caffeinate.start().expect("start caffeinate");
caffeinate.stop().expect("stop caffeinate");
```
