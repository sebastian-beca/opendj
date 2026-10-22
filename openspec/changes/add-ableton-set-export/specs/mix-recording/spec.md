## ADDED Requirements

### Requirement: Per-deck recording
A recording SHALL be able to also record each deck pre-fader to its own file.

#### Scenario: Deck files
- **WHEN** per-deck recording is on and a set is recorded
- **THEN** one file per used deck is written beside the mix

### Requirement: Export to Live
A recording SHALL export to an Ableton Live set (.als) with the mix, each deck's audio and the MIDI, opening in Live 11 and 12.

#### Scenario: Open in Live
- **WHEN** an exported set is opened in Live 12
- **THEN** it shows the mix and a track per deck aligned in time
