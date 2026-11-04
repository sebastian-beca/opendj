## MODIFIED Requirements

### Requirement: Layouts
The layout menu at the top left SHALL offer Player (one bar that plays music like a music player, with Automix mixing between decks 1 and 2 underneath; the library gets most of the screen), 2-Deck (full decks with pads and FX, mixer between) and 4-Deck (djay style, strips in the order 3, 1, 2, 4, each with EQ, filter, gain, fader, transport, loop, tempo, pads and FX). The choice SHALL be saved (`layout`); a saved Simple layout SHALL open as Player.

#### Scenario: 4-Deck order
- **WHEN** 4-Deck is chosen
- **THEN** four strips show left to right as decks 3, 1, 2, 4

#### Scenario: Play starts Automix
- **WHEN** Player is chosen, tracks are on the Automix list and the DJ presses Play
- **THEN** Automix switches on and plays the list, and the bar shows the track that is playing

#### Scenario: Next mixes now
- **WHEN** Automix is playing in Player and the DJ presses Next
- **THEN** Automix mixes into the next track straight away with the chosen mix length

#### Scenario: Queue button
- **WHEN** the DJ clicks the queue button in the Player bar
- **THEN** Up Next (the Automix list) shows on the right, or hides if it was showing

#### Scenario: Nothing extra
- **WHEN** Player is chosen
- **THEN** the large waveforms, the FX bar and the WAVEFORMS and FX buttons are hidden

#### Scenario: Search and play next
- **WHEN** Player is chosen and the DJ types in the search box in the top bar
- **THEN** the best matching tracks show in a list under it, titles that match first, each with Play next, Add to queue and Play now, and Play next puts the track on the Automix list after the tracks Automix is playing or has waiting

#### Scenario: One search box
- **WHEN** Player is chosen and the DJ types in the search box in the top bar
- **THEN** the track list is filtered by the same text, and the list's own search box is hidden

#### Scenario: Repeat
- **WHEN** Repeat is on in the Player bar and Automix has played a track for long enough to count as played
- **THEN** the track moves to the end of the Automix list instead of leaving it

#### Scenario: Key colour
- **WHEN** the track playing in Player has a key
- **THEN** the bar shows the key in the colour of its number on the Camelot wheel
