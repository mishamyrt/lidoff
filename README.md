<p align="center">
    <img src="./docs/logo.svg" alt="lidoff logo" height="130">
</p>

[![](https://github.com/mishamyrt/lidoff/actions/workflows/build.yml/badge.svg)](https://github.com/mishamyrt/lidoff/actions/workflows/build.yml)

Daemon that turns off MacBook display brightness and enables caffeinate when the lid is partially closed.

## What for?

- To start a long refactoring in Cursor/Claude Code, go for a walk and don't worry about your laptop going into sleep mode.
- To listen to a podcast while falling asleep.
- To set a movie/game to download overnight.

I noticed that I perform a frequent sequence of actions: start an amphetamine session, lower the brightness, then raise the brightness and end the session. Sometimes I forgot about the first step, which led to unexpected freezing of code refactoring with LLM.

When I discovered the ability to read the angle of the MacBook, I thought that this feature was not being used to its full potential. In standard mode, the sensor is used to determine the Boolean state “is the lid open”. Why not add an additional state?

## Installation

**Quick install:**

```bash
curl -fsSL https://raw.githubusercontent.com/mishamyrt/lidoff/master/install.sh | bash
lidoff --enable
```

**From source:**

```bash
git clone https://github.com/mishamyrt/lidoff.git
cd lidoff
make
make install
lidoff --enable
```

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

External display shutdown tries three methods in priority order:

- **Skylight API** — disables the display at the system level
- **MonitorPanel mirroring** — mirrors the display to a dummy/virtual target
- **DDC/CI + gamma fallback** — sets brightness/contrast to 0 and zeros gamma

Some monitors or ports may not support DDC/CI, in which case only gamma is applied.

This prevents the issue where fully closing the lid would leave the display at zero brightness after unlock.

## Requirements

- MacBook Air or MacBook Pro with Apple Silicon (M2, M3, M4)
