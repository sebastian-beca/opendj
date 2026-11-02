## MODIFIED Requirements

### Requirement: Info tab
The Info tab SHALL follow the selected track or a chosen deck and show its cover, title, artist, tempo, key, length and an editable rating at the top; then editable Genre, the tag fields and Comment; then Track, Artist, Remix, Album, Year, Track # and Grouping. A Details section (BPM, Key, Gain) and a File & stats section (duration, play count, first seen, last played, format, audio, size, file with Show in Finder, folder) SHALL start closed, each heading summing up what is in it, and SHALL open and close from their headings, remembered between runs.

#### Scenario: Edit a title
- **WHEN** the Track field is changed and confirmed
- **THEN** the list shows the new title

#### Scenario: Folded details
- **WHEN** a 126 BPM track in 2A is shown with Details closed
- **THEN** the Details heading reads "126 BPM · 2A" and the BPM and Key fields are not listed until it is opened

## ADDED Requirements

### Requirement: Editing several tracks
With several tracks selected, the Info tab SHALL edit them together. It SHALL show how many tracks are selected, their BPM range, number of keys, total time and how many files can take tags. A field the tracks share SHALL show its value; a field that differs SHALL show how many values it has and offer the values in use (most used first, with counts), keeping each track's own, clearing it in all, or typing a new value. Changes SHALL wait for Apply or Discard; Apply SHALL write them as one logged batch that Undo takes back, to the library and to the files' tags as for one track, and mark the typed fields as hand-edited.

#### Scenario: One album for many
- **WHEN** twelve tracks are selected, the Album field (3 values) is set to "Brightest Lights" and Apply is pressed
- **THEN** all twelve have that album and Cmd-Z restores each one's own

#### Scenario: Keep differing values
- **WHEN** only Genre is changed with several tracks selected
- **THEN** every other field keeps each track's own value

### Requirement: Tags on several tracks
Each tag field SHALL show its tags as chips: a tag on every selected track plainly, a tag on some with how many, which a click puts on all. A tag SHALL be removable from all and addable to all, with suggestions from tags already in the library.

#### Scenario: Add a mood
- **WHEN** "#sunset" is added in the Mood field with thirty tracks selected
- **THEN** the thirty tracks carry "#sunset" and their other tags are unchanged
