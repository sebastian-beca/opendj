## ADDED Requirements

### Requirement: Mixed Set Editor
A recording with a control log SHALL open in a simple view (its waveform with a block per track and the transitions marked) and, with Show decks on, as a timeline with a row per deck, the transitions marked with their length and style, loops and hard moves marked, and lanes for the mixer and FX moves, played back in sync with the recording. A list of its transitions SHALL sit beside it.

#### Scenario: Review a transition
- **WHEN** the DJ opens a recording in the Mixed Set Editor and clicks a transition in the list
- **THEN** the timeline scrolls to it, playback starts 4 bars before it, and the panel shows both tracks, the start, length in bars and style found

#### Scenario: Recording without a log
- **WHEN** the DJ opens a recording that has no control log
- **THEN** the editor shows the recording's waveform and the tracklist from its cue sheet

### Requirement: Check my mix
The Mixed Set Editor SHALL list possible problems in a recording (clipping or heavy limiting, silences, big level jumps between tracks and, with a control log, beat drift in transitions, FX left on and jumps mid-phrase), each opening playback just before it.

#### Scenario: Drift found
- **WHEN** two decks drift apart by more than 20 ms for several beats during a transition
- **THEN** Check my mix lists it with its time, and clicking it plays from 2 bars before

### Requirement: Quick fixes
The Mixed Set Editor SHALL offer trim with fades, a level change for one track, cutting out a bar-snapped section and a loudness preset per platform, and SHALL write the result to a new file without changing the recording.

#### Scenario: Loudness for Mixcloud
- **WHEN** the DJ picks the Mixcloud preset and saves
- **THEN** a new file beside the recording measures -14 LUFS integrated with no true peak above -1 dBTP, and the recording is unchanged

#### Scenario: Cut moves the tracklist
- **WHEN** the DJ cuts out 16 bars before the fifth track
- **THEN** the exported tracklist and chapters for the fifth track onwards are earlier by the length cut

### Requirement: Export with chapters
The Mixed Set Editor SHALL export the tracklist for Mixcloud, YouTube chapters and a .cue sheet that follow the fixes. YouTube chapters SHALL start at 0:00, number at least three and last at least 10 s each.
