# lidoff-lidsensor

Rust wrapper around the native MacBook lid-angle sensor shim.

The crate exposes one handle, `LidSensor`, which initializes the native sensor on construction, reads the current lid angle in degrees, and closes the native handle on drop.

## Details

- `LidSensor::new` can fail when the machine does not expose the expected Apple Silicon lid sensor.
- `get_angle` takes `&mut self` to make polling explicit and avoid concurrent native reads.
- A native `-1` reading is reported as `SensorError::ReadFailed`.

## Example

```rust
let mut sensor = lidoff_lidsensor::LidSensor::new().expect("open lid sensor");
let angle = sensor.get_angle().expect("read lid angle");
println!("lid angle: {angle} deg");
```
