# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (follow-up to collection maintenance PRs 4 and 5; 2026-11-01)

## Why

A real library had 107 tracks under "Missing or poor metadata" and found that Look up online matched few of them, and that Fill from file name wrote to the files without showing what it would do. Reading the code showed why the look-up missed: it searched MusicBrainz for the exact tagged title, so "(Original Mix)", "[Label]" and "feat." killed the match; "Unknown Artist" was searched as a name; genres only came from Discogs; a busy MusicBrainz (503) counted as "not found"; and the page didn't say which field each track lacked.

## What Changes

- Look up online searches MusicBrainz as tagged, then without bracketed parts and guests, then by the words in any order; the surest result wins, and matches under 50% are dropped.
- Placeholders ("Unknown Artist", "Track 01") count as empty: they aren't searched by and their fields get ticked fills. The file name ("Artist - Title") stands in for a missing artist or title in the search and is offered in the review table when nothing better was found.
- Genres come from MusicBrainz: the recording's (ticked like other fills), or else the artist's (shown, never ticked). Discogs styles still win with a token. The year is the recording's first release.
- Requests are paced per service (MusicBrainz 1/s, AcoustID 3/s, Discogs 1/s); a failed request is asked once more after a pause; the summary tells found, by sound, not on MusicBrainz, unreachable and iCloud-only apart.
- Fill from file name opens the review table instead of writing at once.
- The Missing or poor metadata page says what the flagged tracks lack ("80 no genre, 61 no year, 12 no artist or title").

## Capabilities

### Modified Capabilities

- `library-health`: Look up online, Fill from file name

## Out of Scope

- Paid metadata services; Beatport or store look-ups (v0.6 stores).
- Moving or renaming files (stays off while another DJ app may still use the same files).

## Impact

app lookup.rs, browser/cleanup.rs, browser/health.rs, coverart.rs (`try_json`), library health.rs (`says_nothing` public). User guide: Collection Health.
