#!/bin/sh
# Builds dist/openDJ.app around a compiled binary and zips it as
# dist/opendj-macos.zip. Usage: scripts/bundle-macos.sh BINARY VERSION
set -eu
binary=$1
version=$2

app=dist/openDJ.app/Contents
rm -rf dist/openDJ.app
mkdir -p "$app/MacOS" "$app/Resources"
cp "$binary" "$app/MacOS/opendj"
cp -R res/controllers "$app/Resources/controllers"
# The guided tour's demo tracks, once they are in the repository.
if [ -d res/demo ]; then cp -R res/demo "$app/Resources/demo"; fi

icons=dist/openDJ.iconset
mkdir -p "$icons"
for size in 16 32 128 256 512; do
  sips -z $size $size res/macos/opendj-1024.png --out "$icons/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) res/macos/opendj-1024.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Resources/openDJ.icns"

cat > "$app/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>openDJ</string>
  <key>CFBundleDisplayName</key><string>openDJ</string>
  <key>CFBundleIdentifier</key><string>io.github.sebastian-beca.opendj</string>
  <key>CFBundleExecutable</key><string>opendj</string>
  <key>CFBundleIconFile</key><string>openDJ</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>openDJ plays your microphone and other inputs through the mixer.</string>
  <key>NSLocalNetworkUsageDescription</key><string>Ableton Link shares tempo and beat with other music apps on your network.</string>
</dict>
</plist>
PLIST

# With MACOS_SIGN_IDENTITY set (e.g. "Developer ID Application"), signs for
# notarization: hardened runtime, secure timestamp and the entitlements.
# Without it, an ad-hoc signature, and macOS asks before the first launch.
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp \
    --entitlements res/macos/opendj.entitlements \
    --sign "$MACOS_SIGN_IDENTITY" dist/openDJ.app
  codesign --verify --strict --verbose=2 dist/openDJ.app
else
  codesign --force --sign - dist/openDJ.app
fi
(cd dist && ditto -c -k --keepParent openDJ.app opendj-macos.zip)
