## Purpose

Bringing a DJ's Bandcamp purchases into the library in lossless quality, without manual downloads.

## ADDED Requirements

### Requirement: Sign in
The DJ SHALL be able to sign in to Bandcamp from Settings, with the session kept in the system keychain, and sign out again.

#### Scenario: Signed in
- **WHEN** the DJ signs in
- **THEN** their purchases are listed in the Bandcamp source

### Requirement: Download purchases
Sync SHALL download each new purchase once, as FLAC unless the DJ picked another format, into the purchase inbox, spaced one at a time; the tracks SHALL land in Just bought analysed.

#### Scenario: New purchase
- **WHEN** an album is bought on Bandcamp
- **THEN** after the next sync its tracks are in Just bought as FLAC

#### Scenario: Already synced
- **WHEN** sync runs again with no new purchases
- **THEN** nothing is downloaded
