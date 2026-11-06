## MODIFIED Requirements

### Requirement: Control log
Beside the recording, `<name>.controls.jsonl` SHALL hold one line per control change, stamped with the time in the recording, one line per track loaded on a deck, and `pos` lines with each loaded deck's place in its track: at least once a second and whenever the deck jumps. Loop, cue point and hot cue positions SHALL be logged when they change.

#### Scenario: Fader move logged
- **WHEN** a channel fader moves during a recording
- **THEN** the log has a `[ChannelN],volume` line with its time

#### Scenario: Seek logged
- **WHEN** the DJ clicks the waveform to jump to another place in the track during a recording
- **THEN** the log has a `pos` line for that deck with the new place, stamped with the time of the jump

## ADDED Requirements

### Requirement: Export a set list to Live
A History set, playlist or crate SHALL export to an Ableton Live set (.als) that opens in Live 11 and 12, with each track as a warped clip in play order, a tempo envelope and a locator per track.

#### Scenario: Open in Live
- **WHEN** a History set of ten tracks is exported and opened in Live 12
- **THEN** it shows the ten tracks as warped clips in order on alternating tracks, each with a named locator

### Requirement: Export a recording to Live
A recording SHALL export to a Live set with the mix, a locator per track from its cue sheet and, when its control log allows, one track per deck with clips and automation for volume, crossfader, EQ and filter.

#### Scenario: Rebuilt set lines up
- **WHEN** a recorded set with hot cue jumps and a loop is exported and opened in Live
- **THEN** the deck tracks played together sound like the recorded mix on the reference track
