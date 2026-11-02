## ADDED Requirements

### Requirement: Sync leader source
The MASTER panel SHALL offer where the sync leader comes from (`[Master],sync_leader_source`): **Deck** (the master deck, as now and the default), **Link** (the Ableton Link session) or **Clock** (the MIDI clock input). With Link or Clock chosen, decks with SYNC on SHALL follow that source's tempo and beat, Link SHALL stop leading the session, and MIDI clock out SHALL keep sending the leader's tempo.

#### Scenario: Live act leads
- **WHEN** a live act's Ableton Live set is in the Link session and the DJ chooses Link as the leader
- **THEN** a deck with SYNC on plays at Live's tempo, in phase with its bars, and follows when the act changes tempo

#### Scenario: Drum machine leads
- **WHEN** Clock is chosen and a drum machine sends clock at 132 BPM and Start
- **THEN** a deck with SYNC on plays at 132 BPM with its downbeats on the drum machine's bar starts
