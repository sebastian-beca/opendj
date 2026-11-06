## MODIFIED Requirements

### Requirement: Pad pages
Each deck SHALL have 8 pads in two rows and four pad tabs, each showing one of twelve pages: HOT CUES, PAD FX 1, BEAT JUMP, SAMPLER, STEMS, BEAT LOOP, PAD FX 2, KEYBOARD, KEY SHIFT, LOOP ROLL, SLICER and FREEZE (`pad_panel` 0 to 11 in that order). A tab's page SHALL be changeable from its menu.

#### Scenario: Switch page
- **WHEN** the BEAT JUMP tab is clicked
- **THEN** the pads show beat jump sizes

## ADDED Requirements

### Requirement: Slicer
A SLICER page SHALL loop the 8 beats from the beat playing when it opens (`[ChannelN],slicer_enabled`), one beat on each pad, with the pad of the beat playing lit (`slicer_N_status`). Holding a pad (`slicer_N_activate`) SHALL play its beat at the same place in the beat, repeating while held; letting go SHALL return to where the section would have been. Leaving the page SHALL end the slicer and the track SHALL play on. A slicer pad SHALL start the slicer when it is off.

#### Scenario: Slice
- **WHEN** pad 5 is held on the slicer
- **THEN** the fifth beat of the section plays, in time, until it is let go

#### Scenario: Back in the section
- **WHEN** a slicer pad is let go
- **THEN** playback continues where the section would be had the pad not been pressed

### Requirement: Freeze
A FREEZE page SHALL freeze the beat playing when the first pad goes down into 8 slices of an eighth of a beat. Holding a pad (`freeze_N_activate`) SHALL repeat its slice; letting go of all of them SHALL carry on from where the track would have been (slip).

#### Scenario: Stutter
- **WHEN** a freeze pad is held
- **THEN** the slice repeats until released
