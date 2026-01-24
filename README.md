# lidoff

Daemon that turns off MacBook display brightness when the lid is partially closed.

## Installation

```bash
make
./build/lidoff --install
```

## Usage

```
lidoff [-t threshold] [-i interval]  Run daemon
lidoff --install [-t threshold]      Install as LaunchAgent
lidoff --uninstall                   Remove LaunchAgent
```

**Options:**
- `-t, --threshold <degrees>` — Lid angle threshold (default: 30)
- `-i, --interval <ms>` — Polling interval (default: 500)
- `-v, --verbose` — Log current lid angle

## How it works

When the lid angle drops below the threshold, the current brightness is saved and set to 0. When the lid is opened again, brightness is restored.

## Requirements

- macOS with lid angle sensor (MacBook Pro/Air with Apple Silicon or recent Intel)
