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

### Requirement: Eight tracks
The Groove Box SHALL have eight tracks: kick, snare, closed hat, open hat, clap, rim, cowbell and cymbal. Loading a library groove SHALL put its ride into the cymbal track and set that track to the kit's ride when the groove has a ride, else its crash with the kit's crash. A Groove Box saved with nine rows SHALL move its ninth row into the eighth wherever the eighth has no hits.

#### Scenario: A groove with a ride
- **WHEN** a groove whose ride plays on the off-beats is loaded
- **THEN** track 8 plays the ride on the off-beats

### Requirement: Dice
Setting `[GrooveN],dice` to a move (1 new, 2 mutate, 3 busier, 4 calmer, 5 even out, 6 shift later, 7 shift earlier) SHALL change track `dice_row` (1-8), or every track whose `rowR_lock` is off when `dice_row` is 0, of the groove on screen, at the next bar while playing and at once while stopped. New tracks SHALL follow how the genre picked uses that sound; Mutate, Busier and Calmer SHALL weigh each step by its strength in the bar and the genre's use of it, and SHALL keep the kick's downbeat and the snare's backbeat. `dice_undo` SHALL put back the groove as it was before the last change, up to 16 changes back.

#### Scenario: Vary on the bar
- **WHEN** the Groove Box plays mid-bar and New groove is picked
- **THEN** nothing changes until the next bar line, where every unlocked track plays new hits

#### Scenario: Locked kick
- **WHEN** the kick is locked and Calmer is picked for every track
- **THEN** the kick keeps its hits and the other tracks lose one each
