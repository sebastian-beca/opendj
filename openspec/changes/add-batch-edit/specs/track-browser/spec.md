## MODIFIED Requirements

### Requirement: Selection
Click SHALL select a row, shift-click the rows from the last clicked row to it, and Cmd-click (Ctrl-click on Windows and Linux) SHALL add or drop one row, so any set of tracks can be selected. Actions from the track menu, dragging, rating, adding to a list and Automix SHALL apply to the whole selection. The focused list's selection SHALL be lilac (#8C72D9, white text); the other list's gray.

#### Scenario: Range select
- **WHEN** a row is clicked and another shift-clicked
- **THEN** both and every row between are selected

#### Scenario: Pick separate rows
- **WHEN** rows 2 and 9 are selected with Cmd-click and the selection is dragged to a crate
- **THEN** the crate gets those two tracks and not the rows between
