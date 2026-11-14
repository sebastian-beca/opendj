## ADDED Requirements

### Requirement: Find across download sources
The rail SHALL show a Find entry above the add-on sources whenever at least one installed source downloads. A Find search SHALL go to every download source at once, or only to the sources picked in the chips under the search bar (Any by default, kept for the session). Results SHALL show in one list, each with a chip naming its source, and Preview and Download SHALL act through that source.

#### Scenario: One search, three sources
- **WHEN** "Bonobo - Kerala" is searched in Find with Soulseek, YouTube and SoundCloud installed
- **THEN** results from all three show in one list, each marked with its source

#### Scenario: Narrowed to one source
- **WHEN** only the SoundCloud chip is picked and a search runs
- **THEN** only SoundCloud is asked

### Requirement: Ranked by quality
Find results SHALL be ranked by quality tier (lossless, 320 kbps, 256 kbps, lower), then by how closely artist, title and length match the search, then by the source order set in Settings › Sources.

#### Scenario: Lossless first
- **WHEN** Soulseek finds a FLAC and YouTube finds a 256 kbps copy of the same track
- **THEN** the FLAC is listed first

### Requirement: Wanted tries every source
Get on a Wanted row, and Wanted's auto-download, SHALL search Find and download the best result that meets the Prefer setting. When no source has one, the row SHALL say so and stay on Wanted.

#### Scenario: Not on Soulseek
- **WHEN** a Wanted track set to Prefer Any has no Soulseek result and a YouTube result
- **THEN** the YouTube copy is downloaded

#### Scenario: Below Prefer
- **WHEN** Prefer is Lossless and only a YouTube copy is found
- **THEN** nothing is downloaded and the row says no lossless copy was found

### Requirement: Lossy downloads marked
A download below 256 kbps, or from a source that says its audio is lossy, SHALL be marked Lossy and listed in Health.

#### Scenario: A 128 kbps download
- **WHEN** a 128 kbps YouTube download finishes
- **THEN** Health lists it under Lossy
