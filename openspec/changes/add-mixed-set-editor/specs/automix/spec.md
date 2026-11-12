## ADDED Requirements

### Requirement: Pair mixes from a recorded set
The Mixed Set Editor SHALL let the DJ save transitions of a recording as pair mixes for those ordered pairs of tracks: out point, in point, length and style. Saving a pair mix SHALL NOT change either track's own Automix points. Automix SHALL use the pair mix for the two tracks it is mixing first, then the tracks' own points, then phrases.

#### Scenario: Learned pair
- **WHEN** the DJ mixed track A into track B over 16 bars starting 5:12 into A, and saves that transition as a pair mix
- **THEN** the pair A into B is stored with its out point at 5:12, B's in point and 16 bars, and A's own Automix points are unchanged

#### Scenario: Different next track
- **WHEN** Automix mixes A into C and only a pair mix for A into B exists
- **THEN** it uses A's own points, or phrases when A has none
