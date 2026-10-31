## MODIFIED Requirements

### Requirement: Settings window
Settings SHALL open with ⌘, (Ctrl+, on Linux) or from the macOS app menu, with pages Audio, Controllers, Library, Analysis, Decks, Mixer, Keyboard, Automix, Help, Advanced and one per add-on with settings. Settings › Decks SHALL be split into Loading, Playing, and Tempo and key. Changes SHALL apply at once. Esc SHALL close it.

#### Scenario: Close with Esc
- **WHEN** Settings is open and Esc is pressed
- **THEN** it closes

## ADDED Requirements

### Requirement: Advanced settings
Settings › Advanced SHALL list every setting openDJ has, including those on the other pages, as one searchable list. Each row SHALL show a plain-English name, its stable key in small grey text, and its value; the selected row's description SHALL show below the list.

#### Scenario: Find a tweak
- **WHEN** the DJ opens Advanced and types "tails"
- **THEN** the effect tails row is shown with its description

### Requirement: Show modified and reset
Every Settings page and Advanced SHALL offer Show modified, which lists only settings that differ from their defaults; each changed row SHALL have a reset button that puts it back to its default, and Advanced SHALL offer Reset all, after a confirm.

#### Scenario: See what was changed
- **WHEN** the DJ has turned the crossfader off and set the tempo range to ±16%, and picks Show modified
- **THEN** exactly those two rows are listed, each with a reset button

#### Scenario: Reset one
- **WHEN** the DJ presses reset on the tempo range row
- **THEN** the tempo range goes back to ±8% at once and the row leaves the modified list

### Requirement: Search by meaning
Settings search SHALL match each row's name, description and a few search words, so a DJ finds a setting by the word they know.

#### Scenario: Other apps' words
- **WHEN** the DJ searches "pitch", "PFL" or "hamster"
- **THEN** the tempo, headphones cue and reverse crossfader rows are found

### Requirement: Layout is not a setting
Window and panel sizes, list columns and sort, the last folder and waveform zoom SHALL be remembered without appearing in Settings; View › Reset layout SHALL put them back to the defaults. Help SHALL offer Show all hints again.

#### Scenario: Reset layout
- **WHEN** the DJ has resized the browser and hidden columns and picks View › Reset layout
- **THEN** sizes and columns return to their defaults, and no setting in Settings changes

### Requirement: Deck choices on the control
A Settings choice that changes how a deck control behaves SHALL also be on that control's right-click menu.

#### Scenario: Hot cue mode from a pad
- **WHEN** the DJ right-clicks a hot cue pad
- **THEN** the menu offers Jump and keep playing or Stutter, the same choice as in Settings › Decks › Playing
