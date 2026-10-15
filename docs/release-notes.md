The first public build of openDJ, for macOS and Linux.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): download `openDJ-…-macos.zip`, unzip it and move openDJ to Applications. The app isn't notarised yet, so macOS blocks the first launch. Either right-click openDJ and choose Open, or run:

```sh
xattr -dr com.apple.quarantine /Applications/openDJ.app
```

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `openDJ-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it.
- `opendj-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/opendj`.

openDJ needs ALSA and a Vulkan driver, which most desktops already have.

openDJ reads your `~/Music` folder. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Builds aren't signed yet.
- No Windows build yet.
- Streaming services and stores are coming soon.

Please report bugs in [Issues](https://github.com/sebastian-beca/opendj/issues).
