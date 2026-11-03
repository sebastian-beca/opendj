## ADDED Requirements

### Requirement: Suggested cues from phrases
Suggest from phrases SHALL propose hot cues in empty slots from the track's phrases: the first beat or the end of the intro, whichever comes first; where all musical elements are in; the drop; and the start of the last phrase before the outro. Suggestions SHALL show dashed until kept (K, or Keep) and SHALL never replace a cue the DJ set or imported.

#### Scenario: Keep suggestions
- **WHEN** a track with three phrases and no hot cues gets suggestions and K is pressed
- **THEN** the suggested cues become hot cues saved for the track

### Requirement: Memory cues in Prep
In Prep, Memory cue SHALL add an unnumbered cue at the playhead, shown on the waveform and in the Cues tab with hot cues and saved loops, each renamable.

#### Scenario: Mark a mix point
- **WHEN** Memory cue is pressed at bar 9
- **THEN** a memory cue at bar 9 shows on the waveform and in the Cues tab
