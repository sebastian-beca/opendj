# tempo-and-key Specification

## Purpose
How a deck's speed and musical key are changed: the tempo fader and its range and direction, pitch bends, the pitch reset triangle, key lock and key shift.

## Requirements

### Requirement: Tempo fader range
The tempo fader SHALL change speed within a range chosen in Settings › Decks (±4, 6, 8, 10, 16, 24, 50 or 100 %), applied to every deck (`rateRange`).

#### Scenario: Range applies
- **WHEN** the range is set to ±8 % and the fader is moved fully to the faster end
- **THEN** the deck plays 8 % faster

### Requirement: Tempo fader direction
The tempo fader direction SHALL follow the "Tempo fader direction" setting: "Up slows down" (Pioneer style, the default, `rate_dir` -1) or "Up speeds up". The on-screen fader and controllers SHALL follow the same direction.

#### Scenario: Pioneer direction
- **WHEN** the default direction is used and the fader moves up
- **THEN** the deck slows down

### Requirement: Pitch bends
`rate_temp_up`/`_down` SHALL bend speed by 4 % while held and the `_small` variants by 2 %; `rate_perm_*` SHALL change the tempo setting in steps.

#### Scenario: Temporary bend
- **WHEN** `rate_temp_up` is held and released
- **THEN** the deck plays 4 % faster while held and returns to its tempo after

### Requirement: Pitch reset
A triangle beside each tempo fader SHALL reset the tempo to 0 %: a click glides back at 0.5 % per second (a second click stops the glide), a double click jumps straight to 0 %. Controllers reach these as `rate_reset` and `rate_set_zero`.

#### Scenario: Glide to zero
- **WHEN** a deck at +4 % has its triangle clicked
- **THEN** the tempo glides to 0 % over about eight seconds

#### Scenario: Jump to zero
- **WHEN** the triangle is double-clicked
- **THEN** the tempo is 0 % at once

### Requirement: Key lock
With key lock on (`keylock`), a deck SHALL keep its original key at any tempo. Settings › Decks "Key lock on at start" SHALL set key lock on every deck at launch.

#### Scenario: Key held when speeding up
- **WHEN** key lock is on and the tempo is raised 8 %
- **THEN** the track plays faster at its original key

### Requirement: Key shift
A deck's key SHALL be shiftable in semitones (`pitch`, `pitch_up`, `pitch_down`, `pitch_adjust`), and `reset_key` SHALL return it to the file's key. `sync_key` SHALL shift the deck to the other deck's key or its relative.

#### Scenario: Match keys
- **WHEN** `sync_key` is pressed on a deck in 8A while the other plays 9A
- **THEN** the deck is shifted to play in 9A or a key compatible with it

### Requirement: Key display
The deck SHALL show the key it is actually playing (after shifts, and after tempo when key lock is off), in Camelot notation by default or as note names per Settings › Analysis "Key notation".

#### Scenario: Shifted key shown
- **WHEN** a track in 8A is shifted up one semitone
- **THEN** the deck shows 3A
