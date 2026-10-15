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
  <key>NSLocalNetworkUsageDescription</key><string>Ableton Link shares tempo and beat with other music apps on your network.</string>
</dict>
</plist>
PLIST

# Ad-hoc signature only: macOS still asks before the first launch.
codesign --force --deep --sign - dist/openDJ.app
(cd dist && ditto -c -k --keepParent openDJ.app opendj-macos.zip)
