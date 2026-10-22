## ADDED Requirements

### Requirement: Writes wait for downloads
A tag write to a file that is only in iCloud SHALL wait until the file is downloaded, and SHALL be retried then, without blocking the UI.

#### Scenario: Offloaded file
- **WHEN** a tag is changed on an offloaded file
- **THEN** the library changes at once and the file is written once downloaded
