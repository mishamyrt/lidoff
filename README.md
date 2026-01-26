# lidoff

Daemon that turns off MacBook display brightness when the lid is partially closed.

## Installation

**Quick install:**

```bash
curl -fsSL https://raw.githubusercontent.com/mishamyrt/lidoff/master/install.sh | bash
lidoff --install
```

**From source:**

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
- `-i, --interval <ms>` — Polling interval (default: 300)
- `-v, --verbose` — Log current lid angle

## How it works

The daemon monitors lid angle and manages display brightness with caffeinate session:

- **Lid partially closed** (angle < threshold, but ≥ 5°): saves current brightness, sets it to 0, and starts a caffeinate session to prevent sleep
- **Lid opened** (angle ≥ threshold): restores saved brightness and ends caffeinate session
- **Lid fully closed** (angle < 5°): restores brightness and ends caffeinate session, allowing normal sleep behavior

This prevents the issue where fully closing the lid would leave the display at zero brightness after unlock.

## Requirements

- MacBook Air or MacBook Pro with Apple Silicon (M2, M3, M4)
