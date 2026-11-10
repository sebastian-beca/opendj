## ADDED Requirements

### Requirement: Lyrics on the waveforms
When a track with timed lyrics is on a deck, its big waveform SHALL show the lyrics along its bottom edge, scrolling with the audio: each line placed in time from when it is sung to when it ends, and each word placed at the time it is sung, with the word being sung lit and filled in the accent color as it is sung and the words already sung in the accent color. The lane SHALL move in whole pixels so the words don't shimmer. Settings › Decks › Lyrics on waveforms SHALL turn this off; it is on at first.

#### Scenario: Line under the playhead
- **WHEN** deck A plays a track whose line "Hold the line" is sung from 0:39.4 to 0:43.4 and the deck is at 0:40.5
- **THEN** that line's strip crosses the play line on deck A's waveform, lit, with part of its words in lilac

#### Scenario: Turned off
- **WHEN** the DJ turns off Lyrics on waveforms
- **THEN** no deck's waveform shows lyrics
