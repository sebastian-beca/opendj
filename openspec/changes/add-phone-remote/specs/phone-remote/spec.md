## Purpose

Letting a DJ control Automix and the main levels from a phone on the same network, by opening a page openDJ serves, without installing an app.

## ADDED Requirements

### Requirement: Off until switched on
The remote SHALL not listen on the network until the DJ switches it on under REMOTE in the top bar. It SHALL stay on across restarts only if it was on when openDJ quit.

#### Scenario: First launch
- **WHEN** openDJ starts for the first time
- **THEN** REMOTE is off and nothing listens on the network

### Requirement: Pairing
A phone SHALL pair by scanning the QR code under REMOTE, which holds a code that works once and for two minutes, or by typing the 6-digit PIN shown there. After five wrong codes or PINs in a row, pairing SHALL stop for a minute. A paired phone SHALL reconnect without pairing again until the DJ removes it.

#### Scenario: Scan to pair
- **WHEN** the DJ scans the QR code with a phone on the same Wi-Fi
- **THEN** the phone opens the remote, appears under Paired phones as connected, and REMOTE turns green

#### Scenario: Removed phone
- **WHEN** the DJ clicks Remove beside a phone
- **THEN** the phone is disconnected and asked to pair again

### Requirement: Other pages kept out
The remote SHALL refuse requests whose `Host` is not an address or `.local` name of the computer, and WebSocket connections opened by a page from another origin.

#### Scenario: A web page tries to connect
- **WHEN** a web page on another site tries to open the remote's WebSocket
- **THEN** the connection is refused

### Requirement: Automix on the phone
A paired phone SHALL show both decks (title, artist, tempo, key, time left, and the next mix shaded), Automix's state and status line, and the Automix list. It SHALL switch Automix on and off, Mix now, Skip, set the mix length and style, put the list in order (with Undo), move tracks (Play next, Move up/down, drag) and remove them, using the same controls as the Automix panel (`[AutoDJ],enabled`, `[AutoDJ],fade_now`, `[AutoDJ],skip_next`). Tracks Automix is playing or has waiting on a deck SHALL stay at the top.

#### Scenario: Reorder from the phone
- **WHEN** the DJ drags the fifth track on the phone to just below the waiting track
- **THEN** it moves there in the Automix panel too and plays next

### Requirement: Levels on the phone
A paired phone SHALL set the master level (`[Master],gain`) and, when a booth output is set up, the booth level (`[Master],booth_gain`), and fade the master level out over eight seconds and bring it back.

#### Scenario: Fade out
- **WHEN** the DJ taps Fade out
- **THEN** the master level slides to silence over eight seconds, and Bring the sound back restores it
