## ADDED Requirements

### Requirement: Tag groups
The library SHALL keep the DJ's tags in groups the DJ names, each tag an entry of its own within its group (matched whatever its case, spaces allowed, no `#`). A group SHALL allow several tags per track or one. Each group SHALL be searchable by its name and by its place, and SHALL appear as a field in the Info tab, a filter chip and a column.

Tags another app brings SHALL go into the DJ's groups: each place that app keeps tags (VirtualDJ's User 1 and User 2, a rekordbox My Tag category) SHALL map to one group, made with that place's name the first time it is seen unless a group of that name exists.

#### Scenario: A tag with a space
- **WHEN** "Peak Time" is added to a track's Vibe group
- **THEN** `vibe:"peak time"` finds the track and the Info tab shows one chip "Peak Time"

#### Scenario: Old fields kept
- **WHEN** a library from before tag groups opens with Mood and Instruments named in settings and "#uplifting" in User 1
- **THEN** a Mood group holds the tag "uplifting" on that track

#### Scenario: rekordbox categories
- **WHEN** a rekordbox collection with My Tag category "Situation" and tag "Warm Up" is imported
- **THEN** a Situation group holds "Warm Up" on the tagged tracks

#### Scenario: One per track
- **WHEN** a group is set to One and a second tag is added to a track in it
- **THEN** the new tag replaces the old one
