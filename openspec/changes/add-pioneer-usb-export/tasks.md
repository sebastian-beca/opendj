# Tasks

## 1. Writer

- [x] 1.1 Stick layout: FAT-safe `Contents/Artist/Album/` paths, analysis folders, artwork paths
- [x] 1.2 Analysis files with rbox: path, seek data, beat grid, cues, loops, all waveforms; checked against rekordbox bytes
- [x] 1.3 Device Library and OneLibrary with rbox, same ids in both; work around rbox 0.3 schema bugs (column name, missing play count, foreign keys)
- [x] 1.4 Album art thumbnails
- [x] 1.5 End-to-end test reading an export back with rbox; checked with rekordcrate
- [x] 1.6 Tests read exports back with a second reader that shares no code with rbox (ported from EngineX OS, the reader OpenProLink's CDJ-3000 simulator uses): many tracks over several pages, non-English names, folders, art, loops and memory cues, both formats agreeing

## 2. App

- [x] 2.1 Export to USB… in the tree menu, the dialog, progress, stop and result
- [x] 2.2 Update the matching user guide page in website/book/src/guide

## 3. On hardware

- [ ] 3.1 Try a stick on a CDJ-3000 or XDJ (Device Library) and on a CDJ-3000X, XDJ-AZ or OPUS-QUAD (OneLibrary)
- [ ] 3.2 Drop "experimental" once both play with grids, cues and waveforms

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
