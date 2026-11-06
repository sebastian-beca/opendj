## 1. Native PipeWire (one PR)

- [ ] 1.1 cpal `pipewire` feature on Linux; `realtime` -> `realtime-dbus`
- [ ] 1.2 Release workflow builds the PipeWire client library on Ubuntu 22.04 and bundles it in AppImage, tarball, .deb, .rpm
- [ ] 1.3 Flatpak permission `--filesystem=xdg-run/pipewire-0`
- [ ] 1.4 Settings › Audio: PipeWire devices by name, PipeWire in the status line, raw cards under "ALSA: "
- [ ] 1.5 CI smoke test against PipeWire 0.3.48 on Ubuntu 22.04

## 2. Follow-ups

- [ ] 2.1 Skip drop-or-pad drift handling when every route is on PipeWire
- [ ] 2.2 With add-deck-outputs: unconnected multichannel output, a channel pair per deck
