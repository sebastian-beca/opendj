## ADDED Requirements

### Requirement: Key lock on the tempo fader
Each deck's tempo fader SHALL show its slot line in lilac while `keylock` is on, and a right-click on it SHALL toggle `keylock`. Key lock SHALL be on for every deck at start unless "Key lock on at start" is switched off.

#### Scenario: Toggle key lock
- **WHEN** the tempo fader is right-clicked while key lock is off
- **THEN** key lock turns on and the fader's line turns lilac

### Requirement: Key match on load
With "Key match on load" at ±1 or ±2 (default ±1) and another deck playing, a loaded track that isn't compatible SHALL be shifted by the smallest amount within that range that makes it compatible (ties shift down), turning key lock on. Tracks already compatible SHALL NOT be shifted.

#### Scenario: Shift into key
- **WHEN** deck 1 plays 8A and a 3A track loads on deck 2 with ±1
- **THEN** deck 2 is shifted down one semitone to 8A with key lock on

#### Scenario: Already compatible
- **WHEN** a 9A track loads against 8A
- **THEN** it is not shifted
