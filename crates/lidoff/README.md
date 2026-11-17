# lidoff

CLI entry point for the `lidoff` daemon.

This crate parses CLI options and starts `lidoff-daemon` in the foreground with a resolved recovery-state cache directory.

## Example

```bash
lidoff
lidoff --threshold 25 --interval 500 --verbose
```
