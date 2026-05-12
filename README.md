<p align="center">
    <img src="./docs/logo.svg" alt="lidoff logo" height="141">
</p>

[![](https://github.com/mishamyrt/lidoff/actions/workflows/build.yml/badge.svg)](https://github.com/mishamyrt/lidoff/actions/workflows/build.yml)

Rust daemon with a minimal native C shim that turns off MacBook display brightness and
enables caffeinate when the lid is partially closed.

## What for?

- To start a long refactoring in Cursor/Claude Code, go for a walk and don't worry about your laptop going into sleep mode.
- To listen to a podcast while falling asleep.
- To set a movie/game to download overnight.

I noticed that I perform a frequent sequence of actions: start an amphetamine session, lower the brightness, then raise the brightness and end the session. Sometimes I forgot about the first step, which led to unexpected freezing of code refactoring with LLM.

When I discovered the ability to read the angle of the MacBook, I thought that this feature was not being used to its full potential. In standard mode, the sensor is used to determine the Boolean state “is the lid open”. Why not add an additional state?

## Installation

**Homebrew:**

```bash
brew install mishamyrt/tap/lidoff
lidoff --enable
```

**Quick install:**

```bash
curl -fsSL https://raw.githubusercontent.com/mishamyrt/lidoff/master/install.sh | bash
lidoff --enable
```

**From source:**

```bash
git clone https://github.com/mishamyrt/lidoff.git
cd lidoff
rustup toolchain install stable
make
make install
lidoff --enable
```

**Development tools:**

```bash
brew install llvm
rustup component add rustfmt clippy
```

Add Homebrew LLVM to your shell `PATH` if you want to invoke `clang-tidy` and `scan-build` directly. The `Makefile` also auto-detects `$(brew --prefix llvm)/bin` for local runs and CI.

## Usage

```
lidoff [-t threshold] [-i interval]  Run daemon
lidoff --enable [-t threshold]      Install as LaunchAgent
lidoff --disable                   Remove LaunchAgent
```

**Options:**

- `-t, --threshold <degrees>` — Lid angle threshold (default: 30)
- `-i, --interval <ms>` — Polling interval (default: 300)
- `-v, --verbose` — Log current lid angle

## How it works

The daemon monitors lid angle and manages display brightness with caffeinate session:

- **Lid partially closed** (angle < threshold, but ≥ 10°): saves current brightness, sets it to 0, starts a caffeinate session to prevent sleep, and disables external displays
- **Lid opened** (angle ≥ threshold): restores saved brightness, restores external display state, and ends caffeinate session
- **Lid fully closed** (angle < 10°): restores brightness, restores external display state, and ends caffeinate session, allowing normal sleep behavior

External display shutdown tries two methods in priority order:

- **Skylight API** — disables the display at the system level
- **DDC/CI + gamma fallback** — sets brightness/contrast to 0 and zeros gamma

Some monitors or ports may not support DDC/CI, in which case only gamma is applied.

This prevents the issue where fully closing the lid would leave the display at zero brightness after unlock.

## Requirements

- MacBook Air or MacBook Pro with Apple Silicon (M2, M3, M4)

## Development

The codebase now lives under `rust/`. Runtime orchestration is implemented in Rust, while the
remaining macOS integration shim is kept in `rust/macos`.

Quality targets:

- `make` — builds the Rust daemon into `build/lidoff`
- `make fmt` — runs `cargo fmt` and `clang-format`
- `make lint` — runs `cargo clippy` and `clang-tidy`
- `make test` — runs Rust unit tests

The current codebase still has deprecation warnings around `CGDisplayIOServicePort`. Those warnings
do not fail the lint/static-analysis gates and should be cleaned up in a separate follow-up change.
