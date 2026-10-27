# performance-pads Specification

## Purpose
The eight performance pads under each deck and their pages: hot cues, pad FX, beat jump, sampler and stems, and how controllers switch between them.

## Requirements

### Requirement: Pad pages
Each deck SHALL have 8 pads in two rows and four pad tabs, each showing one of five pages: HOT CUES, PAD FX, BEAT JUMP, SAMPLER and STEMS. A tab's page SHALL be changeable from its menu.

#### Scenario: Switch page
- **WHEN** the BEAT JUMP tab is clicked
- **THEN** the pads show beat jump sizes

### Requirement: Controller pad modes
A controller's pad mode buttons SHALL open the matching page through `[ChannelN],pad_panel`: the tab already showing it, or else the open tab switched to it.

#### Scenario: FLX4 pad mode
- **WHEN** the FLX4's BEAT JUMP pad mode button is pressed
- **THEN** the screen shows that deck's BEAT JUMP page

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
