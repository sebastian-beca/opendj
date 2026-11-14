## ADDED Requirements

### Requirement: Waveform zoom
The large scrolling waveforms SHALL share one zoom in every layout, Prep's included, in twenty steps shown as 5% to 100%, each step √1.5 times tighter than the one before, with 40% showing ten seconds across at normal speed. The zoom SHALL glide to a new step over about 150 ms and SHALL be saved. The wheel, trackpad and the edge bar's drag SHALL move the zoom smoothly and settle on the nearest step when they stop. The step readout SHALL show on one waveform only, and the edge bar SHALL run down the left edge of all the stacked waveforms at once. Deck overviews, mini waveforms and live decks' waveforms SHALL NOT change with it.

#### Scenario: Wheel
- **WHEN** the DJ scrolls the mouse wheel up one notch over deck 1's large waveform at 40%
- **THEN** every large waveform glides smoothly to 45%, and "Zoom 45%" shows for a moment on deck 1's waveform only

#### Scenario: Same zoom across layouts
- **WHEN** the DJ zooms to 90% in Prep and switches to 2-Deck
- **THEN** both decks' large waveforms are at 90%

#### Scenario: Controller
- **WHEN** a controller presses `[Channel2],waveform_zoom_down`
- **THEN** every large waveform zooms out one step

#### Scenario: Reset
- **WHEN** the DJ presses ⌘0
- **THEN** every large waveform goes back to 40%
