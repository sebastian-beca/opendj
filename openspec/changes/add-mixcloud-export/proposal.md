# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-11-03, from a user request)

## Why

DJs record a set to put it on Mixcloud. Mixcloud wants a tracklist with start times; typing it in by hand after a two-hour set is the step most DJs skip, and Mixcloud ranks and licenses shows by it. openDJ already knows exactly when each track became audible in a recording (the recorder's `.cue` sheet), so it can hand Mixcloud a finished show. VirtualDJ and Mixxx write a `.cue` file and stop there; rekordbox makes the DJ type the tracklist in.

Separately, a new recording only appeared in the Mixed Sets source after the next library scan, which made it look as if recording had failed.

## What Changes

- A recording joins the Mixed Sets source as soon as it stops.
- The recorder writes `<name>.tracklist.txt` beside the `.cue`: `0:00 Artist - Title` lines, ready to paste into a show's description.
- **Export for Mixcloud** on a mixed set's track menu makes `<name> (Mixcloud).mp3`: 320 kbps, all parts of a split recording joined, with an ID3v2.4 tag naming the mix and carrying the tracklist as chapters (`CHAP`/`CTOC`) and as a comment. Long recordings are streamed, never held in memory. The file is shown in Finder when done.
- Step 2: **Upload to Mixcloud**. The DJ signs in to their own Mixcloud account in the browser (OAuth); openDJ uploads the show with its name, description, up to five tags, cover and the tracklist as `sections-N-artist/song/start_time`, checking Mixcloud's limits (4 GB, 1000-character description) before sending. Mixcloud's token exchange needs the app's client secret and offers no desktop flow, so where that secret lives is an open question (a small token-exchange function on opendj.app is the proposal).

## Capabilities

### New Capabilities

- `mixcloud-export`: a recorded mix as a Mixcloud-ready show with its tracklist

### Modified Capabilities

None.

## Out of Scope

- Mixcloud Live (RTMP), which belongs to add-live-broadcast step 3.
- Tracklists for mixed sets openDJ didn't record (no `.cue`): they export without chapters.
- SoundCloud and YouTube uploads.

## Difficulty

**Small: 2 to 3 days, two PRs.** Step 1 reuses the recorder's cue sheet and the broadcast crate's LAME encoder. Step 2 is OAuth plus one multipart POST, and the secret's home.

## Performance

Nothing new runs on the audio thread. The export decodes and encodes on its own thread, packet by packet.
