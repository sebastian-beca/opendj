## ADDED Requirements

### Requirement: Keymixing mode
Settings › Analysis SHALL offer Harmonic (same key, relative, and ±1 on the same letter) and Fuzzy (letters ignored, number ±1, six keys). `~key:`, `~deck` and the Key chip SHALL follow it.

#### Scenario: Fuzzy matches
- **WHEN** the mode is Fuzzy and the reference is 8A
- **THEN** 8B, 7B and 9B are listed as compatible as well as 7A and 9A

### Requirement: Near-miss marks
Keys one semitone from a compatible key SHALL show +1 or −1 in dimmer lilac, and two semitones ±2 in gray, against the key the reference deck actually plays.

#### Scenario: One semitone off
- **WHEN** the reference plays 8A and a track is in 3A
- **THEN** the track's key shows a −1 mark
