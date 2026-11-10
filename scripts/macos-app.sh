#!/bin/sh
# Builds BeamForge.app (with its icon) in dist/.
# Usage, from the app folder:
#   sh scripts/macos-app.sh            # this Mac's chip
#   sh scripts/macos-app.sh arm        # Apple Silicon (M1/M2/M3/M4)
#   sh scripts/macos-app.sh intel      # Intel Macs
#   sh scripts/macos-app.sh universal  # one app for both
set -e
cd "$(dirname "$0")/.."

ARCH="${1:-host}"
case "$ARCH" in
  arm)       TARGETS="aarch64-apple-darwin" ;;
  intel)     TARGETS="x86_64-apple-darwin" ;;
  universal) TARGETS="aarch64-apple-darwin x86_64-apple-darwin" ;;
  host)      TARGETS="" ;;
  *) echo "Use: arm, intel, universal (or nothing for this Mac)"; exit 1 ;;
esac

APP="dist/BeamForge.app"
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

# 1. compile
BINS=""
if [ -z "$TARGETS" ]; then
  cargo build --release
  BINS="target/release/beamforge"
else
  for T in $TARGETS; do
    rustup target add "$T"
    cargo build --release --target "$T"
    BINS="$BINS target/$T/release/beamforge"
  done
fi

# 2. the bundle's folders
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
if [ "$ARCH" = "universal" ]; then
  lipo -create -output "$APP/Contents/MacOS/BeamForge" $BINS
else
  cp $BINS "$APP/Contents/MacOS/BeamForge"
fi

# 3. the icon: every size macOS asks for, packed into BeamForge.icns
ICONSET="dist/BeamForge.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for S in 16 32 128 256 512; do
  sips -z $S $S assets/icon.png --out "$ICONSET/icon_${S}x${S}.png" >/dev/null
  D=$((S * 2))
  sips -z $D $D assets/icon.png --out "$ICONSET/icon_${S}x${S}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/BeamForge.icns"
rm -rf "$ICONSET"

# 3b. the icon of saved projects (.beamforge files), packed into BeamForgeDocument.icns
mkdir -p "$ICONSET"
for S in 16 32 128 256 512; do
  sips -z $S $S assets/document.png --out "$ICONSET/icon_${S}x${S}.png" >/dev/null
  D=$((S * 2))
  sips -z $D $D assets/document.png --out "$ICONSET/icon_${S}x${S}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/BeamForgeDocument.icns"
rm -rf "$ICONSET"

# 4. Info.plist: name, icon, version, and .beamforge files open with BeamForge
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>BeamForge</string>
  <key>CFBundleDisplayName</key><string>BeamForge</string>
  <key>CFBundleIdentifier</key><string>org.beamforge.BeamForge</string>
  <key>CFBundleExecutable</key><string>BeamForge</string>
  <key>CFBundleIconFile</key><string>BeamForge</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>BeamForge project</string>
      <key>CFBundleTypeExtensions</key><array><string>beamforge</string></array>
      <key>CFBundleTypeIconFile</key><string>BeamForgeDocument</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Owner</string>
      <key>LSItemContentTypes</key><array><string>org.beamforge.project</string></array>
    </dict>
  </array>
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>org.beamforge.project</string>
      <key>UTTypeDescription</key><string>BeamForge project</string>
      <key>UTTypeIconFile</key><string>BeamForgeDocument</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string><string>public.content</string></array>
      <key>UTTypeTagSpecification</key>
      <dict>
        <key>public.filename-extension</key><array><string>beamforge</string></array>
        <key>public.mime-type</key><string>application/x-beamforge</string>
      </dict>
    </dict>
  </array>
</dict>
</plist>
PLIST

# 5. local signature, so macOS lets it start on this Mac
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true

# 6. tell Finder about the app, so .beamforge files show their icon right away
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
[ -x "$LSREG" ] && "$LSREG" -f "$APP" >/dev/null 2>&1 || true

echo "Done: $APP  (drag it into Applications)"
