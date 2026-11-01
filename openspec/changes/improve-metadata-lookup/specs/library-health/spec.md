## MODIFIED Requirements

### Requirement: Look up online
Missing info and Junk in names SHALL offer Look up online, which identifies each track by fingerprint (with an AcoustID key) or by artist and title, searching as tagged and then without bracketed parts and guests, and proposes field values in the review table with their source and how sure the match is. Placeholder tags SHALL count as empty, and an "Artist - Title" file name SHALL stand in for a missing artist or title.

#### Scenario: DJ title
- **WHEN** a track tagged "Kerala (Original Mix)" by "Bonobo" is looked up
- **THEN** MusicBrainz is also searched for "Kerala" by "Bonobo", and the surest match is proposed

#### Scenario: Unknown artist
- **WHEN** a track tagged "Unknown Artist" in a file "Bonobo - Kerala.mp3" is looked up
- **THEN** it is searched as Bonobo, and the artist change is ticked when the match is sure or nothing is found

#### Scenario: Genre guessed from the artist
- **WHEN** the recording has no genre on MusicBrainz but its artist has
- **THEN** the artist's genre is proposed unticked

### Requirement: Why tracks weren't found
A finished look-up SHALL say how many tracks were found, how many by their sound, how many aren't on MusicBrainz, how many couldn't be looked up because the service was unreachable or busy, and how many were only in iCloud and so not fingerprinted.

#### Scenario: MusicBrainz busy
- **WHEN** MusicBrainz refuses a request twice
- **THEN** that track counts as couldn't be looked up, not as not found

### Requirement: Fill from file name reviewed
Fill from file name SHALL open the review table with the artist and title from each file's name for fields that are empty or say nothing, and SHALL write nothing until Apply.

#### Scenario: Reviewed
- **WHEN** Fill from file name runs on 20 tracks
- **THEN** the review table lists the proposed names and the files are unchanged until Apply
