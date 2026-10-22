## ADDED Requirements

### Requirement: Slicer
A SLICER page SHALL split the next 8 beats into 8 pads; pressing a pad SHALL play its slice in time, looping the section while slicing.

#### Scenario: Slice
- **WHEN** pad 5 is pressed on the slicer
- **THEN** the fifth beat of the section plays, in time

### Requirement: Freeze
A FREEZE page SHALL play short slices of the current beat from each pad while held.

#### Scenario: Stutter
- **WHEN** a freeze pad is held
- **THEN** the slice repeats until released
