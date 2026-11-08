#!/bin/sh
# Installs Horizon Atlas (an unofficial Horizon launcher) on Linux (x86-64) or macOS (Apple Silicon),
# for this user only.
#   curl -fsSL https://raw.githubusercontent.com/nezbut/horizon-atlas/main/install.sh | sh
# Linux: ~/.local/share/horizon-atlas, `horizon-atlas` in ~/.local/bin, and the launcher adds itself to the
# applications menu when it first starts.
# macOS: ~/Applications/Horizon Atlas/Horizon.app, `horizon-atlas` in ~/.local/bin.
# Windows: download horizon-launcher-setup.exe from the releases page instead.
set -eu

REPO="nezbut/horizon-atlas"
BASE="https://github.com/$REPO/releases/latest/download"

case "$(uname -s)" in
  Linux)
    case "$(uname -m)" in
      x86_64 | amd64) ;;
      *)
        echo "Unsupported processor: $(uname -m). Atlas builds the Linux launcher for x86-64 only." >&2
        exit 1
        ;;
    esac
    ASSET="horizon-launcher-linux-x86_64.tar.gz"
    DIR="${XDG_DATA_HOME:-$HOME/.local/share}/horizon-atlas"
    ;;
  Darwin)
    if [ "$(uname -m)" != arm64 ]; then
      echo "This script installs the Apple Silicon build. On an Intel Mac, download horizon-launcher-macos-x64.zip from https://github.com/$REPO/releases/latest" >&2
      exit 1
    fi
    ASSET="horizon-launcher-macos-arm64.zip"
    DIR="$HOME/Applications/Horizon Atlas"
    ;;
  *)
    echo "Unsupported system: $(uname -s). On Windows, download horizon-launcher-setup.exe from https://github.com/$REPO/releases/latest" >&2
    exit 1
    ;;
esac

echo "Horizon Atlas"
echo "  Downloading $ASSET"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fL --progress-bar "$BASE/$ASSET" -o "$TMP/$ASSET"

mkdir -p "$DIR"
case "$ASSET" in
  *.tar.gz)
    tar -xzf "$TMP/$ASSET" -C "$TMP"
    mv -f "$TMP/horizon-launcher" "$DIR/horizon-atlas"
    chmod +x "$DIR/horizon-atlas"
    PROGRAM="$DIR/horizon-atlas"
    ;;
  *.zip)
    (cd "$TMP" && unzip -q "$ASSET")
    rm -rf "$DIR/Horizon.app"
    mv "$TMP/Horizon.app" "$DIR/Horizon.app"
    # macOS asks before opening downloaded programs that aren't notarized.
    xattr -dr com.apple.quarantine "$DIR" 2>/dev/null || true
    PROGRAM="$DIR/Horizon.app/Contents/MacOS/horizon-launcher"
    ;;
esac

mkdir -p "$HOME/.local/bin"
ln -sf "$PROGRAM" "$HOME/.local/bin/horizon-atlas"
echo "  Installed in $DIR"
echo "  Start it with: horizon-atlas"
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) echo "  (~/.local/bin is not on your PATH: add it, or run $PROGRAM)" ;;
esac
echo "  Next: open the launcher, press Prepare (it uses your own Minecraft 26.3) or Choose jar..., then Play."
