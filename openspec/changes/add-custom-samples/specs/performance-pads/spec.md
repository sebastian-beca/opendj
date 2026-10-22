## ADDED Requirements

### Requirement: Own samples
Dropping a file on a sampler pad SHALL load it there, kept across restarts, in banks of 8.

#### Scenario: Load sample
- **WHEN** a WAV is dropped on pad 3
- **THEN** pad 3 plays that file

### Requirement: Record from deck
A pad SHALL record the current loop of a deck as a sample.

#### Scenario: Capture
- **WHEN** record is pressed on an empty pad while a 4-beat loop plays
- **THEN** the pad holds that loop
