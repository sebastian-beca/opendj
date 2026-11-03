## ADDED Requirements

### Requirement: Drag to move the grid
In Prep, with the grid unlocked, dragging the waveform sideways SHALL move the grid under it, snapping in milliseconds, and save it as edited; with the grid locked dragging SHALL only move the playhead.

#### Scenario: Line up a late kick
- **WHEN** the waveform is dragged right until the kicks meet the beat lines
- **THEN** the grid is saved as edited and sync uses the new grid

### Requirement: Check 4 spots
Check 4 spots SHALL show and play two bars at bar 1 and at about a quarter, half and three quarters of the track with the click on, and show at each how far the kick lands from the beat line ("on the beat", or how many milliseconds early or late).

#### Scenario: Drifting grid
- **WHEN** the offset grows from spot to spot
- **THEN** the tempo can be adjusted with Tempo − + until all four show on the beat

### Requirement: Lock on Play Ready
Moving a track to Play Ready from Prep SHALL lock its grid.

#### Scenario: Ready locks
- **WHEN** Play Ready, next track is pressed
- **THEN** the track's grid is locked and analysis no longer changes it
