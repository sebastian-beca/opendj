## ADDED Requirements

### Requirement: First-run guided tour
On first start openDJ SHALL run a guided tour through audio setup, controller setup (only when a controller is connected), the music folder and library imports, and simple playback and mixing; every step SHALL be skippable, and the tour SHALL not show again once finished or closed.

#### Scenario: Skip
- **WHEN** every step is skipped
- **THEN** openDJ opens with defaults and the tour doesn't return

#### Scenario: No controller
- **WHEN** no controller is connected on first start
- **THEN** the controller step is left out

#### Scenario: Run it again
- **WHEN** the DJ chooses Help › Guided Tour
- **THEN** the tour starts from the first step
