## ADDED Requirements

### Requirement: Fix names for selected tracks
The track menu for selected tracks SHALL offer Find and Replace (in one field or all names, with match case, whole words and pattern), Change Case (Title Case keeping small words and DJ, MC and UK as they are; Sentence case; UPPER; lower), Remove Between brackets, Swap Artist and Title and Number in List Order. Each SHALL fill the review table with the proposed changes and write nothing until Apply.

#### Scenario: Remove Original Mix
- **WHEN** Find and Replace removes "(Original Mix)" from the titles of an Inbox
- **THEN** the review table lists each changed title, and the files are unchanged until Apply
