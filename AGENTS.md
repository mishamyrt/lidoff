# AGENTS.md

## Commands

- Build the shipped CLI with `cargo build --release -p lidoff` or `make build`; `cargo build --workspace` is the broader dev build.
- CI quality gates are `make lint` (`cargo clippy --all`) and `make test` (`cargo test --workspace`) on `macos-latest`.
- `make check` runs tests first, then `cargo fmt --all --check`, then clippy; run it before handing off code changes when feasible.
- `make fmt` runs `cargo fmt --all` and `clang-format -i` over `crates/**/*.c` and `crates/**/*.h`; install `clang-format` via LLVM if it is missing.
- Focused checks: `cargo test -p lidoff-daemon`, `cargo test -p lidoff-daemon recovery_state::tests::legacy_v2_state_decodes_without_keyboard_backlight_state`, or `cargo clippy -p lidoff-daemon`.

## Architecture

- This is a Rust 2024 workspace with all real crates under `crates/`; workspace lint settings live in root `Cargo.toml`.
- `crates/lidoff` is intentionally thin: clap CLI parsing, LaunchAgent install/uninstall, cache path resolution, and delegation to `lidoff-daemon`.
- `crates/lidoff-daemon` owns the monitor loop, lid-state transitions, power-event handling, display effects, and persisted recovery state.
- `crates/lidoff-display`, `crates/lidoff-lidsensor`, and `crates/lidoff-power` are macOS native-integration wrappers with C shims compiled from each crate's `build.rs`.
- Current CLI subcommands are `install`, `uninstall`, and `run`; options such as `--threshold`, `--interval`, and `--verbose` are global and must appear before the subcommand.

## macOS Gotchas

- Build/test/linking expects macOS frameworks from the native shims (`CoreFoundation`, `CoreGraphics`, `IOKit`, `objc`); QA intentionally runs on macOS.
- Running the daemon for real requires an Apple Silicon MacBook lid-angle sensor; unit tests avoid that hardware path, but `lidoff-daemon::run` returns false if sensor initialization fails.
- Recovery state is a bincode `state.bin` under `~/Library/Caches/co.myrt.lidoff`; keep v1/v2 decode compatibility tests if changing persisted state, and note that old `state.plist` is only removed as legacy cleanup.

## Release Notes

- Releases are configured by `dist-workspace.toml` for cargo-dist `0.31.0`, Homebrew publishing to `mishamyrt/homebrew-tap`, and `aarch64-apple-darwin` plus `x86_64-apple-darwin` targets.
- `Makefile` has a `publish` target that edits versions, updates `Cargo.lock`, runs `git-cliff`, amends, tags, and pushes; do not run it unless explicitly asked.
- `install.sh` and `crates/lidoff/README.md` currently contain older `--enable`/`--disable` style examples; prefer the clap source/root README unless you are fixing those references.
