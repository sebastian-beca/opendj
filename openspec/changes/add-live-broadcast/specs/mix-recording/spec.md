## ADDED Requirements

### Requirement: Broadcast
Settings SHALL hold broadcast servers; a BROADCAST button SHALL stream the master with the current track as metadata, reconnecting on drops.

#### Scenario: Reconnect
- **WHEN** the connection drops for 5 s
- **THEN** streaming resumes by itself
