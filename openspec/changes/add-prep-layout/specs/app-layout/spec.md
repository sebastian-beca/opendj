## MODIFIED Requirements

### Requirement: Layouts
The layout menu at the top left SHALL offer Simple (decks 1 and 2 as one row each, library gets most of the screen), 2-Deck (full decks with pads and FX, mixer between), 4-Deck (djay style, strips in the order 3, 1, 2, 4, each with EQ, filter, gain, fader, transport, loop, tempo, pads and FX) and Prep (Deck 1 alone for preparing tracks, without mixer, crossfader or FX bar). The choice SHALL be saved (`layout`).

#### Scenario: 4-Deck order
- **WHEN** 4-Deck is chosen
- **THEN** four strips show left to right as decks 3, 1, 2, 4

#### Scenario: Prep shows one deck
- **WHEN** Prep is chosen
- **THEN** only Deck 1 shows, with a large waveform, the grid panel, transport and eight hot cue pads, and the browser opens on In Prep

## ADDED Requirements

### Requirement: Quick review keys
In Prep, keys SHALL step through a track without the mouse: Tab and Shift+Tab to the next or previous phrase, Left and Right by four bars, 1–8 to set or jump to a hot cue, K to keep suggested cues, B for Bar 1 here, [ and ] to move the grid, M for the click, Option+1–9 to tick checklist lines and Enter for Play Ready, next track. A strip under the waveform SHALL list them with the time spent on this track and the average per track.

#### Scenario: One pass by keyboard
- **WHEN** a track is loaded in Prep and Tab is pressed
- **THEN** the playhead jumps to the start of the next phrase and plays from there
