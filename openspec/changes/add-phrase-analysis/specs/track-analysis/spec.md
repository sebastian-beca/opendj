## ADDED Requirements

### Requirement: Phrases
Analysis SHALL split a track with a beat grid into phrases of whole bars, each labelled Intro, Build, Drop, Breakdown or Outro with an energy level from 1 to 10, and store them with the track. Phrases from an import SHALL be kept and not replaced by detected ones. A grid edit SHALL re-derive detected phrases so they stay on bar lines.

#### Scenario: Intro found
- **WHEN** a house track with a 32-bar intro is analysed
- **THEN** an intro of 32 bars is stored, starting on the first downbeat

#### Scenario: Breakdown and drop
- **WHEN** a track drops its bass for 16 bars and brings it back
- **THEN** those 16 bars are a Breakdown and the phrase after them is a Drop

#### Scenario: Imported phrases win
- **WHEN** a track with phrases from an import is analysed again
- **THEN** its imported phrases stay

#### Scenario: Grid moved
- **WHEN** the DJ moves the downbeat of an analysed track
- **THEN** its detected phrases move to the new bar lines
