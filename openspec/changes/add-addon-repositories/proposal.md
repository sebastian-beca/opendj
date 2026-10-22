# Proposal

Planned for: **v0.5 · Buy it from the artist**

## Why

Add-ons already run as separate programs from the add-ons folder (phase 1, PR #36), but installing one means copying files by hand. Store add-ons and Soulseek should install like apps, from repositories the DJ chooses.

## What Changes

- Settings › Add-ons with Installed, Available and Repositories.
- A repository is one `opendj-addons.json` at a URL listing each add-on with a download and checksum per platform; the official one is built in, others are marked unsupported.
- Private GitHub repositories with a read-only token kept in the Keychain or Secret Service.
- Update checks at start and at most daily, never installing while a deck plays.
- Add-ons restart with a growing delay when they crash.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `add-ons`: Settings › Add-ons with repositories, install, update and remove

## Out of Scope

- Sandboxing add-ons (they run with the user's rights; the trust story is shown, not enforced).
- Third-party audio plugins (v2.0).

## Impact

crates/plugins remote, new repository/index code, app prefs Add-ons page, keychain access.
