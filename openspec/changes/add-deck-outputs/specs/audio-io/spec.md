## ADDED Requirements

### Requirement: Deck outputs
Settings › Audio › Outputs SHALL let Deck 1 to Deck 4, the sampler and the Groove Box each go to a chosen device and channel pair, beside the master, headphone and booth routes. Sources without a route SHALL be heard only through the mixes, as now.

#### Scenario: Two decks into Ableton
- **WHEN** master goes to channels 1-2, deck 1 to 3-4 and deck 2 to 5-6 of BlackHole 16ch, and Ableton Live records inputs 3-4 and 5-6 on two tracks
- **THEN** each Live track holds only its deck, and both stay sample-aligned with each other and with the master

### Requirement: Deck output tap
Settings › Audio › Outputs SHALL offer what deck outputs carry (`[Master],deck_tap`): **Clean** (before EQ, filter, deck FX and fader), **Pre-fader** (after EQ, filter and deck FX, before the channel fader and crossfader; the default) or **Post-fader** (after the fader and crossfader). Post-fader deck outputs SHALL add up to the master mix before master effects and the limiter.

#### Scenario: Effects in the DAW
- **WHEN** the tap is Clean and the DJ kills deck 1's bass
- **THEN** deck 1's output still has its bass, and the master mix does not

#### Scenario: Rebuild the mix
- **WHEN** the tap is Post-fader and the crossfader is fully left
- **THEN** the right-side deck's output is silent

### Requirement: No cost when unused
Deck outputs SHALL add no work to the audio thread while none is routed, and SHALL NOT allocate on the audio thread when routes change. With four deck outputs routed, rendering a buffer SHALL take less than 1% longer than without.

#### Scenario: Benchmark
- **WHEN** the engine benchmark renders four playing decks with and without four deck outputs routed
- **THEN** the difference is under 1%, and with none routed it matches the release before this change

### Requirement: One clock for deck outputs
Deck outputs on the master's device SHALL be sample-aligned with each other and with the master. When deck outputs are spread over more than one device, Settings SHALL warn that they can drift apart and suggest one device or an aggregate device.

#### Scenario: Split devices
- **WHEN** deck 1 goes to the controller's outputs and deck 2 to the laptop's output
- **THEN** the outputs list shows a warning that the two decks may drift apart
