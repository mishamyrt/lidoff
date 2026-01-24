#!/bin/bash
set -e

REPO="mishamyrt/lidoff"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="lidoff"

echo "Installing lidoff..."

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

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH"
    echo "Add this to your shell config:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
