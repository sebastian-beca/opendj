## ADDED Requirements

### Requirement: Automix points from a recorded set
openDJ SHALL find the transitions in a recording made by openDJ and offer to save each as Automix points: a mix-out point on the outgoing track, a mix-in point on the incoming track and a pair mix for that ordered pair of tracks. Points already on a track SHALL be kept unless the DJ chooses to replace them.

#### Scenario: Learned points
- **WHEN** the DJ mixed track A into track B over 16 bars starting 5:12 into A, and saves the points from that recording
- **THEN** A has a mix-out point at 5:12, B has a mix-in point where it was when it became audible, and the pair A into B is stored with 16 bars

#### Scenario: Pair mix first
- **WHEN** Automix mixes A into B and a pair mix for A into B exists
- **THEN** it uses the pair mix's points and length, ahead of the tracks' own points and phrases
