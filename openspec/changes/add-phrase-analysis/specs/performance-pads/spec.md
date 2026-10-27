## ADDED Requirements

### Requirement: Automatic cues
Settings › Analysis "Auto cues" SHALL offer Off, Empty pads only (default) and Replace auto cues. When on, an analysed track with phrases SHALL get a CUE point at its first downbeat with sound and hot cues at its phrase changes (Intro, first Build, first Drop, Breakdown and Outro first, then the earliest of the rest), each named after its phrase. Cues the DJ set or an import brought SHALL never be changed.

#### Scenario: Empty pads filled
- **WHEN** a track with hot cues on pads 1 and 2 from the DJ is analysed with Empty pads only
- **THEN** pads 1 and 2 are unchanged and pads 3 onwards get phrase cues

#### Scenario: Replace auto cues
- **WHEN** the DJ chooses Replace auto cues and analyses a track again
- **THEN** only cues placed automatically are moved
