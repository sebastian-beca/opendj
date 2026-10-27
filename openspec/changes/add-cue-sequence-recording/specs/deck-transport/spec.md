## ADDED Requirements

### Requirement: Record a sequence
A deck SHALL record hot cue jumps and censors, in beats, between REC and stop, and save the sequence with the track (up to 6 per track).

#### Scenario: Record
- **WHEN** the DJ records three cue jumps
- **THEN** the sequence is saved with the track

### Requirement: Play a sequence
Playing a sequence SHALL repeat the jumps at the same beats, looping it if set.

#### Scenario: Replay
- **WHEN** a saved sequence plays
- **THEN** the deck jumps at the recorded beats
