#!/bin/bash
set -euo pipefail

REPO="mishamyrt/lidoff"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="lidoff"
LAUNCH_AGENT_PLIST="$HOME/Library/LaunchAgents/co.myrt.lidoff.plist"

echo "Installing lidoff..."

if [ "$(uname -s)" != Darwin ]; then
    echo "Error: lidoff requires macOS" >&2
    exit 1
fi

case "$(uname -m)" in
    arm64) TARGET="aarch64-apple-darwin" ;;
    x86_64) TARGET="x86_64-apple-darwin" ;;
    *) echo "Error: unsupported architecture" >&2; exit 1 ;;
esac

# Detect currently installed binary (PATH first, then default install location)
CURRENT_BINARY=""
if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    CURRENT_BINARY=$(command -v "$BINARY_NAME")
elif [ -x "$INSTALL_DIR/$BINARY_NAME" ]; then
    CURRENT_BINARY="$INSTALL_DIR/$BINARY_NAME"
fi

# Remember whether LaunchAgent was enabled before update
AGENT_WAS_ENABLED=0
if [ -f "$LAUNCH_AGENT_PLIST" ]; then
    AGENT_WAS_ENABLED=1
fi

if [ "$AGENT_WAS_ENABLED" -eq 1 ] && [ -z "$CURRENT_BINARY" ]; then
    echo "Error: lidoff LaunchAgent is enabled, but binary was not found to uninstall it" >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
DOWNLOAD_DIR=$(mktemp -d "$INSTALL_DIR/.lidoff.XXXXXX")
trap 'rm -rf "$DOWNLOAD_DIR"' EXIT

ARCHIVE_DIR="lidoff-$TARGET"
DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/$ARCHIVE_DIR.tar.xz"

echo "Downloading from $DOWNLOAD_URL"
curl -fsSL "$DOWNLOAD_URL" -o "$DOWNLOAD_DIR/release.tar.xz"
tar -xJf "$DOWNLOAD_DIR/release.tar.xz" -C "$DOWNLOAD_DIR" "$ARCHIVE_DIR/$BINARY_NAME"
"$DOWNLOAD_DIR/$ARCHIVE_DIR/$BINARY_NAME" --version >/dev/null
install -m 755 "$DOWNLOAD_DIR/$ARCHIVE_DIR/$BINARY_NAME" "$DOWNLOAD_DIR/$BINARY_NAME"

if [ "$AGENT_WAS_ENABLED" -eq 1 ]; then
    echo "Uninstalling existing LaunchAgent..."
    "$CURRENT_BINARY" uninstall
fi

mv -f "$DOWNLOAD_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
echo "Installed to $INSTALL_DIR/$BINARY_NAME"

if [ "$AGENT_WAS_ENABLED" -eq 1 ]; then
    echo "Re-enabling LaunchAgent..."
    "$INSTALL_DIR/$BINARY_NAME" install
fi

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH"
    echo "Add this to your shell config:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
