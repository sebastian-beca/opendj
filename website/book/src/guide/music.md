# Your music

openDJ reads your `~/Music` folder. To use another folder, start openDJ with
the `OPENDJ_MUSIC` environment variable:

```sh
OPENDJ_MUSIC=/path/to/music opendj
```

The **Files** source on the left of the library lets you browse any folder on
your computer and load tracks from it.

## The library

Your library keeps playlists, smart lists, ratings, colours and play history.

Search matches titles, artists, BPM and key. The **?** next to the search box
lists everything search understands.

## Bringing in a VirtualDJ library

Turn on **Settings › Library › Import from VirtualDJ** to bring your
VirtualDJ library into openDJ. openDJ reads `database.xml` from
`~/Library/Application Support/VirtualDJ` (or `~/.vdj` on Linux) and only
ever reads it.

Each track keeps VirtualDJ's tags, stars, colour, comment, play count, dates,
BPM, beat grid, key, hot cues with their names and colours, saved loops,
gain, and its remix and automix points. Anything VirtualDJ leaves empty is
filled in from the file's own tags.

Below the switch you choose which lists come across, into a **VirtualDJ**
folder in the library: playlists, virtual folders, filter folders (as smart
lists, off by default) and history (in the History tab).

openDJ checks VirtualDJ's files each time it starts, and brings in what has
changed. VirtualDJ's tags win, but grids and cues you edited in openDJ are
kept. The VirtualDJ folder is made again each time, so change those lists in
VirtualDJ, or copy one out of the folder to keep editing it in openDJ.
