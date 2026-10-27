## ADDED Requirements

### Requirement: Mapping list
Settings › Controllers SHALL show a controller's mapping as a list of control, MIDI message and options, searchable.

#### Scenario: Find a binding
- **WHEN** the DJ searches "play" in the FLX4's mapping
- **THEN** the play bindings for each deck are listed

### Requirement: MIDI learn
Learn SHALL bind the next control clicked on screen to the next MIDI message received, with soft takeover for knobs and faders.

#### Scenario: Learn a knob
- **WHEN** Learn is on, a filter knob is clicked and a controller knob turned
- **THEN** that knob now drives the filter

### Requirement: Custom mappings
Edited mappings SHALL be saved as Mixxx-format files in the config folder, override the built-in one, survive updates, and be exportable and importable.

#### Scenario: Share
- **WHEN** a custom mapping is exported and imported on another Mac
- **THEN** the controller behaves the same there
