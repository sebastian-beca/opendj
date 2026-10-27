## ADDED Requirements

### Requirement: Phrase bands
The overview and scrolling waveforms SHALL show phrases as a thin bar along their bottom edge, one muted colour per kind and no text, and the scrolling waveform SHALL show a faint line at each phrase start. Right-clicking a scrolling waveform SHALL offer Show phrases / Hide phrases, remembered between runs. The deck SHALL show the current phrase and the bars left in it.

#### Scenario: Bands
- **WHEN** a track with phrases loads
- **THEN** the overview shows its phrases as bands

#### Scenario: Hidden
- **WHEN** the DJ right-clicks a waveform and picks Hide phrases
- **THEN** no waveform shows phrase bars, also after a restart

#### Scenario: Countdown
- **WHEN** a deck plays 8 bars before the end of a Drop
- **THEN** the deck shows "Drop · 8 bars left"
