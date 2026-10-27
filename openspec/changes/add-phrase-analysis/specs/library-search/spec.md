## ADDED Requirements

### Requirement: Phrase search
Search SHALL find tracks by phrase kind and length in bars, such as `has:intro>=16`.

#### Scenario: Long intros
- **WHEN** the DJ searches `has:intro>=16`
- **THEN** only tracks whose intro is at least 16 bars are listed
