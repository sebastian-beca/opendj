## MODIFIED Requirements

### Requirement: Info tab
The Info tab SHALL follow the selected track or a chosen deck and show its cover, title, artist and an editable rating at the top, then three tabs, the one last shown remembered between runs:
- Track: editable Track, Artist, Remix, Album and Genre, then Year with Track #, BPM with Key, and Gain with the length, two to a line.
- Tags: the DJ's tag fields, Grouping and Comment.
- File: format, audio, size, the file with Show in Finder, folder, play count, date added and last played.

Tempo and key SHALL be shown once, on the Track tab, not repeated in the header.

#### Scenario: Edit a title
- **WHEN** the Track field is changed and confirmed
- **THEN** the list shows the new title

#### Scenario: File details on their own tab
- **WHEN** a track is shown on the Track tab
- **THEN** its bitrate, size and folder are not listed until the File tab is chosen

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

Adding or removing a tag SHALL apply at once to every track shown, as one logged change that Undo takes back. The track menu SHALL offer Add Tag with tags added lately first, then the collection's most used, a box to find one, and a new tag typed there.

#### Scenario: Add a mood
- **WHEN** "#sunset" is added in the Mood field with thirty tracks selected
- **THEN** the thirty tracks carry "#sunset" and their other tags are unchanged

#### Scenario: A tag on some
- **WHEN** "#synth" is on two of three selected tracks
- **THEN** its chip reads "#synth 2/3", and clicking it puts it on the third
