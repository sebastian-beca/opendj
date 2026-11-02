# Tasks

## 1. rekordbox XML export

- [ ] 1.1 Writer beside the reader: tracks (tags, rating, colour, dates, play count, key in rekordbox notation), `TEMPO` per grid marker, `POSITION_MARK` for hot cues (number, colour), memory cues and loops; list tree with folders; URL-encoded `file://localhost/` locations
- [ ] 1.2 Round-trip test through `rekordbox/xml.rs` on a built collection with variable grids, loops and nested folders
- [ ] 1.3 Timing check against rekordbox 7 on Seba's Mac with MP3 (CBR and VBR, with and without LAME header), AAC and FLAC; add an MP3 offset correction if rekordbox reads positions from a different start
- [ ] 1.4 app: Export Collection as rekordbox XML…, Export as rekordbox XML… on lists and folders, progress line; Settings › Collection › General "Keep rekordbox.xml up to date" with path, debounced background writer
- [ ] 1.5 Update the user guide (moving your library to and from other apps, with Lexicon / Rekord Buddy notes)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
