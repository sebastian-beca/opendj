# beatgrids Specification

## Purpose
Beat grids whose tempo can change through a track, how they are edited by hand, and how they are shown.

## Requirements

### Requirement: Variable-tempo grids
A grid SHALL be a list of markers, each on a whole beat with its own tempo until the next, so bars and phase stay continuous across tempo changes.

#### Scenario: Bars continue across a change
- **WHEN** a grid has a section at 120 BPM followed by one at 122 BPM
- **THEN** bar numbers carry on across the change without a gap

### Requirement: Grid editing
The edit menu from a deck's BPM SHALL move the grid to the playhead (`beats_translate_curpos`), earlier or later, make beats faster or slower, halve or double the tempo, add or remove a section at the playhead, and undo the last change.

#### Scenario: Halve the tempo
- **WHEN** a track detected at 172 BPM has its grid halved
- **THEN** it shows 86 BPM and its beats fall on every other old beat

#### Scenario: Undo
- **WHEN** a grid is moved and then undone
- **THEN** it is back where it was

### Requirement: Grids saved
Edited grids SHALL be saved with the track in the library, and imported grids SHALL be kept as they are.

#### Scenario: Edited grid kept
- **WHEN** a grid is edited and the track loaded again later
- **THEN** the edited grid is used

### Requirement: Bar numbers
The scrolling waveforms SHALL show a bar number at the top left of each bar, starting at 1 on the first bar.

#### Scenario: Bar numbers
- **WHEN** a track plays
- **THEN** each bar on the scrolling waveform carries its number, sharp at any position
