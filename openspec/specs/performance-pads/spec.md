# performance-pads Specification

## Purpose
The eight performance pads under each deck and their pages: the DDJ-FLX4's eight pad modes (hot cues, pad FX 1 and 2, beat jump, sampler, keyboard, beat loop and key shift), plus stems and loop rolls, and how controllers switch between them.

## Requirements

### Requirement: Pad pages
Each deck SHALL have 8 pads in two rows and four pad tabs, each showing one of ten pages: HOT CUES, PAD FX 1, BEAT JUMP, SAMPLER, STEMS, BEAT LOOP, PAD FX 2, KEYBOARD, KEY SHIFT and LOOP ROLL (`pad_panel` 0 to 9 in that order). A tab's page SHALL be changeable from its menu.

#### Scenario: Switch page
- **WHEN** the BEAT JUMP tab is clicked
- **THEN** the pads show beat jump sizes

### Requirement: Controller pad modes
A controller's pad mode buttons SHALL open the matching page through `[ChannelN],pad_panel`: the tab already showing it, or else the open tab switched to it.

#### Scenario: FLX4 pad mode
- **WHEN** the FLX4's BEAT JUMP pad mode button is pressed
- **THEN** the screen shows that deck's BEAT JUMP page

#### Scenario: FLX4 stems pads
- **WHEN** the FLX4's KEYBOARD button is pressed while KEYBOARD is already the pad mode
- **THEN** the pads switch to the stems pads (mute stems 1-4, stem FX 5-8) and the screen shows STEMS; pressing it again returns to KEYBOARD

### Requirement: Hot cue pads
On HOT CUES the pads SHALL be hot cues 1 to 8 in their colours, showing names when set.

#### Scenario: Pad shows cue colour
- **WHEN** hot cue 3 is set with a green colour
- **THEN** pad 3 is green

### Requirement: Beat jump pads
On BEAT JUMP the pads SHALL jump back and forward by 4, 8, 16 and 32 beats, matching the DDJ-FLX4's beat jump pads.

#### Scenario: Pad jump
- **WHEN** the "16 ›" pad is pressed
- **THEN** the deck jumps 16 beats forward

### Requirement: Sampler
There SHALL be 8 one-shot samplers (`[Sampler1]` to `[Sampler8]`) with built-in sounds: kick, snare, clap, hat, horn, siren, laser and crash. A pad SHALL play its sound once from the start; samplers SHALL join the main mix after the crossfader.

#### Scenario: Air horn
- **WHEN** the Horn pad is pressed
- **THEN** the horn plays once in the master and the pad lights while it plays

### Requirement: Beat loop pads
On BEAT LOOP the pads SHALL start or end a loop of 1/4, 1/2, 1, 2, 4, 8, 16 and 32 beats, lit while that loop plays, matching the DDJ-FLX4's BEAT LOOP pads.

#### Scenario: Quarter-beat loop
- **WHEN** the "1/4" pad is pressed
- **THEN** a quarter-beat loop starts at the play position and the pad lights

### Requirement: Loop roll pads
On LOOP ROLL each pad SHALL loop 1/16, 1/8, 1/4, 1/2, 1, 2, 4 or 8 beats while held; on release the track SHALL carry on from where it would have been (slip).

#### Scenario: Roll and return
- **WHEN** the "1/8" pad is held for two beats and released
- **THEN** the deck plays from two beats after where the roll started

### Requirement: Keyboard pads
On KEYBOARD the pads (`keyboard_N_activate`) SHALL play the last hot cue pressed (else the first hot cue set, else the cue point) from its start, with the key shifted 0, +1, +2, +3 semitones on the top row and −4, −3, −2, −1 on the bottom row. The pad of the key shift playing SHALL be lit, and the pads SHALL take the colour of that hot cue.

#### Scenario: Play a cue a tone up
- **WHEN** hot cue 3 was pressed last and the "+2" pad is pressed
- **THEN** the deck plays from hot cue 3 two semitones higher

### Requirement: Key shift pads
On KEY SHIFT the pads SHALL set the key shift to 0, +1, +2, +3 (top row) and −3, −2, −1, 0 (bottom row), and with Shift move it by +1, +2, +3, +4 and −4, −3, −2, −1, as the DDJ-FLX4 mapping does.

#### Scenario: Key up
- **WHEN** the "+1" pad is pressed
- **THEN** the deck plays one semitone higher, at the same tempo
