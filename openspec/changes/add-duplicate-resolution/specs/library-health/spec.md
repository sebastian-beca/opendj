## ADDED Requirements

### Requirement: Duplicate groups
The Duplicates check SHALL list duplicates in groups, each labelled with its case (same file, other format, other folder, likely match) and with a suggested keeper.

#### Scenario: Group shown
- **WHEN** a track exists as MP3 in Inbox and FLAC in Collection
- **THEN** both appear in one group with the FLAC suggested as keeper

### Requirement: Merge keeps everything
Merging a group SHALL move cues, loops, grid edits, tags, rating, play count and playlist slots onto the keeper, then move the other files to a quarantine folder without deleting them.

#### Scenario: Cues survive
- **WHEN** the non-keeper had hot cues and the keeper had none
- **THEN** after merging the keeper has those hot cues

### Requirement: Easy fixes in one click
Groups where the keeper rules agree and the merge loses nothing SHALL be fixable together with one action; the rest SHALL be reviewed one group at a time.

#### Scenario: Batch merge
- **WHEN** Fix easy duplicates is pressed
- **THEN** only groups with a clear keeper and no data loss are merged

### Requirement: Undo a merge
A merge SHALL be undoable from the track menu while the quarantined file still exists.

#### Scenario: Undo
- **WHEN** a merge is undone
- **THEN** the quarantined file and its library entry come back as they were
