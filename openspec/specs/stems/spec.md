# stems Specification

## Purpose
Playing a track as separate stems (drums, bass, other, vocals and hi-hats) with their own volume and mute, from NI stem files or by separating any track with Demucs.

## Requirements

### Requirement: NI stem files
A Native Instruments stem file (`.stem.mp4`, `.stem.m4a`) SHALL load with its stems, names and colours from its manifest, while the full mix stays the source of the waveform and analysis.

#### Scenario: Stem file loads
- **WHEN** a `.stem.mp4` is loaded
- **THEN** `stem_count` is 4 and the STEMS pad page shows the four named, coloured stems

### Requirement: Separate with Demucs
For any other track, the STEMS pad page SHALL offer SEPARATE STEMS when Demucs is installed. Separation SHALL run in the background, show its progress, cache the result per file, and offer TRY AGAIN when it fails. Without Demucs the page SHALL say "No stems".

#### Scenario: Separate a track
- **WHEN** SEPARATE STEMS is pressed on a loaded MP3 with Demucs installed
- **THEN** the page shows progress, and when done the deck plays the stems

#### Scenario: Cached stems
- **WHEN** a separated track is loaded again
- **THEN** its stems are available at once

### Requirement: Stem controls
Each stem SHALL have `volume`, `mute` and `color` (`[ChannelN_StemM]`, M = 1 to 5). Muting or changing volume SHALL ramp without clicks. A deck with stems plays their sum.

#### Scenario: Mute vocals
- **WHEN** the vocal stem's pad is pressed
- **THEN** the vocals are muted and the pad dims

### Requirement: Stem pads
The STEMS pad page SHALL show one pad per stem (mute toggles) above a level for each.

#### Scenario: Stem level
- **WHEN** the drums stem level is dragged down to half
- **THEN** the drums play at half volume
