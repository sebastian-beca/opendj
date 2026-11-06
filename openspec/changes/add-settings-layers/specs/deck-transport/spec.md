## ADDED Requirements

### Requirement: Tempo and key on load
Loading a track SHALL undo any key shift and switch every stem back on. The tempo SHALL go back to 0% unless "Tempo on load" is set to Keep. With "Match tempo on load" set to Close tempos or Always, a track loaded while another deck plays SHALL take that deck's tempo, for Close tempos only when the two are within 10%.

#### Scenario: Key shift reset
- **WHEN** deck 1 is shifted up two semitones and a new track is loaded on it
- **THEN** the new track plays in its own key

#### Scenario: Close tempos
- **WHEN** Match tempo on load is Close tempos, deck 2 plays at 124 BPM and a 120 BPM track is loaded on deck 1
- **THEN** deck 1 is set to 124 BPM; a 90 BPM track would stay at 90

### Requirement: Where a track loads
A loaded track SHALL start at its first hot cue, or at its first sound when it has none, unless "Load at" is set to main cue or start.

#### Scenario: Silence at the start
- **WHEN** a track with no hot cues and two seconds of silence is loaded
- **THEN** the playhead waits at the first sound

### Requirement: Instant doubles
Loading a track that is playing on another deck SHALL start the new deck at the same place, playing and in step, without asking.

#### Scenario: Double a playing track
- **WHEN** deck 1 plays a track and the same track is loaded on paused deck 2
- **THEN** deck 2 plays from deck 1's position at deck 1's tempo
