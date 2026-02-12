#!/bin/bash
set -e

REPO="mishamyrt/lidoff"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="lidoff"
LAUNCH_AGENT_PLIST="$HOME/Library/LaunchAgents/co.myrt.lidoff.plist"

echo "Installing lidoff..."

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

if [ "$AGENT_WAS_ENABLED" -eq 1 ]; then
    if [ -z "$CURRENT_BINARY" ]; then
        echo "Error: lidoff LaunchAgent is enabled, but binary was not found to disable it"
        exit 1
    fi

    echo "Disabling existing LaunchAgent..."
    "$CURRENT_BINARY" --disable
fi

# Create install directory if needed
mkdir -p "$INSTALL_DIR"

# Get latest release download URL
DOWNLOAD_URL=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | \
    grep "browser_download_url.*lidoff" | \
    cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find latest release"
    exit 1
fi

# Download binary
echo "Downloading from $DOWNLOAD_URL"
curl -sL "$DOWNLOAD_URL" -o "$INSTALL_DIR/$BINARY_NAME"

# Make executable
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo "Installed to $INSTALL_DIR/$BINARY_NAME"

if [ "$AGENT_WAS_ENABLED" -eq 1 ]; then
    echo "Re-enabling LaunchAgent..."
    "$INSTALL_DIR/$BINARY_NAME" --enable
fi

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH"
    echo "Add this to your shell config:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
