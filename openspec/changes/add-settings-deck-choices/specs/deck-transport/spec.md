## ADDED Requirements

### Requirement: Hot cue on a playing deck
Pressing a hot cue on a playing deck SHALL jump and keep playing, or, with "Hot cue on a playing deck" set to Stutter,
restart from the cue each time it is pressed.

#### Scenario: Stutter
- **WHEN** Stutter is chosen and hot cue 1 is pressed three times in a row on a playing deck
- **THEN** the deck restarts from hot cue 1 on each press
