## MODIFIED Requirements

### Requirement: Slip mode
With `slip_enabled` on, a playing deck SHALL keep a silent playhead moving while a trick takes it elsewhere (scratching, a loop, a hot cue held, reverse, or a pause), and SHALL carry on from it when that trick ends, with slip still on. In slip mode a set hot cue pressed on a playing deck SHALL play only while held. Switching slip off during a trick SHALL also return to the silent playhead and end a loop it leaves. Beat jumps, CUE and a paused deck SHALL NOT slip. While slipping, `slip_position` SHALL give the silent playhead in seconds (-1 otherwise).

#### Scenario: Slip after a loop
- **WHEN** slip is on and a loop is exited after 8 beats
- **THEN** playback continues 8 beats past where the loop started, and slip stays on

#### Scenario: Slip after a scratch
- **WHEN** slip is on and the DJ holds the jog of a playing deck still for half a second
- **THEN** on letting go the track carries on half a second past where it was held

#### Scenario: Slip pause
- **WHEN** slip is on and a playing deck is paused for a second
- **THEN** pressing play carries on a second past where it paused

#### Scenario: Paused deck
- **WHEN** slip is on and the DJ scratches a paused deck to a new spot
- **THEN** it stays at that spot

## ADDED Requirements

### Requirement: Ghost playhead
While slipping, both waveforms SHALL draw the silent playhead as a dashed lilac line, and the deck's SLIP toggle SHALL blink.

#### Scenario: Ghost
- **WHEN** a loop runs with slip on
- **THEN** a dashed line moves on beyond the loop

### Requirement: Bend and censor on screen
Holding « or » in the jog's lower corners SHALL bend the deck by 2% slower or faster (4% with Shift) until released. Holding ← or → SHALL bend the deck started last the same way, unless the DJ gave those keys shortcuts. Shift-click and hold PLAY on a playing deck SHALL be censor (`reverseroll`).

#### Scenario: Arrow bend
- **WHEN** the DJ holds → while deck 2 was started last
- **THEN** deck 2 plays 2% faster until the key is released
