# lidoff-daemon

Runtime orchestration for lid-angle driven display and power behavior.

This crate owns the monitoring loop. It reads the lid sensor, classifies the lid state, applies display/keyboard/cursor/caffeinate effects, and persists recovery data so brightness can be restored after restarts or sleep transitions.

## Example

```rust
use std::path::PathBuf;

let config = lidoff_daemon::DaemonConfig {
    threshold: 30,
    interval_ms: 300,
    verbose: false,
    recovery_cache_dir: PathBuf::from("/tmp/lidoff-cache"),
};

let ok = lidoff_daemon::run(&config);
```
