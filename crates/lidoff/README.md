# lidoff

CLI entry point for the `lidoff` daemon.

This crate is intentionally thin: it parses user-facing flags, installs or removes the macOS LaunchAgent, and otherwise starts `lidoff-daemon` with a resolved recovery-state cache directory.

## Example

```bash
lidoff --threshold 30 --interval 300
lidoff --enable --threshold 25
lidoff --disable
```
