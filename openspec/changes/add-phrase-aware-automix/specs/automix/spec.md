## ADDED Requirements

### Requirement: Phrase-aware
When both tracks have phrases or automix points, Automix SHALL start the mix at the outgoing outro and align the incoming intro to it.

#### Scenario: Intro over outro
- **WHEN** both tracks have phrases
- **THEN** the mix starts on the outgoing outro's first bar

#### Scenario: Imported automix point wins
- **WHEN** the outgoing track has an automix point from VirtualDJ and detected phrases
- **THEN** the mix starts at the VirtualDJ automix point
