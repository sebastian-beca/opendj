## ADDED Requirements

### Requirement: Artist page
In Player mode, clicking an artist (the Player bar's artist, an artist in the search box's results, or Go to Artist) SHALL open their page over the track list, with their collection tracks (any credit counting) most played first, their albums newest first, and Play, Shuffle and Mix.

#### Scenario: From the bar
- **WHEN** "Midnight Drive" by Lilac Waves plays in Player and the DJ clicks "Lilac Waves" on the bar
- **THEN** Lilac Waves' page shows their tracks and albums over the list

### Requirement: Album page
Clicking an album on an artist's page, or Go to Album in Player, SHALL open the album's page with its tracks in disc and track order, its artist as a link to their page, and Play, Shuffle and Mix.

#### Scenario: Play from a track
- **WHEN** the DJ clicks the third track on an album's page
- **THEN** it plays now and the rest of the album follows it in Up Next

### Requirement: Playing a page
Play SHALL put the page's tracks in Up Next in the page's order and mix into the first now; Shuffle in a random order; Mix in the smoothest order by key and tempo.

#### Scenario: Mix an artist
- **WHEN** the DJ presses Mix on an artist's page
- **THEN** their tracks go in Up Next ordered so each mixes into the next, and the first plays now

### Requirement: Back and close
Pages SHALL stack: back SHALL return to the page before, then to the list; picking a list in the tree SHALL close every page. In the DJ layouts, Go to Artist and Go to Album SHALL keep narrowing the list instead.

#### Scenario: Back from an album
- **WHEN** the DJ opens an artist, then one of their albums, and presses back
- **THEN** the artist's page shows again

### Requirement: Home
Player mode SHALL open on Home, which shows shelves of the collection's recently played and recently added albums, the DJ's crates and lists (most recently added to first), and mixes made from the collection by genre, decade and key. A house button beside the search box SHALL return to Home.

#### Scenario: Open a crate from Home
- **WHEN** the DJ clicks the "Friday" crate on Home
- **THEN** the tree selects Friday and the list shows its tracks

### Requirement: Mixes
A genre, decade or key mix SHALL be offered only with at least 8 tracks, and SHALL open as a page of up to 50 of its most played and best rated tracks, in the smoothest order by key and tempo, with Play, Shuffle and Mix.

#### Scenario: A key mix
- **WHEN** the DJ opens "8A Mix" on Home
- **THEN** a page lists tracks that mix with 8A, ordered so each mixes into the next
