## ADDED Requirements

### Requirement: Look up online
Missing info and No cover SHALL offer Look up online, which identifies each track by fingerprint and proposes field values and a cover in the review table.

#### Scenario: Year found
- **WHEN** a track with no year is looked up and matched
- **THEN** the review table proposes its release year

### Requirement: Cover size
Covers found online SHALL be the largest offered up to about 1024 px, embedded in the file when accepted.

#### Scenario: Big cover
- **WHEN** a 3000 px cover is the only one found
- **THEN** it is scaled to about 1024 px before embedding

### Requirement: Rate limits and privacy
Lookups SHALL respect each service's rate limits, run in the background, and send only fingerprints and tags, never audio.

#### Scenario: Many tracks
- **WHEN** 500 tracks are looked up
- **THEN** requests are paced within the services' limits and the UI stays responsive
