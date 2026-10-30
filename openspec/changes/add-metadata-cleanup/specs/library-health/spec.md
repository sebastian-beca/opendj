## ADDED Requirements

### Requirement: Clean-up check
Collection Health SHALL have a Clean up check listing tracks whose tags break house-style rules (stray spaces, case, remix or feat. in the title, near-identical artist or genre spellings).

#### Scenario: Feat in title
- **WHEN** a title is "Track (feat. X)"
- **THEN** the check proposes title "Track" and artist featuring X

### Requirement: Review table
Fixing SHALL show a table of proposed changes per track and field, each ticked by default when safe, so the DJ can untick any before applying.

#### Scenario: Untick
- **WHEN** one proposed change is unticked and Apply is pressed
- **THEN** every other change is applied and that one is not

### Requirement: Protect hand edits
Fields the DJ edited by hand, or that an active import owns, SHALL NOT be changed unless the DJ ticks them explicitly.

#### Scenario: Hand edit
- **WHEN** the DJ typed an artist in the Info tab
- **THEN** clean-up proposes no change to it by default
