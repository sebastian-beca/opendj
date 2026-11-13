## Purpose

Letting openDJ work next to Pioneer CDJs, XDJs and DJM mixers on a Pro DJ Link network: see what they play, follow or lead their tempo, and be visible to show tools that follow them.

## ADDED Requirements

### Requirement: Off until switched on
Pro DJ Link SHALL be off at first and marked Beta. With it off, openDJ SHALL open no Pro DJ Link socket.

#### Scenario: First launch
- **WHEN** openDJ starts for the first time on a network with CDJs
- **THEN** openDJ sends nothing on ports 50000 to 50002 and the players do not list it

### Requirement: Watch
With Pro DJ Link on, openDJ SHALL list every player and mixer on the chosen interface with its model, number, play state, tempo, master and on-air flags and loaded track, without sending anything unless it has joined (Follow, Lead or show tools). Where a player's dbserver answers, the list SHALL show the track's title, artist and artwork; where it refuses, the track id.

#### Scenario: Two CDJs
- **WHEN** two CDJs are on the network and one plays a track
- **THEN** both appear in Settings › Controllers › Pro DJ Link and the playing one shows its track and tempo

#### Scenario: Beats for visuals
- **WHEN** OSC is on and a CDJ is tempo master and playing
- **THEN** OSC `/beat` messages follow that CDJ's beats

### Requirement: Player number
When openDJ joins the network it SHALL take the lowest free player number from 5 upward, after listening for 5 seconds, and SHALL never take 1 to 4.

#### Scenario: Four CDJs
- **WHEN** players 1 to 4 are on the network and openDJ joins
- **THEN** openDJ appears as player 5

### Requirement: Lead
With Lead allowed, openDJ's master deck SHALL be able to become the network's tempo master, only through the hand-off the players use: a request to the current master, its acceptance, and openDJ naming itself master only once handed the role. openDJ SHALL hand the role on the same way when asked, and SHALL cancel a request not granted within 3 seconds. There SHALL never be a moment when openDJ and another player both claim to be master.

#### Scenario: CDJs follow openDJ
- **WHEN** Lead is allowed, openDJ takes tempo master and a CDJ has SYNC on
- **THEN** the CDJ plays at openDJ's master deck tempo, in phase

#### Scenario: Hand back
- **WHEN** openDJ is master and a CDJ asks for the role
- **THEN** openDJ names the CDJ as its successor, the CDJ takes the role, and openDJ stops claiming it

#### Scenario: No answer
- **WHEN** openDJ asks for the role and the master does not hand it over within 3 seconds
- **THEN** the request is cancelled and openDJ does not claim master

### Requirement: Visible to show tools
While joined, openDJ SHALL announce each playing deck's state in its status and beats, and SHALL answer metadata queries for its loaded tracks with title, artist, album, BPM, key, duration and artwork. Requests it does not support SHALL get an empty answer, not an error.

#### Scenario: Lighting follows openDJ
- **WHEN** a show tool that follows CDJs is on the network and openDJ plays a track
- **THEN** the tool shows openDJ as a player with that track's title and artwork and receives its beats

### Requirement: DJM mixer
openDJ SHALL read on-air flags and fader start from a DJM on the network. In external mixer mode, a fader start for the channel a deck is routed to SHALL play that deck.

#### Scenario: Fader start
- **WHEN** a deck is routed to DJM channel 2 and the DJ opens channel 2 with fader start on
- **THEN** the deck starts playing
