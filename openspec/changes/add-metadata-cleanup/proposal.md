# Proposal

Planned for: **v0.4 · Your collection, cleaned up (collection maintenance PR 4)**

## Why

601 of Seba's tracks had no artist or title and names are inconsistent. Fill-from-file-name fixes one case; a clean-up pass should propose consistent names and write them to the files.

## What Changes

- A clean-up review table: proposed changes per field, ticked by default when safe, untick to skip.
- House-style rules: trim spaces, fix case, move "(Remix)" and "feat." to their fields, merge near-identical artist and genre spellings.
- Write accepted changes to the files and the library together, waiting for iCloud downloads.
- Never overwrite fields the DJ edited by hand or that an import owns, without asking.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-health`: a Clean up check with a review table
- `track-info`: tag writes wait for files that are only in the cloud

## Out of Scope

- Online lookups (see add-online-metadata-lookup).

## Impact

crates/library health rules, crates/engine tags.rs, app browser/health.rs.
