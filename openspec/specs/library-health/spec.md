# library-health Specification

## Purpose
Library Health finds what needs fixing across the library and fixes it in batches: not music, strays, files outside the music folders, duplicates, unanalysed, missing info, no cover, no rating, missing files and long tracks.

## Requirements

### Requirement: Health page
Library Health, under All Tracks, SHALL show one line per check with how many tracks it finds; Review lists them, the bar above the list fixes all, and the right-click menu fixes a selection.

#### Scenario: Review a check
- **WHEN** Review is pressed on No rating
- **THEN** the unrated tracks are listed

### Requirement: Checks
The checks SHALL be: Not music (audiobooks, podcasts, spoken word, `.m4b`, chapters), Strays (outside the stage folders), Outside folders (files outside the music folders and stage folders, `is:outside`), Duplicates, Not analyzed, Missing info (artist, title, album, year or genre), No cover, No rating, Missing files and Long tracks (over 20 minutes). Each SHALL also be a search (`is:notmusic` and so on).

#### Scenario: Audiobook found
- **WHEN** an `.m4b` file is in the library
- **THEN** it is listed under Not music

### Requirement: Fixes
Fixes SHALL be: Remove from library or It's music; Move to Triage; Analyze (background); Fill from file name (`Artist - Title`, written to the file); find a cover; Find files (same file name in the stage and music folders); Tag as #mix.

#### Scenario: Fill from file name
- **WHEN** Fill from file name runs on `Bicep - Glue.mp3` with empty tags
- **THEN** artist is Bicep and title Glue, in the library and the file

### Requirement: Ignore and exclusion
"Ignore for" SHALL stop a check showing a track. A not-music track SHALL be left out of the other checks.

#### Scenario: Audiobook not missing a cover
- **WHEN** an audiobook has no cover
- **THEN** it shows under Not music only

### Requirement: Removal keeps files
Remove from library SHALL keep the file; rescans and imports SHALL NOT bring it back, adding the file again or Bring back SHALL.

#### Scenario: Rescan doesn't restore
- **WHEN** a removed track's folder is rescanned
- **THEN** the track stays out of the library

### Requirement: Cover scan respects iCloud
Covers SHALL be looked for in the background, a file only again once it changes, and on macOS files iCloud has taken off the disk SHALL be skipped.

#### Scenario: Offloaded file
- **WHEN** a file is only in iCloud
- **THEN** the cover check doesn't download it
