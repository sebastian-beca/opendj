## ADDED Requirements

### Requirement: Offloaded mark
Tracks whose file is only in iCloud SHALL show a cloud mark in lists and be searchable as `is:offloaded`.

#### Scenario: Cloud mark
- **WHEN** a file is offloaded by macOS
- **THEN** its row shows a cloud mark

### Requirement: Keep downloaded
The DJ SHALL be able to keep the Collection or chosen lists downloaded, asking macOS to keep those files on disk.

#### Scenario: Pinned crate
- **WHEN** Keep downloaded is turned on for a crate
- **THEN** its files stay on disk after macOS frees space

### Requirement: Pre-gig warning
Loading a list's tracks onto a deck, or opening a set, SHALL warn when any of its tracks are offloaded and offer Download now.

#### Scenario: Warn before playing
- **WHEN** a set has three offloaded tracks
- **THEN** a warning lists them with Download now
