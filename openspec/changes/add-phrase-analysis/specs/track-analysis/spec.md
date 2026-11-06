## ADDED Requirements

### Requirement: Phrases
Analysis SHALL split a track with a beat grid into phrases of whole bars, each labelled Intro, Build, Drop, Chorus, Verse, Breakdown or Outro with an energy level from 1 to 10 (relative to the rest of the track), and store them with the track. Cuts SHALL follow changes of harmony (the chroma kept with each analysis) as well as changes of loudness per band. Phrases from an import SHALL be kept and not replaced by detected ones. A grid edit SHALL re-derive detected phrases so they stay on bar lines.

#### Scenario: Intro found
- **WHEN** a house track with a 32-bar intro is analysed
- **THEN** an intro of 32 bars is stored, starting on the first downbeat

#### Scenario: Breakdown and drop
- **WHEN** a track drops its bass for 16 bars and brings it back
- **THEN** those 16 bars are a Breakdown and the phrase after them is a Drop

#### Scenario: Kick enters early
- **WHEN** the kick comes in alone 4 bars before the full groove
- **THEN** the intro ends where the kick comes in, not on the neater 16-bar line

#### Scenario: Groove after the last drop
- **WHEN** a strong groove carries on after the last drop and the track then fades to a quiet tail
- **THEN** only the tail is the Outro

#### Scenario: Chords change at the same loudness
- **WHEN** a track plays 16 bars of one chord loop and then 16 bars of different chords, equally loud
- **THEN** a phrase starts where the chords change

#### Scenario: Verses in a song
- **WHEN** a song plays verse, chorus, verse, chorus, with the bass in the verses
- **THEN** the verses are labelled Verse and the choruses as its strongest parts

#### Scenario: Energy
- **WHEN** a track has a quiet breakdown and a loud drop
- **THEN** the drop's energy is 8 or more and the breakdown's the lowest of the track

#### Scenario: Imported phrases win
- **WHEN** a track with phrases from an import is analysed again
- **THEN** its imported phrases stay

#### Scenario: Grid moved
- **WHEN** the DJ moves the downbeat of an analysed track
- **THEN** its detected phrases move to the new bar lines
