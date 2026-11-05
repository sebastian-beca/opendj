# Tasks

## 1. Playlist files

- [x] 1.1 library: M3U8 writer (extended M3U, absolute or relative paths, comment lines for placeholders), folder tree export, tests. Paths are relative when the track is in the file's folder or below, absolute otherwise, with no setting.
- [x] 1.2 library: M3U/M3U8, PLS and XSPF readers, matching by path, relative path, then file name; files found on disk join the collection as dropped files do. Shipped with its own M3U parser (the VirtualDJ import keeps its own) and without the file size check.
- [x] 1.3 app: Export as M3U8… on lists, crates, folders, History sessions, Albums and Mixed Sets; Import Playlist File… in the Playlists + menu; the result shows in the list's header. Not yet: export from Automix, dropping a file on the Playlists node, a report dialog.
- [x] 1.4 Update the user guide (Playlists page)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
