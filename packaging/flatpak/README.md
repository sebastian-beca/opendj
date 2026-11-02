# Flathub

`app.opendj.openDJ.yml` is a draft Flatpak manifest for Flathub. It builds
openDJ from a release tag inside the Flatpak SDK, offline, so every crate
in `Cargo.lock` has to be listed in a `cargo-sources.json` next to it.

## Try it locally

```sh
flatpak install flathub org.freedesktop.Sdk//24.08 \
  org.freedesktop.Sdk.Extension.rust-stable//24.08 \
  org.freedesktop.Sdk.Extension.llvm18//24.08
# The crate list, from flatpak-builder-tools (cargo/flatpak-cargo-generator.py):
python3 flatpak-cargo-generator.py ../../Cargo.lock -o cargo-sources.json
# For a local test, point the git source at this checkout (type: dir, path: ../..).
flatpak-builder --user --install --force-clean build app.opendj.openDJ.yml
flatpak run app.opendj.openDJ
```

## Submit

1. Fill in `tag` and `commit` for the release, regenerate `cargo-sources.json`
   for that tag's `Cargo.lock`.
2. Fork https://github.com/flathub/flathub, branch from `new-pr`, add the
   manifest and `cargo-sources.json`, and open a pull request against `new-pr`.
3. Flathub reviewers will ask about `--device=all` and `--filesystem=home`.
   The answers are in the manifest's comments: USB DJ controllers (MIDI and
   HID) and direct ALSA audio need device access, as for Mixxx's Flatpak;
   music folders and the VirtualDJ/rekordbox/Mixxx imports need home.
4. Once accepted, Flathub creates `flathub/app.opendj.openDJ`. Its bot reads
   `x-checker-data` and opens a pull request for each new GitHub release; the
   crate list has to be regenerated in that pull request when `Cargo.lock`
   changed.
5. Verify the app on Flathub (domain check for opendj.app) so it shows as
   verified.

Inside a Flatpak openDJ doesn't check GitHub for updates (Flathub brings
them), see `crates/app/src/update.rs`.
