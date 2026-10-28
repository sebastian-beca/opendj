## ADDED Requirements

### Requirement: Per-deck quantize
Each deck's SYNC button SHALL open a quantize menu on right-click whose switch toggles the deck's `quantize`; Settings › Decks Quantize SHALL set every deck's default.

#### Scenario: Quantize off on one deck
- **WHEN** deck 2's quantize is turned off
- **THEN** deck 2's hot cues land where pressed while deck 1 still quantizes

### Requirement: Quantize resolution
The same menu SHALL choose the quantize step: 1/16, 1/8, 1/4, 1/2 or 1 beat, or 4 beats (a bar), 1 by default (`quantize_beats`). Jumps and starts SHALL land in step within that step; cue points and loops SHALL snap to it, at most a beat. A synced deck SHALL keep at least the beat.

#### Scenario: Bar quantize
- **WHEN** the step is 4 and a deck starts while the other plays
- **THEN** it lands on the same beat of the bar as the other deck

#### Scenario: Quarter-beat cue
- **WHEN** the step is 1/4 and a hot cue is set a little after a quarter beat
- **THEN** the cue is saved on that quarter beat

### Requirement: Leader shown and pinned
The deck synced decks follow SHALL show a lit MASTER badge in its header (`sync_leader` 1); clicking a deck's badge SHALL pin it as leader (`sync_leader` 2, badge filled) until it is clicked again.

### Requirement: SYNC on and once
SYNC SHALL sit over the tempo fader and stay on until pressed again; Shift-click SHALL sync tempo and beat once (`beatsync`) without staying on.

#### Scenario: Pin leader
- **WHEN** deck 2's badge is clicked while deck 1 leads
- **THEN** deck 2 becomes leader and stays leader when deck 1 stops
