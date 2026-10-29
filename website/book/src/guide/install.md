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

## First start: the guided tour

The first time openDJ opens, a short guided tour sets it up with you. Every step
can be skipped.

1. **Sound:** pick where the master (your speakers) and the headphones play,
   and play a test tone in each. A controller with its own sound card, such as
   the DDJ-FLX4, is its own device: pick it for both, on channels 1 & 2 and
   3 & 4.
2. **Controller** (only when one is plugged in): openDJ shows which mapping it
   runs. Press a button or move a knob and the tour says it heard it.
3. **Your music:** add your music folders. If openDJ finds another DJ app's
   library, such as VirtualDJ's, it offers to bring it in.
4. **Your first mix:** a hands-on lesson on two demo tracks. It switches to
   the 2-Deck layout, rings each control in turn and waits for you to use it,
   on screen or on your controller: load, play and cue, listen in the
   headphones, sync, bring the second track in with its fader and EQ, then a
   bass swap.

**Settings → Help** runs the tour again, whole or one part. If you used openDJ
before the tour existed, it's offered once in a banner under the top bar.
