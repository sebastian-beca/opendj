# Proposal

Planned for: **v0.5 · Stems, performance and switching over**

## Why

Collection Health is a flat list of thirteen checks plus housekeeping rows, each with a big number and a Review button. Real problems (missing files), personal to-dos (no rating) and plain information (tracks removed, files in quarantine) look alike, the sidebar repeats every check and every case of copies as a row, and nothing says where to start or that order matters: non-music has to go before the duplicate check (audiobook chapters look like copies), relinking before merging copies, name clean-up before online lookup.

Other library tools point the way: Traktor's Consistency Check is a short report of counts with one action each; Lexicon runs each fix as scan, preview, confirm and archives instead of deleting; beets applies strong matches on its own and asks only about weak ones.

## What Changes

- **Five areas** group the checks by the question a DJ asks, in the order they are best fixed: **Files** (Can it play?: missing files, low bitrate), **Out of place** (Should it be here?: not music, long tracks, strays, outside folders), **Duplicates** (Same track twice?), **Info** (Can you find it?: poor metadata, junk in names, album tags, no cover) and **Prep** (Ready to play?: not analyzed, no rating). Prep is a goal, not a problem.
- **Overview dashboard**: the share of the collection that is clean (no Files, Out of place, Duplicates or Info finding), then the stages (Inbox, In Prep, Play Ready) each with its own clean bar and its biggest issue, then the five areas as numbered tiles with their checks; the first unfinished area is marked "Start here". Removed tracks, the quarantine folder, list tidy-up and genres sit in a quiet footer. The Overview row uses the Collection Health icon.
- **Sidebar**: Overview and the five areas with their counts, instead of a row per check and per case of copies. An area lists its tracks with a chip per check (per case for Duplicates) above the list.
- Later steps: each area page shows the problem and the proposed fix per track, applies the sure fixes at once and sends the rest to a one-by-one review card with keys and a play preview.

## Capabilities

### Modified Capabilities

- `library-health`: areas, Overview dashboard, area pages with chips

## Out of Scope

- New checks (iCloud offloaded files, corrupt files, no cues).
- Settings › Collection, reworked separately.

## Impact

library (`health::Area`, `Facts::has_problem`), app browser (health tree, chips, dashboard data), app ui (`HealthPanel`, `HealthBar`, tree icons). Mockups and research in the project notes (collection-health-redesign).
