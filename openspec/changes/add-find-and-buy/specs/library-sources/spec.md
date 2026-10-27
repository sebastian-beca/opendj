## ADDED Requirements

### Requirement: Find and buy
Selecting tracks without a local file and choosing Find and buy SHALL search the stores, artist-direct first, and list each track's best matches with links.

#### Scenario: Spotify playlist
- **WHEN** Find and buy runs on a Spotify playlist
- **THEN** each track not in the collection gets store matches, Bandcamp first

### Requirement: Artist first
Store links SHALL order the artist's own page or shop first, then Bandcamp, then other stores.

#### Scenario: Order
- **WHEN** a track is sold on the artist's site and Beatport
- **THEN** the artist's site is listed first
