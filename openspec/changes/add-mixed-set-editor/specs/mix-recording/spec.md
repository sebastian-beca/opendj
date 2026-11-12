## ADDED Requirements

### Requirement: Mixed Set Editor
A recording with a control log SHALL open as a timeline with a row per deck, the transitions marked with their length and style, loops and hard moves marked, and lanes for the mixer and FX moves, played back in sync with the recording. A list of its transitions SHALL sit beside the timeline.

#### Scenario: Review a transition
- **WHEN** the DJ opens a recording in the Mixed Set Editor and clicks a transition in the list
- **THEN** the timeline scrolls to it, playback starts 4 bars before it, and the panel shows both tracks, the start, length in bars and style found

#### Scenario: Recording without a log
- **WHEN** the DJ opens a recording that has no control log
- **THEN** the editor shows the recording's waveform and the tracklist from its cue sheet
