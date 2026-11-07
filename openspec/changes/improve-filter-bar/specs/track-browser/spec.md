## ADDED Requirements

### Requirement: Linked filter values
Each filter chip SHALL list and count its values over the tracks left by every other filter: the typed search, the other chips and BPM or Key following a deck. The chip's own picks SHALL NOT narrow its list. Values SHALL be ordered by: picked first, then most tracks, then most recently picked, then the field's own order (leading numbers as numbers, then A–Z). A tag group set to Own order SHALL skip the track count. Values with no tracks SHALL be offered behind "Show N more", and a picked value SHALL always be listed.

#### Scenario: Key narrows Trip Stage
- **WHEN** Key is set to 8A and the Trip Stage chip is opened
- **THEN** it lists the stages that have 8A tracks, with counts of 8A tracks, and the others behind "Show N more"

#### Scenario: Most tracks first
- **WHEN** Mood has Dark on 40 tracks of the list and Acid on 1
- **THEN** Dark is listed above Acid

### Requirement: Arranging the filter bar
The DJ SHALL be able to drag a chip to a new place on the bar, and to remove a chip by dragging it off the bar or with "Remove from bar". × on a chip SHALL clear its value and keep the chip. The bar's chips and order SHALL be the same for every list.

#### Scenario: Clear keeps the chip
- **WHEN** × is clicked on the Mood chip set to Dark
- **THEN** Mood no longer filters and the chip stays on the bar

### Requirement: Chip options
The Key chip SHALL offer Exact, Harmonic or Fuzzy matching. The chip's choice SHALL override the app-wide key mixing setting and stay until changed.

#### Scenario: Fuzzy on the chip
- **WHEN** the app setting is Harmonic and the Key chip is set to Fuzzy
- **THEN** the list shows keys that mix by the Fuzzy rule, and the chip keeps Fuzzy after Clear
