<p align="center">
    <img src="./docs/logo.svg" alt="lidoff logo" height="141">
</p>

<p align="center">
    <a href="https://github.com/mishamyrt/lidoff/actions/workflows/qa.yml">
        <img src="https://github.com/mishamyrt/lidoff/actions/workflows/qa.yml/badge.svg" alt="Build status" />
    </a>
    <a href="https://github.com/mishamyrt/lidoff/releases/latest">
        <img src="https://img.shields.io/github/v/tag/mishamyrt/lidoff?label=version"
    </a>
    <a href="./LICENSE">
        <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license" />
    </a>
</p>

`lidoff` is a macOS daemon for Apple Silicon MacBooks. It watches the lid angle, turns off the built-in display and keyboard backlight when the lid is partially closed, locks cursor movement, disables external displays, and keeps the machine awake with `caffeinate`.

## Why

`lidoff` is useful when you want the MacBook to keep working with the lid nearly closed:

- Run a long refactoring or LLM coding session while you step away.
- Keep audio playing without lighting the room.
- Leave a download, build, or sync running overnight.

The idea is to add a middle state between "open" and "closed": when the lid is only partially closed, the laptop stays awake but the displays and keyboard backlight are dimmed to zero.

https://github.com/user-attachments/assets/86740896-9a4b-4e2e-a616-a1353a61575c

## Features

- Reads the MacBook lid angle instead of relying only on open/closed state.
- Dims the built-in display and keyboard backlight to zero when the lid is partially closed.
- Locks cursor movement from any mouse or trackpad while the displays are off.
- Starts a `caffeinate` session while the partial-close state is active.
- Restores saved brightness values when the lid opens or fully closes.
- Disables and restores external displays when possible.

## Requirements

- MacBook Air or MacBook Pro with Apple Silicon (M2, M3, or M4).
- macOS with access to the built-in lid angle sensor.

## Installation

### Homebrew

```bash
brew install mishamyrt/tap/lidoff
```

### Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/mishamyrt/lidoff/master/install.sh | bash
```

The quick installer places the binary in `~/.local/bin`. Make sure that directory is in your `PATH`.

### From Source

```bash
git clone https://github.com/mishamyrt/lidoff.git
cd lidoff
rustup toolchain install stable
cargo build --release --package lidoff
mkdir -p "$HOME/.local/bin"
install -m 755 target/release/lidoff "$HOME/.local/bin/lidoff"
```

## Usage

### Foreground

Run `lidoff` to monitor the lid and see logs in your terminal. Press Ctrl+C to stop.

```bash
lidoff
lidoff --threshold 25 --interval 500 --verbose
lidoff --help
```

### Background service

With the Homebrew installation, start the service to run `lidoff` with its default settings and register it to start at login.

```bash
brew services start lidoff
brew services info lidoff
brew services restart lidoff
brew services stop lidoff
```

The service writes its logs to `var/log/lidoff.log` under the Homebrew prefix. This is typically `/opt/homebrew/var/log/lidoff.log` on Apple Silicon or `/usr/local/var/log/lidoff.log` on Intel. Use `brew --prefix` to follow the log on any installation.

```bash
tail -F "$(brew --prefix)/var/log/lidoff.log"
```

## Behavior

`lidoff` classifies lid angle into three states:

- **Partially closed**: `5° <= angle < threshold`. Saves current brightness values, dims the built-in display and keyboard backlight to zero, locks cursor movement, disables external displays when possible, and starts `caffeinate`.
- **Open**: `angle >= threshold`. Restores saved brightness values, restores external displays, and stops `caffeinate`.
- **Fully closed**: `angle < 5°`. Restores saved brightness values, restores external displays, and stops `caffeinate`, allowing normal sleep behavior.

External display shutdown uses the available macOS display controls and falls back where possible. Some monitors or connection types may not support full external brightness control.

## Development

The project is a Rust workspace under `crates/`:

- `crates/lidoff` parses CLI options and starts the monitor.
- `crates/lidoff-daemon` contains monitor state management and runtime orchestration.
- `crates/lidoff-display`, `crates/lidoff-lidsensor`, and `crates/lidoff-power` contain macOS integration shims.

Useful commands:

```bash
cargo build --workspace
make fmt
make lint
make test
make check
```

Development tools:

```bash
brew install llvm
rustup component add rustfmt clippy
```

## License

MIT
