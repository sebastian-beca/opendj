## ADDED Requirements

### Requirement: Edit on the waveform
A grid edit mode SHALL let the DJ drag a beat marker to move the grid, tap to set the tempo, click to set the downbeat, halve or double the tempo, add or remove a tempo change, and undo.

#### Scenario: Set downbeat
- **WHEN** the DJ clicks a beat in edit mode and chooses Downbeat
- **THEN** bar 1 starts there

### Requirement: Saved grids
An edited grid SHALL be saved with the track and marked as edited; the DJ SHALL be able to lock it, and to go back to the analysed grid.

#### Scenario: Back to analysed
- **WHEN** the DJ chooses Use Analysed Grid on an edited track
- **THEN** the analysed grid comes back and the track is no longer marked as edited

### Requirement: Grid lock
A locked grid SHALL never be changed by analysis or an import.

#### Scenario: Locked
- **WHEN** a locked track is re-analysed
- **THEN** its grid stays

### Requirement: Unsure warning
Tracks whose beats drift from the grid SHALL be marked unsure on the deck and searchable as `is:gridunsure`.

#### Scenario: Drifting
- **WHEN** a track drifts over 10 ms from its grid
- **THEN** it is marked unsure
