## ADDED Requirements

### Requirement: iTunes and Music import
Settings › Collection › Import SHALL offer Import from iTunes, with the three-step wizard, reading the iTunes or Music app Library XML read-only, kept up to date at start, and removable.

#### Scenario: iTunes library
- **WHEN** an iTunes or Music library is imported
- **THEN** each local music track keeps its tags, BPM, stars, play count, date added and last played, and podcasts, audiobooks and videos stay out

#### Scenario: iTunes playlists
- **WHEN** the Playlists group is ticked
- **THEN** the library's own playlists appear in their folders, without the app's built-in lists, and Apple Music tracks in them show as streamed tracks

#### Scenario: Smart playlists
- **WHEN** the Smart playlists group is ticked
- **THEN** each smart playlist appears as a plain list of the tracks it held

#### Scenario: No Library XML
- **WHEN** no Library XML is found where iTunes or Music keeps one
- **THEN** the wizard says how to export one from the Music app and lets the DJ choose it
