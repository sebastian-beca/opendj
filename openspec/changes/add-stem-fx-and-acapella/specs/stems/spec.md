## ADDED Requirements

### Requirement: Stem effects
Each stem SHALL have a quick effect slot (`[QuickEffectRack1_[ChannelN_StemM]]`) defaulting to Echo, so a single stem can be echoed out.

#### Scenario: Echo out vocals
- **WHEN** the vocal stem's echo is triggered and the vocal muted
- **THEN** the vocal tail echoes on while the rest plays dry

### Requirement: Acapella and instrumental
The STEMS page SHALL have Acapella (only vocals) and Instrumental (no vocals) pads that toggle, restoring the previous mix when released.

#### Scenario: Instrumental
- **WHEN** Instrumental is pressed
- **THEN** the vocals are muted and the other stems play

### Requirement: Stems on controllers
In STEMS knob mode, the DDJ-FLX4's and Xone K2's EQ knobs SHALL control stem levels and their LEDs SHALL follow.

#### Scenario: FLX4 stems
- **WHEN** the FLX4's HI knob turns in STEMS mode
- **THEN** the hi-hat stem level changes
