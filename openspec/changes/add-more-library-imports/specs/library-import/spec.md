## ADDED Requirements

### Requirement: More importers
Settings › Library SHALL offer Import from Serato, Traktor and Apple Music, each with the three-step wizard, read-only, kept up to date at start, and removable.

#### Scenario: Traktor cues
- **WHEN** a Traktor NML with cues and loops is imported
- **THEN** tracks keep their cues, loops, grid and playlists

### Requirement: Cross-source matching
When two imports point at different files of the same track (same artist, title and duration within 2 s), openDJ SHALL ask which copy to keep, defaulting to the one already in the stages, and merge the rest onto it.

#### Scenario: Same song twice
- **WHEN** VirtualDJ and Traktor hold different copies of a track
- **THEN** the library has one track with both apps' cues
