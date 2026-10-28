## ADDED Requirements

### Requirement: Analyze on request
The DJ SHALL be able to analyse chosen tracks again from the browser. Each SHALL be decoded and analysed afresh in the background, one track at a time, ignoring any cached result, and its tempo, grid, key, loudness, waveform and phrases replaced. A grid the DJ edited or an import brought, and a key from the tags, an import or the DJ, SHALL be kept.

#### Scenario: Older analysis redone
- **WHEN** the DJ analyses a track analysed by an earlier version
- **THEN** its tempo, grid, key, waveform and phrases are found again and saved

#### Scenario: Hand-edited grid kept
- **WHEN** the DJ analyses a track whose downbeat they moved
- **THEN** its grid stays where they put it, and its key and waveform are found again

#### Scenario: Stopped
- **WHEN** the DJ presses ✕ beside "Analyzing 3 of 12"
- **THEN** the tracks still waiting are not analysed

### Requirement: Stored phrases
Phrases found by analysis SHALL be stored with the track (markers of kind phrase, source analysis) each time it is analysed, and found again and stored when its grid changes. Phrases from an import SHALL be kept and no detected ones stored beside them. Until a loaded track's waveform is ready, its deck SHALL show the stored phrases.

#### Scenario: Phrases kept
- **WHEN** a track is analysed in the background
- **THEN** its phrases are in the library without loading it

#### Scenario: Grid moved
- **WHEN** the DJ moves the downbeat of an analysed track on a deck
- **THEN** its stored phrases move to the new bar lines
