# controllers Specification

## Purpose
MIDI controller support using Mixxx's own mappings unchanged, how controllers are found and shown, and how two-deck controllers reach decks 3 and 4.

## Requirements

### Requirement: Mixxx mappings run unchanged
openDJ SHALL read Mixxx `.midi.xml` mappings and run their JavaScript in QuickJS against Mixxx's `engine` and `midi` API (soft takeover, timers, scratch and brake filters, MIDI out), using the same control names.

#### Scenario: Supported controller
- **WHEN** a controller Mixxx supports is plugged in
- **THEN** its mapping loads and it works with nothing to set up

### Requirement: Hot plugging
Plugging a controller in or out SHALL be picked up within a couple of seconds, each controller on its own thread. LEDs SHALL be sent again 1.5 s after start.

#### Scenario: Plug in while running
- **WHEN** a DDJ-FLX4 is plugged in while openDJ runs
- **THEN** it works within about two seconds

### Requirement: Controllers settings
Settings › Controllers SHALL list connected controllers with the mapping in use, let one be set to Not used, and say when mappings are missing or a mapping failed.

#### Scenario: Turn off a controller
- **WHEN** a controller is set to Not used
- **THEN** openDJ ignores it

### Requirement: Decks 3 and 4 on two-deck controllers
A mapping without decks 3 and 4 SHALL get `[Controller],deck3`/`deck4` switches. In 4-Deck, clicking deck 3's (or 4's) number SHALL move the left (or right) side there, LEDs included; leaving 4-Deck SHALL move both back.

#### Scenario: FLX4 on deck 3
- **WHEN** deck 3's number is clicked in 4-Deck
- **THEN** the FLX4's left side plays deck 3 and the number is lit

### Requirement: Controls list and docs
`opendj --list-controls` SHALL print every control; the website SHALL have a page per mapped controller. `OPENDJ_MIDI_DEBUG` SHALL log MIDI both ways.

#### Scenario: CI catches new controls
- **WHEN** a control is added without regenerating supported-controls.txt
- **THEN** CI fails

### Requirement: HID not supported
HID controllers SHALL NOT be supported yet, and the docs SHALL say so.

#### Scenario: HID controller
- **WHEN** a HID-only controller is plugged in
- **THEN** openDJ does not use it
