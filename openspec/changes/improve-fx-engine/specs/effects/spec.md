## ADDED Requirements

### Requirement: FX performance budget
The effect rack SHALL NOT allocate on the audio thread. It SHALL process active effects in blocks, and it SHALL reserve delay memory only for effects that need it. A benchmark SHALL report each effect's CPU cost per frame.

#### Scenario: Four decks with effects
- **WHEN** all four decks each run three effects at 48 kHz
- **THEN** the benchmark reports the rack's cost per frame, and no allocation happens on the audio thread

### Requirement: Clean modulated delays
Echo, Flanger and Spiral SHALL read their delay lines with cubic or better interpolation, so sweeps keep their high frequencies and don't zipper.

#### Scenario: Flanger sweep
- **WHEN** a flanger sweeps a 10 kHz tone
- **THEN** the tone's level stays within 1 dB across the sweep

### Requirement: More effects
openDJ SHALL add Drive (oversampled so it doesn't alias), Chorus, a Ladder filter, a Hall reverb, and the Slip Roll, Rev Roll and Helix roll variants. Each SHALL be beat-synced where it has a time setting, and each SHALL have click-free parameters.

#### Scenario: Drive without aliasing
- **WHEN** Drive distorts a 5 kHz sine at full drive
- **THEN** aliased components stay at least 60 dB below the fundamental
