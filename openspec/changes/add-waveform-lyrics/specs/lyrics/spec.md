## ADDED Requirements

### Requirement: Lyrics on the waveforms
When a track with timed lyrics is on a deck, its big waveform SHALL show the lyrics along its bottom edge, each line placed in time from when it is sung to when it ends and scrolling with the audio, with the line being sung lit and filled in the accent color as it is sung. Settings › Decks › Lyrics on waveforms SHALL turn this off; it is on at first.

#### Scenario: Line under the playhead
- **WHEN** deck A plays a track whose line "Hold the line" is sung from 0:39.4 to 0:43.4 and the deck is at 0:40.5
- **THEN** that line's strip crosses the play line on deck A's waveform, lit, with part of its words in lilac

#### Scenario: Turned off
- **WHEN** the DJ turns off Lyrics on waveforms
- **THEN** no deck's waveform shows lyrics
