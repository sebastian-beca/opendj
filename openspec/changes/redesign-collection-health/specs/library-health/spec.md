## ADDED Requirements

### Requirement: Health areas
Collection Health SHALL group its checks into five areas, in this order: Files (missing files, low bitrate), Out of place (not music, long tracks, strays, outside folders), Duplicates, Info (missing or poor metadata, junk in names, album tags differ, no cover) and Prep (not analyzed, no rating). A track SHALL count as clean when no check of Files, Out of place, Duplicates or Info finds it; Prep SHALL NOT make a track unclean.

#### Scenario: Unrated track
- **WHEN** a track has every tag, plays and has no stars
- **THEN** it counts as clean and shows under Prep

### Requirement: Health overview
The Overview SHALL show the share of clean tracks, then each stage (Inbox, In Prep, Play Ready, when stages are set) with its track count, its share of clean tracks and its biggest issue, then one tile per area with its count and checks, marking the first area with findings as the place to start. Removed tracks, quarantined files, list tidy-up and genres SHALL show in a footer and SHALL NOT count as problems.

#### Scenario: Start here
- **WHEN** Files finds nothing and Out of place finds 28 long tracks
- **THEN** the Out of place tile is marked Start here

### Requirement: Area lists
The Health sidebar SHALL list Overview and the five areas with how many tracks each finds. Selecting an area SHALL list the tracks of one of its checks, with a chip per check (for Duplicates: all copies and each case) to switch between them; the check's fixes stay on the bar above the list.

#### Scenario: Switch check
- **WHEN** the DJ selects Info and clicks the No cover chip
- **THEN** the list shows the tracks without a cover and the bar offers Find covers
