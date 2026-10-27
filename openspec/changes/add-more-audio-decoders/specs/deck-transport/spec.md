## ADDED Requirements

### Requirement: Supported formats
Decks SHALL load MP3, AAC/M4A (including HE-AAC and ALAC), FLAC, WAV, AIFF, Ogg Vorbis, Opus and WavPack.

#### Scenario: Opus
- **WHEN** an .opus file is dropped on a deck
- **THEN** it plays

### Requirement: Undecodable files
A file that can't be decoded SHALL leave the deck empty and show why.

#### Scenario: Broken file
- **WHEN** a corrupt file is loaded
- **THEN** the deck shows that the file can't be read
