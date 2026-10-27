## ADDED Requirements

### Requirement: Phrase bands
Each deck's overview SHALL show, from top to bottom and without overlapping, the waveform, a dot at every minute and a phrase bar with one colour per kind (intro and outro #245A78, build #9472D5, drop #CA4B68, chorus #48C5A1, break #E99B68) and no text. The large scrolling waveforms SHALL NOT show phrases. Right-clicking an overview SHALL offer Show phrases / Hide phrases for all decks, remembered between runs. The deck SHALL show the current phrase and the bars left in it.

#### Scenario: Bands
- **WHEN** a track with phrases loads
- **THEN** the overview shows its phrases as bands

#### Scenario: Hidden
- **WHEN** the DJ right-clicks a deck's overview and picks Hide phrases
- **THEN** no overview shows phrase bars, also after a restart, and the waveform takes their room

#### Scenario: Countdown
- **WHEN** a deck plays 8 bars before the end of a Drop
- **THEN** the deck shows "Drop · 8 bars left"
