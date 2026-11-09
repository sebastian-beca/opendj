# Tasks

## 1. rekordbox XML export

- [x] 1.1 Writer beside the reader: tracks (tags, rating, colour, dates, play count, key in rekordbox notation), `TEMPO` per grid marker, `POSITION_MARK` for hot cues (number, colour), memory cues and loops; list tree with folders; URL-encoded `file://localhost/` locations
- [x] 1.2 Round-trip test through `rekordbox/xml.rs` on a built collection with variable grids, loops and nested folders
- [ ] 1.3 Timing check against rekordbox 7 on a test Mac with MP3 (CBR and VBR, with and without LAME header), AAC and FLAC; add an MP3 offset correction if rekordbox reads positions from a different start
- [x] 1.4a app: File › Export › rekordbox XML… (the list picked, or the whole collection), Export as rekordbox XML… on the Collection node, lists and folders; written on the save dialog's thread
- [ ] 1.4b Settings › Collection › General "Keep rekordbox.xml up to date" with path, debounced background writer
- [x] 1.5 Update the user guide (moving your library to and from other apps, with Lexicon / Rekord Buddy notes)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
