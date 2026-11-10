## ADDED Requirements

### Requirement: Live deck waveform
A deck playing a live source (an input, an instrument plugin or the Groove Box) SHALL show its own audio on its scrolling waveform as it plays: drawn in band colours up to the play line and scrolling away from it, with nothing drawn ahead of the play line. The lane SHALL show the beat grid and bar numbers of the leading deck (the sync leader, else the first playing deck with a tempo) at that deck's playing tempo; with no leading deck it SHALL show no grid.

#### Scenario: Aux against a track
- **WHEN** deck 1 plays a track at 124 BPM and deck 2 plays the aux input
- **THEN** deck 2's scrolling waveform draws the aux audio up to its play line, with deck 1's beat lines and bar numbers scrolling with deck 1's

#### Scenario: Silence draws flat
- **WHEN** a deck plays an input that receives no sound
- **THEN** its scrolling waveform shows a flat line behind the play line, and its grid still scrolls

### Requirement: Instrument and Groove Box views
An instrument deck's scrolling waveform SHALL show the notes played as a note roll fitted to the range of notes in view, with the current scale's rows tinted and held notes lit. A Groove Box deck's scrolling waveform SHALL show its eight tracks as lanes with the hits played and the hits coming, its fills marked and the bar where the next groove starts. In the 2-Deck layout, an instrument deck SHALL offer a second view showing its knobs as automation lines across the full height, a colour per knob, drawing only the knobs that moved in view.

#### Scenario: Coming hits
- **WHEN** the Groove Box plays and a fill is due in the next bar
- **THEN** the fill's hits show ahead of the play line, marked as a fill
