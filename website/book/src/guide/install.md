# Install

Download openDJ from the [download page](https://opendj.app/#download) or the
[latest release on GitHub](https://github.com/sebastian-beca/opendj/releases/latest).

## macOS

Apple Silicon and Intel, macOS 11 or newer.

1. Download `openDJ-…-macos.zip`.
2. Unzip it and move **openDJ** to Applications.

The app is signed and notarized by Apple, so it opens like any other app, and
it offers new versions itself when it starts.

## Linux

x86_64, glibc 2.35 or newer (Ubuntu 22.04+, Fedora 36+, Debian 12+). openDJ
needs ALSA and a Vulkan driver, which most desktops already have.

- **AppImage:** download `openDJ-…-x86_64.AppImage`, make it executable with
  `chmod +x`, and run it.
- **Tarball:** download `opendj-…-linux-x86_64.tar.gz`, unpack it anywhere and
  run `bin/opendj`.

## Windows

Windows is a preview from openDJ 0.4: 64-bit Windows 10 or 11.

- **Setup:** download `openDJ-…-windows-x64-setup.exe` and run it. It installs
  openDJ for your user only (no administrator needed), adds it to the Start
  menu, and openDJ updates itself from then on.
- **Portable:** download `opendj-…-windows-x64.zip`, unzip it anywhere and run
  `openDJ.exe`. It doesn't update itself.

The builds aren't signed yet, so on the first start Windows shows "Windows
protected your PC". Click **More info**, then **Run anyway**.

In Settings → Audio, pick an **ASIO** driver if your controller or audio
interface has one, for the lowest latency. Windows lets only one app use a
MIDI controller at a time, so quit any other DJ app before you start openDJ.
