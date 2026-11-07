## Purpose

How the Groove Box keeps time, plays in key, changes its patterns and plays fills.

## ADDED Requirements

### Requirement: Own tempo
The Groove Box SHALL follow the leading deck (else Link) while `[GrooveN],follow` is on (the default), at `tempo_scale` (0.5, 1 or 2) times its tempo and beat count, and SHALL keep its own `bpm` while `follow` is off. Changing the BPM from the screen SHALL turn `follow` off.

#### Scenario: Half time
- **WHEN** a deck plays at 126 BPM and the Groove Box's `tempo_scale` is 0.5
- **THEN** the Groove Box plays at 63 BPM, its bar lines on the deck's bar lines

#### Scenario: Own tempo
- **WHEN** `follow` is off and `bpm` is 100 while a deck plays at 126 BPM
- **THEN** the Groove Box plays at 100 BPM

### Requirement: Kick in key
With `[GrooveN],kick_key` at 1 (root) or 2 (fifth) and a leading deck with a key, the kick SHALL play at the nearest note with that pitch class, at most six semitones from the kit's kick's own pitch, on top of its PITCH knob; `kick_note` SHALL show the note (1-12 from C, 0 untuned).

#### Scenario: Root of C
- **WHEN** the kick rings at A and the leading deck plays in C minor with KEY on Root
- **THEN** the kick plays three semitones higher, at C
