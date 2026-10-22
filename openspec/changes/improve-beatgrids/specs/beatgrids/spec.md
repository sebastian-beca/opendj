## ADDED Requirements

### Requirement: Edit on the waveform
A grid edit mode SHALL let the DJ drag a beat marker to move the grid, tap to set the tempo, and click to set the downbeat.

#### Scenario: Set downbeat
- **WHEN** the DJ clicks a beat in edit mode and chooses Downbeat
- **THEN** bar 1 starts there

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
