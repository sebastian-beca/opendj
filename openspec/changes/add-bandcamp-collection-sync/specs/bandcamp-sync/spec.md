## Purpose

Bringing a DJ's Bandcamp purchases into the library in lossless quality, without manual downloads, as an add-on.

## ADDED Requirements

### Requirement: Sign in
The Bandcamp add-on SHALL let the DJ sign in from its Settings page and keep the session in the system keychain.

#### Scenario: Signed in
- **WHEN** the DJ signs in
- **THEN** their purchases are listed in the Bandcamp source

### Requirement: Download purchases
New purchases SHALL download as FLAC into the purchase inbox, once each, and land in Inbox analysed.

#### Scenario: New purchase
- **WHEN** an album is bought on Bandcamp
- **THEN** after the next sync its tracks are in Inbox as FLAC
