#!/bin/sh
# Builds BeamForge and installs it for the current user, with its icon in the applications menu.
# Usage, from the app folder on Linux:  sh scripts/install-linux.sh
set -e
cd "$(dirname "$0")/.."

cargo build --release

BIN="$HOME/.local/bin"
ICONS="$HOME/.local/share/icons/hicolor"
APPS="$HOME/.local/share/applications"
mkdir -p "$BIN" "$APPS"

# the program
install -m 755 target/release/beamforge "$BIN/beamforge"

# the icon in the sizes desktops look for (ImageMagick's "convert" if present, else 1024 px only)
for S in 48 64 128 256 512; do
  mkdir -p "$ICONS/${S}x${S}/apps"
  if command -v convert >/dev/null 2>&1; then
    convert assets/icon.png -resize ${S}x${S} "$ICONS/${S}x${S}/apps/beamforge.png"
  fi
done
mkdir -p "$ICONS/1024x1024/apps"
cp assets/icon.png "$ICONS/1024x1024/apps/beamforge.png"

# the icon of saved projects (.beamforge files) and their file type
MIME="$HOME/.local/share/mime"
mkdir -p "$ICONS/1024x1024/mimetypes" "$MIME/packages"
cp assets/document.png "$ICONS/1024x1024/mimetypes/application-x-beamforge.png"
for S in 48 64 128 256; do
  mkdir -p "$ICONS/${S}x${S}/mimetypes"
  if command -v convert >/dev/null 2>&1; then
    convert assets/document.png -resize ${S}x${S} "$ICONS/${S}x${S}/mimetypes/application-x-beamforge.png"
  fi
done
install -m 644 packaging/linux/beamforge-mime.xml "$MIME/packages/beamforge.xml"
command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$MIME" || true

# the menu entry
install -m 644 packaging/linux/beamforge.desktop "$APPS/beamforge.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$ICONS" || true

echo "Installed: $BIN/beamforge — BeamForge is in the applications menu."
echo "If 'beamforge' is not found in a terminal, add ~/.local/bin to your PATH."
