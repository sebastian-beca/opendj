## ADDED Requirements

### Requirement: Export lists as M3U8
Every playlist, crate, set, folder, History day, Albums and Mixed Sets entry and the Automix list SHALL offer **Export as M3U8…**. The file SHALL be UTF-8 extended M3U (`#EXTM3U`, `#EXTINF:<seconds>,<artist> - <title>`, one path per track) in list order. A folder SHALL export as a folder holding one `.m3u8` per list in the same tree. Paths SHALL be absolute unless the DJ chooses paths relative to the saved file. Streamed placeholders SHALL be written as comment lines naming the track and service.

#### Scenario: Set to a USB stick
- **WHEN** the DJ exports a set to a USB stick with relative paths and the tracks copied beside it
- **THEN** another DJ app opening the file on another computer finds every track

#### Scenario: Folder export
- **WHEN** the DJ exports the folder "Weddings" holding three playlists
- **THEN** a folder "Weddings" with three `.m3u8` files is written

### Requirement: Import playlist files
Dropping `.m3u`, `.m3u8`, `.pls` or `.xspf` files on the Playlists node or the browser, or choosing **Import Playlist File…** in the Playlists + menu, SHALL make one playlist per file, named after the file (or the XSPF title). Each entry SHALL be matched to a collection track by path, then by path relative to the playlist file, then by file name and size in the music folders. Files not in the collection SHALL be added to it in the Inbox stage. Entries that can't be found SHALL be listed in a report the DJ can copy.

#### Scenario: Playlist from a friend
- **WHEN** the DJ drops `friday.m3u8` from another computer whose paths don't exist here, and the same files are in the DJ's music folders
- **THEN** a playlist "friday" holds those tracks, matched by file name and size

#### Scenario: Missing files
- **WHEN** three entries of an imported file can't be found
- **THEN** the playlist holds the rest and the report lists the three
