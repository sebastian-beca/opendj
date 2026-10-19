# Install

Download openDJ from the [download page](https://opendj.app/#download) or the
[latest release on GitHub](https://github.com/sebastian-beca/opendj/releases/latest).

## macOS

Apple Silicon and Intel, macOS 11 or newer.

1. Download `openDJ-…-macos.zip`.
2. Unzip it and move **openDJ** to Applications.
3. The app isn't notarised yet, so macOS blocks the first launch. Right-click
   openDJ and choose **Open**, or run:

   ```sh
   xattr -dr com.apple.quarantine /Applications/openDJ.app
   ```

## Linux

x86_64, glibc 2.35 or newer (Ubuntu 22.04+, Fedora 36+, Debian 12+). openDJ
needs ALSA and a Vulkan driver, which most desktops already have.

- **AppImage:** download `openDJ-…-x86_64.AppImage`, make it executable with
  `chmod +x`, and run it.
- **Tarball:** download `opendj-…-linux-x86_64.tar.gz`, unpack it anywhere and
  run `bin/opendj`.

## Windows

Windows isn't supported yet. It's planned for openDJ 2.0.
