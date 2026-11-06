## ADDED Requirements

### Requirement: Phrases length
Automix SHALL offer a Phrases length after 32 bars. With it, the mix SHALL start on a phrase change of the outgoing track (an imported fade-out point, else the start of its outro at least 8 bars before its sound end, else the last phrase start 8 to 32 bars before its sound end) and bring the incoming track in from its in point (an imported start point, else bar 1 of the phrase its sound starts in), beatmatched. The mix SHALL last the bars from the out point to the sound end, capped by the incoming intro, rounded down to a multiple of 4 and kept within 8 to 32 bars.

#### Scenario: Intro over outro
- **WHEN** the length is Phrases, the outgoing track has a 16-bar outro and the incoming track a 16-bar intro
- **THEN** the mix starts on the outro's first bar, lasts 16 bars, and the incoming track's first phrase change lands as the outgoing track ends

#### Scenario: Short intro
- **WHEN** the outgoing outro is 32 bars and the incoming intro 8 bars
- **THEN** the mix lasts 8 bars and starts 8 bars before the outgoing sound end, on a phrase boundary of the outgoing track when one is there

#### Scenario: Imported automix point wins
- **WHEN** the outgoing track has a VirtualDJ fadeStart point and detected phrases
- **THEN** the mix starts at the fadeStart point

### Requirement: Phrases fallback
When the outgoing track has no usable phrase or automix point, the Phrases length SHALL mix 16 bars; when the tempos can't be matched within the tempo fader range, it SHALL mix 15 s. The status line SHALL say which.

#### Scenario: Tempo out of reach
- **WHEN** the length is Phrases and the incoming track is 30 % faster than the outgoing one can be matched
- **THEN** Automix mixes 15 s before the outgoing sound end and the status line says the tempos don't match

### Requirement: Bass swap on a phrase
With the Phrases length and the Bass swap style, the bass SHALL change over at the phrase change nearest the middle of the mix.

#### Scenario: Swap on the bar
- **WHEN** a 16-bar Phrases mix with Bass swap has an outgoing phrase change at its 8th bar
- **THEN** the bass changes over on that bar
