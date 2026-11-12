## ADDED Requirements

### Requirement: Pair mixes from a recorded set
openDJ SHALL find the transitions in a recording made by openDJ and offer to save each as a pair mix for that ordered pair of tracks: out point, in point, length and style. Saving a pair mix SHALL NOT change either track's own Automix points. Automix SHALL use a pair mix for the two tracks it is mixing first, then the tracks' own points, then phrases.

#### Scenario: Learned pair
- **WHEN** the DJ mixed track A into track B over 16 bars starting 5:12 into A, and saves it from that recording
- **THEN** the pair A into B is stored with its out point at 5:12, B's in point and 16 bars, and A's own mix-out point is unchanged

#### Scenario: Different next track
- **WHEN** Automix mixes A into C and only a pair mix for A into B exists
- **THEN** it uses A's own points, or phrases when A has none

#### Scenario: Same point every time
- **WHEN** A was mixed out at the same point into three different tracks across saved sets
- **THEN** the learn screen offers to make that A's own mix-out point, unticked
