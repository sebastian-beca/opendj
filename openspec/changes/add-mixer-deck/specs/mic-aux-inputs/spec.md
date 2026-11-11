## MODIFIED Requirements

### Requirement: Aux input
Up to eight auxiliary inputs, `[Auxiliary1]` to `[Auxiliary8]`, SHALL each take one channel (mono) or a pair (stereo) of an interface, picked by channel. Each interface SHALL be opened once for all the inputs that use it. The aux inputs SHALL be played as strips of a Mixer deck.

#### Scenario: Phone in
- **WHEN** a phone is on an aux input played by the Mixer deck and its strip is on
- **THEN** it is heard in the master

#### Scenario: Mono by picking one channel
- **WHEN** a strip's input is set to channel 3
- **THEN** the strip is mono, heard in both sides, and shows one meter bar

#### Scenario: Eight inputs on one interface
- **WHEN** eight strips take channels of the same interface
- **THEN** the interface is opened once and all eight are heard

## ADDED Requirements

### Requirement: Mixer deck
The Jam source SHALL offer a Mixer that loads onto one deck at a time. A Mixer deck SHALL sum its strips into the deck's channel, so the deck's EQ, filter, effects and fader act on the sum. Each strip SHALL have TRIM, PAN, a fader, an on/off number button, solo, cue, an FX switch, and a pre-fader meter with peak hold and a clip readout.

#### Scenario: Deck fader is the master
- **WHEN** two strips play and the Mixer deck's fader is pulled down
- **THEN** both strips go quiet in the master

### Requirement: Strip solo, mute and FX
Solo SHALL silence the other strips of the same Mixer and no other deck; several strips MAY be soloed, and solo SHALL win over off. Strips with FX on SHALL go through the deck's effects and the others SHALL stay dry. A new strip SHALL start off.

#### Scenario: Echo on the mic only
- **WHEN** the Mic strip has FX on, the Turntable strip has FX off, and the deck's echo is on
- **THEN** the voice has the echo and the turntable is heard dry

#### Scenario: Solo stays in the Mixer
- **WHEN** a strip is soloed while deck 1 plays a track
- **THEN** the other strips go silent and deck 1 is still heard

### Requirement: Mic strip
The microphone MAY be a strip of the Mixer, marked Mic with a DUCK switch. The MIC button in the top bar SHALL keep controlling the same microphone.

#### Scenario: Talkover from either place
- **WHEN** the mic is a Mixer strip and the DJ presses the MIC button
- **THEN** the mic is heard and the music ducks as set
