## Purpose

Letting an AI assistant search and organise the library and, when allowed, drive the decks, through a local Model Context Protocol server.

## ADDED Requirements

### Requirement: Off by default, local only
The MCP server SHALL be off until turned on in Settings, and SHALL only accept connections from the same computer.

#### Scenario: Remote refused
- **WHEN** a connection comes from another machine
- **THEN** it is refused

### Requirement: Library tools
The server SHALL offer tools to search tracks, read and edit tags and ratings, and create and fill lists.

#### Scenario: Warm-up list
- **WHEN** an assistant asks for 118-122 BPM house tracks as a new list
- **THEN** the list appears in the tree

### Requirement: Deck tools
The server SHALL offer tools to load, play, cue, sync, set loops and effects, record, and read deck state, through the same controls as controllers.

#### Scenario: Load and play
- **WHEN** an assistant loads a track on deck 2 and plays it
- **THEN** deck 2 plays it

### Requirement: Library-only mode
In library-only mode, deck tools SHALL be refused, so the decks can't be touched during a live set.

#### Scenario: Refused
- **WHEN** library-only is on and a play tool is called
- **THEN** the call is refused and the deck is untouched
