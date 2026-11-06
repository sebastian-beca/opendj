## ADDED Requirements

### Requirement: Show modified and reset
Settings SHALL offer Show modified, which lists only the settings that differ from their defaults, on every page and in
a search. Each changed row SHALL have a reset button that puts it back to its default at once, and Settings SHALL
offer Reset all, after a confirm. Every setting's default SHALL come from one table.

#### Scenario: See what was changed
- **WHEN** the DJ has turned the crossfader off and set the tempo range to ±16%, and turns on Show modified
- **THEN** exactly those two rows are listed, each with a reset button

#### Scenario: Reset one
- **WHEN** the DJ presses reset on the tempo range row
- **THEN** the tempo range goes back to ±8% at once and the row leaves the modified list

### Requirement: Search by meaning
Settings search SHALL match each row's name, help and a few search words, so a DJ finds a setting by the word they
know from another app.

#### Scenario: Other apps' words
- **WHEN** the DJ searches "pitch", "PFL" or "hamster"
- **THEN** the tempo, Split cue and Reverse crossfader rows are found

### Requirement: Short rows with help on demand
A Settings row SHALL show its name and control only. Help that the name doesn't already give SHALL be in an ⓘ popover
beside the name.

#### Scenario: Help on demand
- **WHEN** the DJ clicks the ⓘ beside "Vinyl mode"
- **THEN** a popover says what touching the jog's top does with it on and off
