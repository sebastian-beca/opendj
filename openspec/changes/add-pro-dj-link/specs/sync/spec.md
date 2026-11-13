## ADDED Requirements

### Requirement: Pro DJ Link as sync leader
The sync leader choice (`[Master],sync_leader_source`) SHALL also offer **Pro DJ Link** while Pro DJ Link is switched on. With it chosen, decks with SYNC on SHALL follow the network's tempo master in tempo, beat and bar. When there is no tempo master on the network, synced decks SHALL keep the last tempo.

#### Scenario: CDJ leads
- **WHEN** Pro DJ Link is chosen and the tempo master CDJ plays at 126 BPM
- **THEN** a deck with SYNC on plays at 126 BPM with its downbeats on the CDJ's bar starts, and follows within two beats when the CDJ's tempo changes

#### Scenario: Master leaves
- **WHEN** the tempo master CDJ stops being master and no other takes the role
- **THEN** synced decks keep playing at the last tempo
