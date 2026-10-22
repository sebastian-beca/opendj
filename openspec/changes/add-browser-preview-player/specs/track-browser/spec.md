## ADDED Requirements

### Requirement: Preview from the list
Each row SHALL have a preview button that plays the track from about a third in, through `[PreviewDeck1]`, into the headphone mix only; pressing again or another row stops it.

#### Scenario: Audience doesn't hear
- **WHEN** a track is previewed while a deck plays
- **THEN** the preview is in the headphones and not in the master

### Requirement: Scrub the preview
While previewing, a waveform in the side panel SHALL show the preview and seek where clicked.

#### Scenario: Seek
- **WHEN** the preview waveform is clicked near the end
- **THEN** the preview plays from there
