## MODIFIED Requirements

### Requirement: Settings window
Settings SHALL open with ⌘, (Ctrl+, on Linux) or from the macOS app menu, with pages Audio, Controllers, Collection,
Analysis, Decks, Mixer, Sources, Plugins, Broadcast, Keyboard, Help and one per add-on with settings. Settings › Decks
SHALL be split into Loading, Playing, and Tempo and key. Changes SHALL apply at once. Esc SHALL close it.

#### Scenario: Close with Esc
- **WHEN** Settings is open and Esc is pressed
- **THEN** it closes

## ADDED Requirements

### Requirement: Deck choices on the control
A Settings choice that changes how a deck control behaves SHALL also be on that control's right-click menu.

#### Scenario: Hot cue mode from a pad
- **WHEN** the DJ right-clicks a hot cue pad
- **THEN** the menu offers Jump and keep playing or Stutter, the same choice as in Settings › Decks › Playing
