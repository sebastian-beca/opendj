## ADDED Requirements

### Requirement: Track menu
Right-clicking a track SHALL offer its rating, Analyze Track ("Analyze N Tracks" for a selection), Add to Automix, the stage moves, Show in Finder and the DJ's lists. It SHALL NOT offer a colour; colours tracks already have SHALL still show at the row's edge.

#### Scenario: Several tracks
- **WHEN** the DJ shift-clicks 12 tracks, right-clicks and picks Analyze 12 Tracks
- **THEN** all 12 are queued for analysis

### Requirement: Analyze All
Right-clicking a list, folder, smart list, stage or crate in the tree SHALL offer Analyze All, which queues every track in it.

#### Scenario: Folder
- **WHEN** the DJ picks Analyze All on a folder of lists
- **THEN** every track in its lists is queued once

### Requirement: Analysis progress
While tracks are queued, each one's row SHALL show a dot beside the title area: dim while waiting, pulsing lilac while being analysed. The footer SHALL show "Analyzing N of M" and a ✕ that stops the rest.

#### Scenario: Progress
- **WHEN** 12 tracks are queued and 2 are done
- **THEN** the footer reads "Analyzing 3 of 12"
