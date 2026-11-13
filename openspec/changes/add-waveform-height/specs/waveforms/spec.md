## ADDED Requirements

### Requirement: Waveform height
The large scrolling waveforms SHALL offer ten heights, 1 to 10, with 3 the height they had before, kept and saved separately for each layout. When the chosen height would leave the library less than about five track rows, the window SHALL show the next smaller height that fits, never below 3 unless the DJ chose a smaller one, and SHALL show the chosen height again once there is room. The deck rows SHALL NOT change height.

#### Scenario: Drag
- **WHEN** the DJ drags the grip under the waveforms in 2-Deck Pro down and lets go nearest height 6
- **THEN** the waveforms stretch with the pointer, show "6" in the middle, settle on height 6, and 2-Deck Pro opens at 6 next time

#### Scenario: Per layout
- **WHEN** the DJ picks 10 in 2-Deck and switches to 4-Deck
- **THEN** 4-Deck's waveforms keep the height chosen for 4-Deck

#### Scenario: Small window
- **WHEN** 10 is chosen and the window is too short for it with five tracks in the library
- **THEN** the waveforms show the largest height that fits, and 10 again when the window grows

#### Scenario: Controller
- **WHEN** a controller presses `[Skin],waveform_height_up`
- **THEN** the waveforms in the layout showing grow one height
