# Tasks

## 1. Automix from your phone

- [x] 1.1 `crates/remote`: HTTP page and `/ws` WebSocket on plain threads, Host and Origin checks, message size and rate limits, tests with a real socket
- [x] 1.2 Pairing: one-time QR code (2 minutes), 6-digit PIN, lockout after 5 wrong tries, device tokens kept as hashes, revoke
- [x] 1.3 Phone page: decks, Automix status and controls, the Automix list (drag, Play next, Move, Remove), master and booth levels, Fade out
- [x] 1.4 App: REMOTE button and panel with QR code, PIN, addresses and paired phones; requests carried out on the UI thread
- [x] 1.5 Update the user guide (new Phone remote page)

## 2. Search and add

- [x] 2.1 Search the collection from the phone with the browser's search syntax; add to Automix at the end or next
- [x] 2.2 Update the user guide page
- [ ] 2.3 Browse crates, playlists and Play Ready from the phone
- [ ] 2.4 Advertise the remote over Bonjour (mDNS)

## 3. Automix manager

- [x] 3.1 Now-playing screen: large cover, one track (the one you hear), Order, play/pause, Mix now, Mix settings, next track with time to its mix
- [x] 3.2 Queue: covers on every track, Now playing and Next up, Mix/Order/Levels/Timer bar; the waiting track stays movable until 15 s before its mix
- [x] 3.3 Edit: pick several tracks for Play next, Move to the end, Remove
- [x] 3.4 Search full screen with Done; covers served to paired phones only
- [x] 3.5 Sleep timer; lilac theme
- [x] 3.6 Update the user guide page

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
