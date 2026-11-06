## ADDED Requirements

### Requirement: Timeline in the waveform strip
A Groove Box deck SHALL use the strip a track deck uses for its waveform to show a bar timeline: grooves as blocks along the bars, the hits of each row as ticks, and a playhead at the same position and zoom as the other decks' waveforms, with the groove's bars on the leading deck's bars. The leading deck's phrase markers SHALL be drawn behind the blocks. With no chain, the timeline SHALL show the selected groove repeating.

#### Scenario: Line up with a drop
- **WHEN** deck 1 plays a track with a drop in 6 bars and the Groove Box on deck 2 plays groove A
- **THEN** deck 2's timeline shows the drop's phrase marker 6 bars ahead, under the same point as deck 1's waveform

### Requirement: Chains
A Groove Box SHALL hold eight grooves (A to H) and a chain of up to 64 bars built by dragging grooves onto the timeline. A chain SHALL play in order and loop; clicking a block SHALL move playback there on the next bar.

#### Scenario: Four bars of A, four of B
- **WHEN** A is dragged onto bars 1-4 and B onto bars 5-8 and PLAY is pressed
- **THEN** A plays for four bars, B for four, then A again

### Requirement: Step detail
Each step SHALL have an accent (soft, medium, hard), a chance (100, 75, 50, 25 %), a nudge (−50 % to +50 % of a step) and a roll (1 to 4 hits within the step), set from the step's right-click menu or by holding the step and turning a knob. The face SHALL show accent as brightness and rolls as a split cell, and nothing else.

#### Scenario: Roll at the end of the bar
- **WHEN** step 16 of the Snare row is set to a roll of 4
- **THEN** four evenly spaced snares play within that 16th, each on its exact sample

### Requirement: Rows and lengths
A kit SHALL hold up to 16 sounds and show up to 8 rows. Each row SHALL have its own length (1 to 64 steps) and step size (1/16, 1/8, 1/16 triplet). Rows of different lengths SHALL run independently and restart together when the groove restarts or changes.

#### Scenario: Three-step hats
- **WHEN** the Hat row's length is set to 3 while the Kick row is 16 steps
- **THEN** the hat pattern repeats every 3 steps against the 16-step kick

### Requirement: Sound knobs
Every row SHALL have LEVEL, PITCH, DECAY and a fourth knob named by the kit. For built-in synthesised sounds the fourth knob SHALL change the synth voice; for sample sounds it SHALL be a low-pass filter.

#### Scenario: Snappier snare
- **WHEN** the Snare row of a built-in kit is selected and SNAPPY is turned right
- **THEN** the snare's noise part is louder on the next hit

### Requirement: Fills and variation
A groove MAY have a fill and a variation. FILL SHALL play the fill once on the last bar before the leading deck's next phrase, or on the next bar when there are no phrases; holding FILL SHALL play it until released. The Variation switch SHALL alternate the groove and its variation every bar or every two bars. Every built-in groove SHALL have a fill.

#### Scenario: Fill into the drop
- **WHEN** FILL is pressed 3 bars before the leading deck's next phrase
- **THEN** the groove plays normally for 2 bars, the fill on the third, and the groove again from the phrase

### Requirement: Melodic row
A kit MAY have one melodic row whose steps hold a scale degree, octave, length and slide. Notes SHALL be played in the key of the leading deck, following key changes on the bar, unless a fixed key is set. The row SHALL play through a built-in mono synth or a hosted instrument plugin.

#### Scenario: Follow the key
- **WHEN** the melodic row plays degrees 1, 1, 5, 4 and the leading deck is in A minor
- **THEN** the row plays A, A, E, D

### Requirement: Record from pads
With REC armed, hits on the sampler pads or the Steps page SHALL be written into the selected groove at the nearest step, with accent from velocity. When the groove is stopped, recording SHALL start after a one-bar count-in. Undo SHALL remove the last pass.

#### Scenario: Record a clap
- **WHEN** REC is on and the Clap pad is hit slightly late on beats 2 and 4
- **THEN** the Clap row's steps 5 and 13 turn on

### Requirement: Sounds from the mix
A file, a sampler pad or a region selected on a track deck's waveform dropped onto a row SHALL become that row's sound, trimmed to its first transient and kept as an override of the kit. With stems loaded on a track deck, a row's menu SHALL offer the kick, snare or hat cut from that deck's drum stem.

#### Scenario: Kick from the track
- **WHEN** deck 1 has stems and "Kick from deck 1" is chosen on the Kick row
- **THEN** the Kick row plays a kick cut from deck 1's drum stem

### Requirement: Replace drums
A track deck with stems SHALL offer Replace drums: a Groove Box over the deck that mutes the track's drum stem and plays its groove in the track's grid on the deck's channel, with roll and stutter. Turning it off SHALL bring the track's drums back on the next beat.

#### Scenario: Swap the drums
- **WHEN** Replace drums is turned on during a track's chorus
- **THEN** the track's drum stem fades out and the groove's drums play on the track's beats until it is turned off
