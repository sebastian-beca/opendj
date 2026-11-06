## ADDED Requirements

### Requirement: Jog latency
A controller's jog, scratched through `engine.scratchEnable` and `engine.scratchTick`, SHALL follow the hand closely without making a steady hand wobble. The mapping's own alpha-beta filter SHALL be paired with a quicker one (3 times alpha, 9 times beta) that takes over while the two disagree by more than the quick filter's own wobble for that wheel's resolution. A latency probe test SHALL measure the result on a DDJ-FLX4-like wheel (720 ticks a turn, a MIDI message every 2 ms).

#### Scenario: Grabbing the record
- **WHEN** the hand starts the record at normal speed from a stop
- **THEN** the deck reaches half that speed within 16 ms and 90 % within 40 ms, before the audio buffer

#### Scenario: Baby scratch
- **WHEN** the hand scratches back and forth 8 times a second at up to 3 times normal speed
- **THEN** the deck lags the hand by at most 14 ms and reaches over 2.5 times normal speed

#### Scenario: Steady hand
- **WHEN** the hand holds normal speed
- **THEN** the speed wobbles by under 1 %, as with the mapping's filter alone

### Requirement: Millisecond timers on Windows
On Windows the app SHALL ask for 1 ms timer resolution at start, so the controller threads' 1 kHz scratch filter and timers run on time.

#### Scenario: Windows jog
- **WHEN** a jog is scratched on Windows
- **THEN** the deck's speed is updated every millisecond, not every 15.6 ms
