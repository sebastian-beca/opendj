#!/bin/sh
# Packages a compiled binary for Linux as dist/opendj-VERSION-linux-x86_64.tar.gz
# and dist/openDJ-VERSION-x86_64.AppImage. Usage: scripts/bundle-linux.sh BINARY VERSION
# Needs ImageMagick (convert) and network access to fetch appimagetool.
set -eu
binary=$1
version=$2
root=$(pwd)
mkdir -p dist

# A usr/ tree, the same layout as an install under /usr or /usr/local.
appdir=dist/AppDir
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/opendj" \
  "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/256x256/apps"
cp "$binary" "$appdir/usr/bin/opendj"
strip "$appdir/usr/bin/opendj"
cp -R res/controllers "$appdir/usr/share/opendj/controllers"
# The guided tour's demo tracks, once they are in the repository.
if [ -d res/demo ]; then cp -R res/demo "$appdir/usr/share/opendj/demo"; fi
convert res/macos/opendj-1024.png -resize 256x256 "$appdir/usr/share/icons/hicolor/256x256/apps/opendj.png"
cat > "$appdir/usr/share/applications/opendj.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=openDJ
Comment=DJ software
Exec=opendj
Icon=opendj
Categories=AudioVideo;Audio;
Terminal=false
DESKTOP

# Tarball: unpack anywhere and run opendj/bin/opendj.
tarname=opendj-$version-linux-x86_64
rm -rf "dist/$tarname"
cp -R "$appdir/usr" "dist/$tarname"
cp README.md LICENSE NOTICE "dist/$tarname/"
tar -C dist -czf "dist/$tarname.tar.gz" "$tarname"

# AppImage: one file, chmod +x and run. System libraries (ALSA, fontconfig,
# Vulkan, X11/Wayland) come from the host, as for any desktop app.
cp "$appdir/usr/share/applications/opendj.desktop" "$appdir/opendj.desktop"
cp "$appdir/usr/share/icons/hicolor/256x256/apps/opendj.png" "$appdir/opendj.png"
cat > "$appdir/AppRun" <<'APPRUN'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/opendj" "$@"
APPRUN
chmod +x "$appdir/AppRun"
tool=dist/appimagetool
if [ ! -x "$tool" ]; then
  curl -fsSL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$tool"
fi
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$root/dist/openDJ-$version-x86_64.AppImage"
