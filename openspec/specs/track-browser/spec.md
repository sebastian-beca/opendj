# track-browser Specification

## Purpose
The library browser: the tree, the track list with its columns, selection and drag and drop, colours, and the side panel.

## Requirements

### Requirement: Tree
The tree SHALL start with the stages (when set up), then All Tracks with Collection Health beneath, then the DJ's playlists, folders and smart lists, imported folders, and History, each with its count.

#### Scenario: Stages first
- **WHEN** triage and collection folders are set
- **THEN** the tree starts with Inbox, Triage and Collection

### Requirement: Columns
Right-clicking a heading SHALL choose which columns show (tag fields included) or reset them. Dragging a divider SHALL resize only the column on its left, with later columns moving along unchanged; dragging a heading SHALL move it, with a lilac drop marker; clicking SHALL sort. Columns SHALL be remembered.

#### Scenario: Resize one column
- **WHEN** the divider right of Artist is dragged 40 px right
- **THEN** Artist is 40 px wider and every later column keeps its width

### Requirement: Prep column
The optional Prep column SHALL show a dot each for tags, rating, and tempo and key.

#### Scenario: Missing rating
- **WHEN** a track has tags, tempo and key but no rating
- **THEN** its rating dot is empty

### Requirement: Waveform column
The Waveform column (shown by default, and offered in the heading menu like any other column) SHALL draw each analysed track's overview as a mini waveform mirrored about its middle: the RMS body in Electric Lilac (#B69CFF), the peaks around it fainter, white on the selection highlight and dimmed for missing tracks. A track not yet analysed SHALL show nothing. It SHALL resize and move like other columns, the waveform stretching to the width. Clicking its heading SHALL sort analysed tracks first.

The overview SHALL be a small summary (256 slices of peak and RMS) kept beside each stored analysis and made from the stored analysis the first time it is missing, read off the UI thread, newest request first. Each row's waveform SHALL be drawn once into a small image and only rows in view (and a screenful either side) SHALL hold one, so the list does no drawing of its own between changes and a long list costs no more than a screenful. A track analysed on a deck or in the background SHALL show its waveform at once. Clearing stored analyses SHALL clear the overviews too.

#### Scenario: Analysed and not analysed
- **WHEN** the list shows a track that has been analysed and one that hasn't
- **THEN** the first row shows a lilac mini waveform and the second an empty cell

#### Scenario: Loaded on a deck
- **WHEN** a track with an empty Waveform cell is loaded on a deck and analysed
- **THEN** its row and the Info panel fill in as the deck's waveform does, and show the whole track once the analysis finishes, without the list being opened again

### Requirement: Selection
Click SHALL select a row, shift-click a range, and actions from the track menu apply to the whole selection. The focused list's selection SHALL be lilac (#8C72D9, white text); the other list's gray.

#### Scenario: Range select
- **WHEN** a row is clicked and another shift-clicked
- **THEN** both and every row between are selected

### Requirement: Row look
Rows SHALL alternate #161819 and #101112; keys compatible with the playing deck SHALL be lilac; tracks loaded on a deck SHALL carry a badge with the deck number.

#### Scenario: Deck badge
- **WHEN** a track is on deck 3
- **THEN** its row shows a 3 badge

### Requirement: Drag and drop
Rows SHALL drag onto decks, lists, stages, the Automix and Sidelist tabs; a deck's cover SHALL drag out onto lists.

#### Scenario: Drag to deck
- **WHEN** a row is dropped on deck 2
- **THEN** it loads on deck 2

### Requirement: Side panel
The side panel SHALL have icon tabs Automix, Sidelist, Linked and Info, with a lilac underline on the open one.
Info's preview bar SHALL show the track's waveform (the same overview as the Waveform column), gray with the part played in lilac and a white playhead, and click or drag to seek; a track not yet analysed SHALL keep the plain line.

#### Scenario: Open Info
- **WHEN** the Info tab icon is clicked
- **THEN** the selected track's details show
