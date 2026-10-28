## ADDED Requirements

### Requirement: Controller Editor
Settings › Controllers SHALL open a Controller Editor that fills the window and shows any mapping, the one a connected controller runs first. It SHALL say it is the original mapping and that it is view only.

#### Scenario: Open from Settings
- **WHEN** the DJ presses Open beside "Controller Editor" with a DDJ-FLX4 connected
- **THEN** the editor shows the Pioneer DDJ-FLX4 mapping

### Requirement: Controls by function
The editor SHALL list every mapped control grouped by function category, each row naming the function, its target, the hardware control, its layer (SHIFT, pad mode or bank), an icon for its kind, and its MIDI message, and SHALL be searchable.

#### Scenario: Find hot cues
- **WHEN** the DJ searches "hot cue 2" in the FLX4 mapping
- **THEN** rows for Hot cue 2 on Deck 1 and Deck 2 list Pad 2 in Hot cue mode, and Pad 2 with SHIFT clearing it

### Requirement: Script rows explained
For script-bound rows the editor SHALL show which controls the script changes, found by pressing each control once in a sandbox copy of the engine controls, also with SHIFT held.

#### Scenario: Twister EQ
- **WHEN** the Midi Fighter Twister mapping is opened
- **THEN** encoder 2 is listed under EQ & filter as High EQ on Deck 1, though its row is a script binding

### Requirement: Lights and options
The editor SHALL list what each LED shows in a Lights tab and the mapping's own settings with their defaults in an Options tab.

#### Scenario: Twister options
- **WHEN** the Twister's Options tab is opened
- **THEN** its settings are listed, such as "Use gain knob as vu meter"

### Requirement: Controller pictures and press to find
The DDJ-FLX4, Xone K2 and Midi Fighter Twister SHALL have a drawn picture: clicking a control lists its rows, picking a row lights its control. Touching a control on connected hardware SHALL select its rows and light it.

#### Scenario: Press play
- **WHEN** the FLX4's left PLAY/PAUSE is pressed while the editor is open
- **THEN** the Play row for Deck 1 is selected and PLAY/PAUSE lights on the picture
