## ADDED Requirements

### Requirement: Pre or post fader
Each deck effect unit SHALL be pre- or post-fader; post-fader echo tails SHALL continue after the fader closes.

#### Scenario: Post-fader tail
- **WHEN** a post-fader echo plays and the fader closes
- **THEN** the repeats keep sounding

### Requirement: Headphone preview
An effect unit SHALL be switchable to headphones only, heard in the cue mix and not the master until switched back.

#### Scenario: Preview FX
- **WHEN** a reverb is on in headphone mode
- **THEN** it is heard in the headphones only
