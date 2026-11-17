#!/bin/bash
set -euo pipefail

REPO="mishamyrt/lidoff"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="lidoff"

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

mv -f "$DOWNLOAD_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
echo "Installed to $INSTALL_DIR/$BINARY_NAME"

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH"
    echo "Add this to your shell config:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
