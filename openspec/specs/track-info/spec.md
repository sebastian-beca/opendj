# track-info Specification

## Purpose
Viewing and editing a track's details in the side panel's Info tab, writing tags to the file, and finding cover art.

## Requirements

### Requirement: Info tab
The Info tab SHALL follow the selected track or a chosen deck and show editable Track, Artist, Remix, Album, Year, Genre, Track #, Grouping, BPM, Key, Gain, Rating, the tag fields and Comment; then duration, play count, first seen and last played; then format, bitrate, size and Show in Finder.

#### Scenario: Edit a title
- **WHEN** the Track field is changed and confirmed
- **THEN** the list shows the new title

### Requirement: Tags written to the file
Edits to the file's own tags (title, artist, album, year, genre, rating and the like) SHALL be written to the file as well as the library; tag fields and gain SHALL stay library-only. Rewriting tags SHALL keep the track's analysis and stems.

#### Scenario: Rating in the file
- **WHEN** a track is rated 4 stars
- **THEN** the file's tags carry the rating and it isn't re-analysed on next load

### Requirement: Cover art search
Right-clicking a cover (or clicking a missing one) SHALL open a dialog offering covers from the file's tags, its folder, Cover Art Archive, iTunes Search and Discogs (with a token in Settings › Library).

#### Scenario: Pick a cover
- **WHEN** a found cover is chosen
- **THEN** the track's cover changes in every list and on loaded decks
