# deck-transport Specification

## Purpose
How a DJ loads a track on one of the four decks and moves around in it: play and CUE, hot cues, saved loops, loops, beat jumps, slip, reverse and the jog wheel.

## Requirements

### Requirement: Four decks
openDJ SHALL have four decks, `[Channel1]` to `[Channel4]`, each able to hold and play one track. Decks 1 and 2 SHALL be shown in every layout; decks 3 and 4 SHALL be shown in the 4-Deck layout.

#### Scenario: Four deck groups exist
- **WHEN** `opendj --list-controls` is run
- **THEN** it lists controls for `[Channel1]` through `[Channel4]` and `[App],num_decks` reads 4

### Requirement: Loading a track
A track SHALL be loaded by dragging it onto a deck, from a track's menu, or with `LoadSelectedTrack`. The deck SHALL take the track as soon as its file opens and SHALL be playable while the rest decodes; the waveform fills in as the audio is analysed.

#### Scenario: Track plays before decoding finishes
- **WHEN** a long track is dropped on a stopped deck and PLAY is pressed at once
- **THEN** playback starts without waiting for the whole file to decode

#### Scenario: Analysed track loads instantly
- **WHEN** a track that was analysed before is loaded
- **THEN** its waveform, tempo and beat grid show at once from the cache

### Requirement: Protect playing decks
While "Protect playing decks" is on (the default), loading onto a playing deck SHALL be refused and the DJ told why. The setting maps to `[Controls],AllowTrackLoadToPlayingDeck` (inverted).

#### Scenario: Load refused while playing
- **WHEN** protection is on and a track is dropped on a playing deck
- **THEN** the deck keeps playing its track and an alert says the deck is playing

### Requirement: Play and CUE
PLAY SHALL toggle playback. CUE SHALL follow the "CUE button" setting: "Hold to preview" (default, Mixxx `cue_mode` 0) plays while held from the cue point and returns on release, and sets the cue when pressed while paused; "Set and return" (`cue_mode` 3) only sets and returns.

#### Scenario: Hold to preview
- **WHEN** a paused deck's CUE is held and then released
- **THEN** the deck plays from the cue point while held and jumps back to it, paused, on release

#### Scenario: CUE while playing returns
- **WHEN** CUE is pressed on a playing deck
- **THEN** the deck stops and returns to the cue point

### Requirement: Paused indicators blink
The PLAY and CUE indicators SHALL blink in step on a paused deck with a track loaded, on screen and on controller LEDs.

#### Scenario: Blink while paused
- **WHEN** a track is loaded and paused
- **THEN** PLAY and CUE blink together until the deck plays

### Requirement: Hot cues
Each deck SHALL have 8 hot cues with a position, colour and optional name, saved with the track in the library. Pressing an empty hot cue sets it at the playhead (snapped to the beat grid); pressing a set one jumps to it; Shift with the pad, or `hotcue_N_clear`, deletes it.

#### Scenario: Set and recall a hot cue
- **WHEN** an empty hot cue pad is pressed while playing and later pressed again
- **THEN** the first press stores the nearest beat, and the second jumps playback there

#### Scenario: Hot cues survive a restart
- **WHEN** hot cues are set, openDJ quits and the track is loaded again
- **THEN** the same hot cues are there

### Requirement: Saved loops
A hot cue MAY be a saved loop (Mixxx hot cue type 4 with an end position). Pressing it SHALL jump to its start and turn the loop on.

#### Scenario: Recall a saved loop
- **WHEN** a pad holding a saved loop is pressed
- **THEN** the deck jumps to the loop start and loops between its start and end

### Requirement: Memory cues kept
Cue points beyond the 8 hot cues brought in from other apps (memory cues) SHALL be kept in the library with the track and SHALL NOT be lost on a re-import.

#### Scenario: VirtualDJ memory cues kept
- **WHEN** a VirtualDJ track with more than 8 cues is imported
- **THEN** all its cues are stored, the first 8 as hot cues

### Requirement: Beat loops
The loop box SHALL show the loop length; pressing it starts a beat loop of that length at the playhead (`beatloop_activate`), pressing again ends it, and the arrows halve or double it (`loop_halve`, `loop_double`). Lengths SHALL run from 1/2 to 64 beats. IN and OUT SHALL set a loop of any length.

#### Scenario: Four-beat loop
- **WHEN** the loop length is 4 and the loop box is pressed
- **THEN** the deck loops 4 beats from the current beat and the length turns green

#### Scenario: Manual loop
- **WHEN** IN is pressed, then OUT a few beats later
- **THEN** the deck loops between the two points and OUT lights lilac

### Requirement: Loop rolls
Loop rolls (`beatlooproll_N_activate`) SHALL loop while held and, on release, continue from where the track would have been without the roll.

#### Scenario: Roll returns in time
- **WHEN** a 1/2-beat roll is held for two bars and released
- **THEN** playback continues two bars later than where the roll started

### Requirement: Beat jumps
Beat jump SHALL move the playhead a whole number of beats forward or back (`beatjump_N_forward`, `beatjump_N_backward`, N = 1 to 32), keeping its place in the beat.

#### Scenario: Jump 16 beats
- **WHEN** beat jump 16 forward is pressed
- **THEN** the playhead moves exactly 16 beats later on the grid

### Requirement: Slip mode
With `slip_enabled` on, loops, reverse and scratching SHALL keep a silent playhead moving, and playback SHALL resume from it when the action ends.

#### Scenario: Slip after a loop
- **WHEN** slip is on and a loop is exited after 8 beats
- **THEN** playback continues 8 beats past where the loop started

### Requirement: Reverse and censor
`reverse` SHALL play the deck backwards while on. `reverseroll` (censor) SHALL play backwards while held and resume as if it had played forwards.

#### Scenario: Censor
- **WHEN** censor is held for one bar
- **THEN** the deck plays backwards and on release continues one bar further on

### Requirement: Jog wheel and scratching
Jog input SHALL nudge a playing deck's speed (`jog`) and scratch when scratching is enabled (`scratch2_enable`, `scratch2`). A deck moved by hand faster than 8x SHALL be muted while it moves. Scrolling over an on-screen jog SHALL nudge the track by 1 ms per scrolled pixel.

#### Scenario: Fast search is silent
- **WHEN** a controller jog spins a deck faster than 8x normal speed
- **THEN** the deck is muted until it slows down

### Requirement: Scrubbing on the waveform
Holding the scrolling waveform SHALL stop the track under the pointer and dragging SHALL scrub through it; releasing SHALL resume the previous state. Clicking the overview waveform SHALL seek there.

#### Scenario: Drag to scrub
- **WHEN** the DJ holds and drags a playing deck's scrolling waveform
- **THEN** audio scrubs with the drag and playback resumes on release

### Requirement: Eject
`eject` SHALL unload the deck's track.

#### Scenario: Eject a stopped deck
- **WHEN** eject is pressed on a stopped deck with a track
- **THEN** the deck is empty
