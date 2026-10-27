## ADDED Requirements

### Requirement: Slip everywhere
With slip on, hot cue jumps, pad FX, slicer and scratching SHALL return to where the track would have been when released.

#### Scenario: Hot cue slip
- **WHEN** slip is on and a hot cue is held for a bar
- **THEN** on release the track continues a bar past where it left

### Requirement: Ghost playhead
While slipping, the waveforms SHALL show the hidden playhead.

#### Scenario: Ghost
- **WHEN** a loop runs with slip on
- **THEN** a faint playhead moves on beyond the loop
