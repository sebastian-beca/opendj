## ADDED Requirements

### Requirement: Stem effects
Each stem SHALL have a quick effect switch (`[QuickEffectRack1_[ChannelN_StemM]],enabled`) that sends it, at its level, into the deck's stem effect (`[ChannelN],stem_fx`: 1 Echo, the default, or 2 Reverb), whose output adds to the deck while the dry stem plays on. Muting a sent stem SHALL leave the effect's tail ringing. A stem muted in the same moment its effect goes on SHALL play on for one echo (half a beat; a quarter second for Reverb) before the mute is heard. The effect SHALL stop running once it has rung out, and a track load SHALL switch every stem's effect off and silence its tail. Switching the effect on SHALL NOT change the dry sound.

#### Scenario: Echo out vocals
- **WHEN** the vocal stem's effect is switched on and the vocal muted
- **THEN** the vocal plays on for half a beat, then its echo rings on and fades while the rest plays dry

#### Scenario: Key lock
- **WHEN** the same happens on a key-locked deck at another tempo
- **THEN** the echo plays at the deck's key and tempo

### Requirement: Acapella and instrumental
The STEMS page SHALL have Acapella (only vocals) and Instrumental (no vocals) pads that toggle, bringing every stem back on the second press. A Stems FX pad SHALL switch the pads to echo out (`[ChannelN],stem_fx_mode`): a stem's pad then switches its effect on and mutes it, and a second press brings it back dry with its effect off; Acapella and Instrumental echo out what they remove. Right-clicking Stems FX SHALL pick Echo or Reverb.

#### Scenario: Instrumental
- **WHEN** Instrumental is pressed
- **THEN** the vocals are muted and the other stems play

#### Scenario: Instrumental with echo
- **WHEN** Stems FX is on and Instrumental is pressed
- **THEN** the vocals echo out

### Requirement: Stems on controllers
In STEMS knob mode, the DDJ-FLX4's and Xone K2's EQ knobs SHALL control stem levels. The DDJ-FLX4's stems pads SHALL follow the screen's layout, Stems FX included.

#### Scenario: FLX4 stems
- **WHEN** the FLX4's HI knob turns in STEMS mode
- **THEN** the hi-hat stem level changes

#### Scenario: FLX4 echo out
- **WHEN** the FLX4's Stems FX pad and then its Vocal pad are pressed
- **THEN** the vocals echo out
