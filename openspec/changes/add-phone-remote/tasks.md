# Tasks

## 1. Automix from your phone

- [x] 1.1 `crates/remote`: HTTP page and `/ws` WebSocket on plain threads, Host and Origin checks, message size and rate limits, tests with a real socket
- [x] 1.2 Pairing: one-time QR code (2 minutes), 6-digit PIN, lockout after 5 wrong tries, device tokens kept as hashes, revoke
- [x] 1.3 Phone page: decks, Automix status and controls, the Automix list (drag, Play next, Move, Remove), master and booth levels, Fade out
- [x] 1.4 App: REMOTE button and panel with QR code, PIN, addresses and paired phones; requests carried out on the UI thread
- [x] 1.5 Update the user guide (new Phone remote page)

## 2. Search and add

- [ ] 2.1 Search the collection and browse crates, playlists and Play Ready from the phone; add to Automix at the end or next
- [ ] 2.2 Advertise the remote over Bonjour (mDNS)

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
