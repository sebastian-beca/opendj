# sync Specification

## Purpose
Keeping decks in time with each other and with other apps: SYNC with a leader, phase lock, half and double tempo, quantize, and Ableton Link.

## Requirements

### Requirement: Sync leader
With one deck synced, it SHALL follow a playing deck among the others. With several synced, the leader SHALL be the synced deck playing longest, handing over when it stops. Without a playing deck, a synced deck SHALL follow any deck with a track.

#### Scenario: Leader hands over
- **WHEN** decks 1 and 2 are synced, deck 1 started first and then stops
- **THEN** deck 2 becomes the leader and keeps its tempo

### Requirement: Tempo follow
A synced deck SHALL play at its leader's tempo, at half or double when that is closer to its own (153 BPM follows 80 at 160), keeping that ratio while the same two tracks play. Sync SHALL reach tempos beyond the tempo fader's range.

#### Scenario: Double tempo
- **WHEN** a 140 BPM track is synced to a 70 BPM leader
- **THEN** it plays at 140 BPM, not slowed to 70

#### Scenario: Beyond fader range
- **WHEN** the range is ±8 % and a 120 BPM track syncs to 132 BPM
- **THEN** it plays at 132 BPM

### Requirement: Phase lock
A synced deck SHALL stay on its leader's beat, catching up drift by bending speed at most 3 % and never by jumping. Turning sync on while playing, or `beatsync`, SHALL line the beats up at once. A nudge by hand SHALL set an offset that is kept.

#### Scenario: Drift corrected silently
- **WHEN** a synced deck drifts a quarter beat behind its leader
- **THEN** it speeds up slightly until it is back on the beat, without a jump

#### Scenario: Nudge kept
- **WHEN** a synced deck is nudged ahead with the jog
- **THEN** it stays that far ahead of the beat afterwards

### Requirement: Quantize
With quantize on (Settings › Decks, on by default, `quantize`), loops SHALL snap to the beat, and play, CUE, hot cues and beat jumps on a deck SHALL land in step with the other playing deck, at most half a beat from where they were aimed. A synced deck SHALL always land in phase.

#### Scenario: Hot cue lands in phase
- **WHEN** quantize is on and a hot cue is pressed on a deck while another plays
- **THEN** the deck lands on the same part of the beat as the playing deck

### Requirement: Sync on controllers
`sync_enabled` SHALL toggle sync from controllers and the deck's SYNC button alike.

#### Scenario: FLX4 BEAT SYNC
- **WHEN** BEAT SYNC is pressed on a DDJ-FLX4
- **THEN** the deck's SYNC turns on and lights

### Requirement: Ableton Link
LINK in the top bar SHALL join an Ableton Link session on the network (`[Link],enabled`). Link SHALL follow the master deck, the one playing (or started first while several play), so other apps play in time with the mix; there is no per-deck LINK button. The peer count SHALL be shown.

#### Scenario: Link follows the playing deck
- **WHEN** LINK is on and deck 2 is the only deck playing at 124 BPM
- **THEN** the Link session tempo is 124 BPM and its beat matches deck 2's
