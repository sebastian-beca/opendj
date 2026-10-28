# Export to USB

> **Experimental.** USB export hasn't been tried on a Pioneer player yet.
> Check the stick on a player before you rely on it at a gig, and tell us
> how it went (see [Feedback and logs](feedback.md)).

openDJ can put your lists on a USB stick that Pioneer DJ players read, the
way rekordbox's USB export does: the music, its beat grid, cues, loops and
waveforms, album art, and your playlists and folders.

## Exporting

1. Plug in a USB stick formatted **FAT32** or **exFAT**.
2. In the tree on the left, right-click a playlist, folder, smart list or
   stage and choose **Export to USB…**. A folder takes its lists and
   subfolders with it.
3. Pick the stick under **Drive** (press **Refresh** if it isn't there yet).
4. Pick which **Players** the stick is for. **All players** is right unless
   you know you only need one of the two:
   - **CDJ-3000, NXS2 and older, XDJ-XZ, XDJ-RX3** read the Device Library
     (`export.pdb`).
   - **CDJ-3000X, XDJ-AZ, OPUS-QUAD, OMNIS-DUO** read OneLibrary
     (`exportLibrary.db`).
5. Press **Export**. You can press **Stop** at any time; the stick then
   keeps the library it had.

Tracks that were never analysed are analysed first, so the first export of
a big list takes a while. Music already on the stick isn't copied again.

## What ends up on the stick

- The audio, under `Contents/Artist/Album/` on the stick.
- Each track's analysis: beat grid, hot cues A–H with their names and
  colours, memory cues and loops, and the waveforms players draw (blue,
  RGB and three-band).
- Title, artist, album, genre, label, key, BPM, rating, colour, comment,
  year and play count. Keys are written the way openDJ shows them (Camelot
  or notes, from **Settings › Analysis**).
- Album art.
- The lists you exported, in their folders.

## Good to know

- **The stick's playlists are replaced.** Each export writes the stick's
  library afresh with the lists you picked. To have several lists on one
  stick, put them in a folder and export the folder.
- **Hot cues past H** are left out, as players have eight. The result says
  how many.
- **Tracks that can't be read** (a missing file, say) are left out, and the
  result lists them.
- Exporting the same lists again is quick: only new music is copied.
