# library-search Specification

## Purpose
Finding tracks: the search language (Mixxx's syntax with additions), the filter bar's chips, the DJ's own tag fields, and harmonic and tempo matching against the playing track.

## Requirements

### Requirement: Search syntax
Plain words SHALL match artist, title, album, genre, comment, grouping and tags; quotes match a phrase; `field:value` searches one field; `field:=value` the whole field and `field:""` an empty one; `-` excludes; all terms must match unless joined by `|` or `OR`.

#### Scenario: Exclude a word
- **WHEN** the DJ searches `house -deep`
- **THEN** house tracks without "deep" are listed

### Requirement: Numbers and dates
`bpm:`, `rating:`, `played:`, `year:` and `duration:` SHALL take a value, a comparison or a range; `added:` and `lastplayed:` SHALL take days or weeks ago or a date.

#### Scenario: Recently added
- **WHEN** the DJ searches `added:<7d`
- **THEN** tracks added in the last week are listed

### Requirement: Mixing matches
`~key:8A` SHALL find keys that mix with 8A, `~bpm:124` tempos within 6 % (also at half or double), and `~deck` tracks that mix with the playing track by key and tempo.

#### Scenario: Mixes with the playing track
- **WHEN** a deck plays 8A at 124 BPM and the DJ searches `~deck`
- **THEN** tracks in compatible keys within 6 % of 124 BPM are listed

### Requirement: Status keywords
`color:<name>`, `stage:inbox|triage|collection|none`, and `is:` with missing, duplicate, played, cued, prepped and each Library Health check SHALL filter by those states.

#### Scenario: Prepared tracks
- **WHEN** the DJ searches `is:prepped`
- **THEN** only tracks with artist, title, rating, tempo and key are listed

### Requirement: Own tag fields
The DJ SHALL name the grouping field and any number of tag fields in Settings › Library › Fields. Each name SHALL work as a search word (`mood:uplifting`), as does each number (`tag1:`), with `#` optional. Removing a field SHALL keep its tags on tracks.

#### Scenario: Named field
- **WHEN** tag field 1 is named Mood and the DJ searches `mood:uplifting`
- **THEN** tracks tagged uplifting in that field are listed

### Requirement: Filter bar
The filter bar SHALL narrow the list with chips (genre, BPM range or near the playing track, key or mixes with the playing track, rating, colour, year, date added, tag fields and more) that combine with each other and the search. **+** adds any field, "Keep on the bar" pins it, **Clear** empties all, and **Save** keeps search and chips as a smart list. The bar SHALL start hidden.

#### Scenario: Chips and search together
- **WHEN** the Genre chip has House ticked and the search is `vocal`
- **THEN** only house tracks matching "vocal" are listed

### Requirement: Search help
Resting the pointer on the search box SHALL show everything search understands.

#### Scenario: Hover help
- **WHEN** the pointer rests on the search box
- **THEN** a tooltip lists the search keywords
