# collection-stages Specification

## Purpose
Stages take new music from where it lands to a prepared collection: Inbox, Triage (with triage crates) and Collection, each with its folder, with queued file moves and undo.

## Requirements

### Requirement: Stage folders
Settings › Library › Stages SHALL hold any number of Inbox folders, one Triage folder and one Collection folder. Every track SHALL get the stage of the folder its file is in; tracks elsewhere have no stage.

#### Scenario: Stage from folder
- **WHEN** a file sits in the Triage folder
- **THEN** it shows under Triage and matches `stage:triage`

### Requirement: Triage crates
Subfolders of the Triage folder SHALL be triage crates. Adding a track from a crate to the Collection SHALL also add it to the list named after the crate without "STG" (`Yoga STG` joins Yoga).

#### Scenario: Crate to list
- **WHEN** a track in `Yoga STG` is added to the Collection
- **THEN** it joins the Yoga list

### Requirement: Moving between stages
The track menu and dropping on a stage SHALL move tracks: To Triage (or a crate), Not interested (hides from Inbox), Add to Collection, Back to Triage, Back to Inbox. Undo SHALL take back the last batch.

#### Scenario: Undo
- **WHEN** five tracks are added to the Collection and Undo is chosen
- **THEN** they are back in Triage

### Requirement: Queued file moves
A stage change SHALL apply in openDJ at once. Its file SHALL only move while "Move files" is on (off by default); otherwise the move waits and is made when it is turned on. A file SHALL never be moved over another.

#### Scenario: Moves wait
- **WHEN** Move files is off and a track goes to Triage
- **THEN** it shows in Triage but its file stays where it was until Move files is on

### Requirement: Rename into Triage
Files moving from Inbox to Triage SHALL be renamed `Artist - Title (Remix)` from their tags unless "Rename files sent to Triage" is off.

#### Scenario: Clean name
- **WHEN** `track01_final.mp3` tagged Artist A, Title B, Remix C moves to Triage
- **THEN** it is named `Artist A - Title B (C).mp3`

### Requirement: Background analysis in Triage
Tracks entering Triage SHALL be analysed in the background so tempo and key fill in.

#### Scenario: Prep dots fill in
- **WHEN** a track enters Triage
- **THEN** its tempo and key dot fills once analysis finishes
