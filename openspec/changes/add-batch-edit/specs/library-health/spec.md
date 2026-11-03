## ADDED Requirements

### Requirement: Fix names for selected tracks
The track menu for selected tracks SHALL offer Fix Names, a dialog with Find and Replace (in one field or all names, with match case, whole words and pattern), Change Case (Title Case keeping small words lower and DJ, MC and UK in capitals; Sentence case; UPPER; lower), Remove Brackets and Swap Artist and Title, previewing the first changes and how many tracks change. Review SHALL fill the review table with the proposed changes and write nothing until Apply. No tool SHALL empty a track or artist name.

#### Scenario: Remove Original Mix
- **WHEN** Find and Replace removes "(Original Mix)" from the titles of an Inbox
- **THEN** the review table lists each changed title, and the files are unchanged until Apply
