## ADDED Requirements

### Requirement: Phrase bands
The overview waveform SHALL show phrases as coloured bands, one colour per kind, and the scrolling waveform SHALL show a line at each phrase start. The deck SHALL show the current phrase and the bars left in it.

#### Scenario: Bands
- **WHEN** a track with phrases loads
- **THEN** the overview shows its phrases as bands

#### Scenario: Countdown
- **WHEN** a deck plays 8 bars before the end of a Drop
- **THEN** the deck shows "Drop · 8 bars left"
