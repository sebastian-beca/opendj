## ADDED Requirements

### Requirement: Plugin effects
An effect slot SHALL load an installed VST3, CLAP or AU effect, running out of the audio thread's process so a crashing plugin can't stop the decks.

#### Scenario: Crash
- **WHEN** a plugin crashes
- **THEN** its slot goes dry and the decks keep playing
