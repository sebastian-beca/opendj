## ADDED Requirements

### Requirement: No install needed
Separation SHALL work on a fresh install of openDJ without Demucs or Python.

#### Scenario: Fresh Mac
- **WHEN** SEPARATE STEMS is pressed on a fresh install
- **THEN** the track separates

### Requirement: Separate ahead of time
The DJ SHALL be able to separate a list or set in the background, one track at a time, and see which tracks have stems ready.

#### Scenario: Prepare a set
- **WHEN** Separate stems is chosen for a 20-track list
- **THEN** tracks gain a stems mark as each finishes while the decks keep playing

### Requirement: Speed
A 6-minute track SHALL separate in under a minute on an Apple Silicon Mac.

#### Scenario: M1 timing
- **WHEN** a 6-minute track is separated on an M1
- **THEN** it finishes in under 60 seconds
