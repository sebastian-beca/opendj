## ADDED Requirements

### Requirement: Waveform color schemes
Settings › Decks › Waveform color SHALL pick how every deck's waveforms are colored, on every layout: RGB (one shape colored by its loudest band; the default), 3-Band (low, mid and high stacked, low widest), Blue (one blue shape, lighter where the highs are loud) or Stems (each stem stacked in its pad color when the track has stems, RGB otherwise). For 3-Band, Settings › Decks › Palette SHALL pick Blue-Orange-White or Lilac-Cyan-White for low, mid and high. Changing either SHALL redraw the decks at once without loading the tracks again.

#### Scenario: rekordbox colors
- **WHEN** the DJ picks 3-Band with Blue-Orange-White
- **THEN** each deck's waveforms show the bass in blue, the mids in orange and the highs in white, stacked

#### Scenario: Default
- **WHEN** a new user opens the app
- **THEN** the waveforms are RGB

### Requirement: Half overview
Settings › Decks › Overview SHALL draw each deck's overview Full (mirrored about its middle) or Half (growing from its bottom edge up). The big scrolling waveform SHALL stay whole either way.

#### Scenario: Half
- **WHEN** the DJ picks Half
- **THEN** each deck's overview grows from its bottom edge, its cue and hot cue flags at the top

### Requirement: Show vocals
Analysis SHALL give each track a vocal level along the waveform, from a vocal-detection pass or, when the track has stems, its vocal stem. Settings › Decks › Show vocals SHALL tint the parts of the waveform where the level is high green on the RGB, 3-Band and Blue looks.

#### Scenario: Chorus
- **WHEN** Show vocals is on and a deck's track has a sung chorus at 1:00
- **THEN** that stretch of its waveforms is tinted green
