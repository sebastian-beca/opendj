## MODIFIED Requirements

### Requirement: Rail
The rail SHALL list Collection, Files, Spotify, TIDAL, Apple Music, Bandcamp (with the other stores in its list), Beatport, OneLibrary, SoundCloud and Mixcloud with their official logos, then add-ons' sources. The "…" menu SHALL hide and show sources (`hidden_sources`).

#### Scenario: Hide a source
- **WHEN** SoundCloud is hidden from the "…" menu
- **THEN** it leaves the rail and stays hidden after a restart

### Requirement: Sources not switched on
A built-in source SHALL say "Coming soon" until it is switched on for release; Spotify, TIDAL, Apple Music, SoundCloud and Mixcloud SHALL be switched on in v0.5 once their API access is in place, and the stores in v0.6. Add-ons' sources SHALL show whenever the add-on is installed.

#### Scenario: Spotify in a release build
- **WHEN** Spotify is clicked and the DJ isn't signed in
- **THEN** the list offers Sign in to Spotify

## ADDED Requirements

### Requirement: Streaming playlists
A signed-in streaming source SHALL list the DJ's playlists and liked or favourite tracks; tracks the collection holds (same ISRC, or artist, title and duration within 2 s) SHALL be marked and load the local file, and the rest SHALL be greyed out and not loadable.

#### Scenario: TIDAL playlist
- **WHEN** the DJ opens a TIDAL playlist of 40 tracks, 25 of them in the collection
- **THEN** 25 tracks can be loaded and 15 are greyed out

### Requirement: Copy a streaming playlist
Copy to openDJ list SHALL make an openDJ list of a streaming playlist's matched tracks, and add the unmatched ones to the wanted list.

#### Scenario: Copy
- **WHEN** the DJ copies a Spotify playlist with 3 unmatched tracks
- **THEN** a list with the matched tracks appears, and the 3 others are on the wanted list
