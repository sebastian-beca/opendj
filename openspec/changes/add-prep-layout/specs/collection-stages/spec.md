## ADDED Requirements

### Requirement: Prep checklist
A Checklist tab in the side panel SHALL list what makes a track ready: lines the app checks (grid set, four hot cues, genre, rating and cover) and lines the DJ adds in Settings › Library › Stages and ticks by hand, stored per track, with an optional personal score kept in its own column. The Prep header SHALL show how many lines are ticked, and the Play Ready button SHALL light up when all are.

#### Scenario: Own line
- **WHEN** the DJ adds "Mix-in and mix-out checked" and ticks it for a track
- **THEN** the line stays ticked for that track and counts towards ready

### Requirement: Play Ready and next
Play Ready, next track SHALL move the track to Play Ready, lock its grid and load the next track of the In Prep list into Deck 1. It SHALL work whether or not every line is ticked.

#### Scenario: Next track
- **WHEN** Enter is pressed in Prep
- **THEN** the track leaves In Prep, shows in Play Ready and the next In Prep track loads
