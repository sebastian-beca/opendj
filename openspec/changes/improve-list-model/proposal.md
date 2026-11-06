# Proposal

Planned for: **v0.6 · Buy it from the artist** (from a review of folders, playlists and crates, 2026-11-05)

## Why

The sidebar has four kinds of list that look alike and follow rules the DJ can't see:

- A crate and a playlist are the same ordered list; a list counts as a crate only because it sits at the top with no folder. So a crate can't go in a folder, and nothing can turn one into the other.
- Nothing can be moved once made.
- Other apps' imported folders look editable, but each import deletes and remakes them, taking along anything the DJ renamed, added or made inside them.
- Playlists mixes the DJ's lists with generated folders (Groupings and Tags) and imports, all with the same folder icon.

## What Changes

- **Imports read-only.** Other apps' lists and Groupings and Tags move under an **Imported** heading, read-only, the app's folder marked with a padlock. **Make These Lists Mine** on that folder makes its lists the DJ's own (they move to Playlists) and the import keeps bringing tracks, cues and grids but no lists. Own lists made inside an imported folder move out to Playlists before any import or removal touches it.
- **Crates as their own kind.** A crate is stored as a crate, not guessed from its place: unordered, each track once, sorted like the Collection, its name in the Crates column. Crates can live in folders (crate folders nest, like Serato subcrates). Right-click any folder: New Crate, New Playlist, New Smart Playlist, New Folder. **Make It a Crate** / **Make It a Playlist** change a list's kind. Crates keep their crate icon (or the one the DJ picks); playlists have a fixed playlist icon, different from a folder's.
- **Drag to move.** Lists and folders drag onto a folder, or onto a heading to go back to its top.

Crates and Tags both stay: tracks are dragged into a crate; a tag is typed on a track.

## Capabilities

### Modified Capabilities

- `music-library`: list kinds, imported lists
- `library-browser`: the sidebar tree

## Out of Scope

- Writing changes back to the other app.
- Merging crates into tag groups.
