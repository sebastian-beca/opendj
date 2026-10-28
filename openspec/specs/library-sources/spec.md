# library-sources Specification

## Purpose
The rail of sources beside the browser: the Collection, Files on the computer, streaming services and stores, and add-ons' own sources.

## Requirements

### Requirement: Rail
The rail SHALL list Collection, Files, Spotify, TIDAL, Apple Music, Bandcamp (with the other stores in its list), Beatport, OneLibrary and SoundCloud with their official logos, then add-ons' sources. The "…" menu SHALL hide and show sources (`hidden_sources`).

#### Scenario: Hide a source
- **WHEN** SoundCloud is hidden from the "…" menu
- **THEN** it leaves the rail and stays hidden after a restart

### Requirement: Sources not switched on
Every built-in source but Collection and Files SHALL say "Coming soon" until it is switched on for release; add-ons' sources SHALL show whenever the add-on is installed.

#### Scenario: Spotify in a release build
- **WHEN** Spotify is clicked
- **THEN** the list says "Coming soon."

### Requirement: Files source
Files SHALL browse pinned folders, the music folders, Home, Desktop, Downloads, drives and cloud folders (iCloud Drive, Google Drive, Dropbox, OneDrive, Box), reading subfolders on opening and tags on a background thread. Cloud-only files SHALL NOT be downloaded to read tags. A browsed file SHALL join the collection only when it is loaded, dragged, rated, listed or added.

#### Scenario: Browsing doesn't add
- **WHEN** a folder in Downloads is browsed
- **THEN** its files are listed but not in the Collection until one is loaded

### Requirement: Streaming is browse only
A streaming source SHALL never play streamed audio. A streamed track SHALL play only when the collection holds the same track.

#### Scenario: Matched track
- **WHEN** a Spotify playlist track is also in the collection
- **THEN** it can be loaded and plays the local file
