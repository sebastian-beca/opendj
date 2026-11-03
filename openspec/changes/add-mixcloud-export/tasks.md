# Tasks

## 1. Mixcloud-ready MP3

- [x] 1.1 Recordings join Mixed Sets as soon as they stop
- [x] 1.2 Recorder writes `<name>.tracklist.txt` beside the `.cue`
- [x] 1.3 Read the recorder's `.cue` (several parts, tracks in the same second dropped) into one tracklist
- [x] 1.4 Export for Mixcloud: 320 kbps MP3, parts joined, streamed, ID3v2.4 tag with title, artist, tracklist comment and chapters
- [x] 1.5 Mixcloud upload form fields and limits checked before sending

## 2. Upload to Mixcloud

- [ ] 2.1 Decide where the client secret lives (token-exchange function on opendj.app proposed)
- [ ] 2.2 Sign in with Mixcloud in the browser; token kept in the keychain; sign out
- [ ] 2.3 Upload dialog: name, description (tracklist filled in), tags, unlisted, cover; progress; link to the show when done
- [ ] 2.4 Update the user guide (recording, Mixed Sets, Mixcloud)

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
