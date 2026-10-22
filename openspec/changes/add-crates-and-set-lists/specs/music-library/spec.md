## ADDED Requirements

### Requirement: Crates and sets
A list folder SHALL be a crate or a set, shown in different colours, and changeable from its menu.

#### Scenario: Mark as set
- **WHEN** a folder is marked as a set
- **THEN** it takes the set colour in the tree

### Requirement: New set template
New set… SHALL create a set folder named by the DJ with lists Staging, Set and Live set, and optionally Openers, Closers and Transitions.

#### Scenario: New set
- **WHEN** New set… is used with the name "Club X 2026-11"
- **THEN** a set folder with Staging, Set and Live set appears

### Requirement: Add to Set List from History
Selecting history rows and choosing Add to Set List… SHALL ask which set and append the tracks, in played order, to its Live set.

#### Scenario: Live set filled
- **WHEN** a night's history is added to a set
- **THEN** its Live set lists those tracks in the order played
