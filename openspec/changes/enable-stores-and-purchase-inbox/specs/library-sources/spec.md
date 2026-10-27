## ADDED Requirements

### Requirement: Store search
Bandcamp SHALL search in the app and show results; choosing one SHALL open its page in the browser to buy. Shopping SHALL offer other stores' own search pages filled with the search. openDJ SHALL never take payment.

#### Scenario: Buy on Bandcamp
- **WHEN** a Bandcamp result's Buy is clicked
- **THEN** its Bandcamp page opens in the browser

### Requirement: Purchase inbox
With the inbox on, audio arriving in its folder SHALL be unpacked, recognised (store tags, wanted list) and moved, copied or left (as set), then added to the Inbox stage. Unrecognised zips SHALL be left alone; files older than when the inbox was turned on SHALL be ignored.

#### Scenario: Bandcamp zip
- **WHEN** a Bandcamp album zip lands in Downloads
- **THEN** its tracks appear in Inbox, filed under Purchased/Bandcamp/Artist/Album

### Requirement: Wanted list
A store result or a missing track SHALL be addable to a wanted list, and a purchase matching it SHALL be ticked off.

#### Scenario: Ticked off
- **WHEN** a wanted track's purchase arrives in the inbox
- **THEN** it leaves the wanted list and is in Inbox

### Requirement: Inbox off by default
The purchase inbox SHALL be off until the DJ turns it on, so nothing in Downloads moves unasked.

#### Scenario: Fresh install
- **WHEN** openDJ runs for the first time
- **THEN** no file in Downloads is touched
