## ADDED Requirements

### Requirement: Per-deck quantize
Each deck SHALL have a Q button toggling its `quantize`; Settings › Decks Quantize SHALL set every deck's default.

#### Scenario: Q off on one deck
- **WHEN** deck 2's Q is turned off
- **THEN** deck 2's hot cues land where pressed while deck 1 still quantizes

### Requirement: Quantize resolution
Quantize SHALL snap to the beat, half beat or bar, chosen in Settings › Decks.

#### Scenario: Bar quantize
- **WHEN** resolution is bar and a hot cue is pressed mid-bar
- **THEN** the jump lands in step with the leader's bar

### Requirement: Leader shown and pinned
The deck synced decks follow SHALL carry a leader badge; clicking a synced deck's badge SHALL pin it as leader until unpinned.

#### Scenario: Pin leader
- **WHEN** deck 2's badge is clicked while deck 1 leads
- **THEN** deck 2 becomes leader and stays leader when deck 1 stops
