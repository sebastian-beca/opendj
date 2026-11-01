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

### Requirement: Export to a Spotify playlist
Export to Spotify Playlist on one of the DJ's lists, crates or history sessions SHALL make a new private Spotify playlist with the list's tracks in the list's order (a history session in the order played), finding each by its Spotify id, then ISRC, then artist, title and length, and SHALL name the tracks Spotify doesn't have.

#### Scenario: Export a night's history
- **WHEN** the DJ exports a history session of 24 plays, 2 of them not on Spotify
- **THEN** a private Spotify playlist named after the session holds the other 22 in the order played, and the list's header names the 2 left out

### Requirement: History in the order played
A history session SHALL open in the order its tracks were played, with a # column (play order) and a Played At column (the time each play was recorded); one of the DJ's lists SHALL open in its own order. Clicking a column sorts by it, and clicking # goes back to the list's order.

#### Scenario: Open last night's session
- **WHEN** the DJ opens a history session after sorting the collection by BPM
- **THEN** the session lists its tracks 1, 2, 3… as played, with their times, and the collection is still sorted by BPM when they go back to it
