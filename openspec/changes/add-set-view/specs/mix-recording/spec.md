## ADDED Requirements

### Requirement: Frame-accurate control log
Control changes in the control log SHALL be stamped with the audio frame at which they took effect. Plugin parameter changes and instrument notes SHALL be logged.

#### Scenario: Exact time
- **WHEN** a hot cue is pressed during a recording
- **THEN** its log line's time matches the frame where the jump is heard in the recording

### Requirement: Set view
A recording with a control log SHALL open as a timeline with a row per deck, the transitions marked with their length and style, and lanes for the mixer and FX moves, played back in sync with the recording.

#### Scenario: Review a transition
- **WHEN** the DJ opens a recording in Set view and clicks a transition
- **THEN** playback jumps there and the panel shows both tracks, the start, length in bars and style found

### Requirement: Render an edited set
After the DJ moves, stretches or edits transitions or lanes, openDJ SHALL render the set through its engine to a new file beside the recording and SHALL NOT change the recording.

#### Scenario: Unedited render matches
- **WHEN** a recording with no edits is rendered
- **THEN** the render matches the recording within -60 dBFS, apart from third-party plugin effects
