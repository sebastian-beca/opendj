## ADDED Requirements

### Requirement: Start on phrase
The quantize popover SHALL offer a Phrase size. With it, PLAY on a paused synced deck while its leader plays SHALL arm the deck, which then starts, in phase, on the leader's next phrase change (every 8 bars of the leader's grid when the leader has no phrases). PLAY SHALL blink fast while armed; PLAY or CUE again SHALL disarm it.

#### Scenario: Waits for the phrase
- **WHEN** quantize is Phrase, deck 1 leads and is 5 bars from its next phrase, and PLAY is pressed on synced, paused deck 2
- **THEN** deck 2 starts 5 bars later, on the first beat of deck 1's next phrase

#### Scenario: Cancel
- **WHEN** deck 2 is armed and PLAY is pressed again
- **THEN** deck 2 stays paused
