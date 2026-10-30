## ADDED Requirements

### Requirement: Tutorials
Help › Tutorials SHALL list hands-on tutorials for Looping, Using FX, Filtering tracks, Automix and the DDJ-FLX4; each SHALL highlight the real control at every step, move on only once the DJ does what the step asks (on screen or on a controller), offer Skip step and Close at every step, link to its user guide page, and be marked done once finished.

#### Scenario: Learn looping
- **WHEN** the DJ opens Help › Tutorials › Looping and follows every step
- **THEN** they have set, halved, doubled and saved a loop on a deck, and Looping is marked done

#### Scenario: Demo tracks
- **WHEN** a tutorial that needs music starts
- **THEN** it loads the demo tracks that ship with openDJ, unless the DJ picks Use my tracks

#### Scenario: Close part way
- **WHEN** the DJ closes a tutorial before the last step
- **THEN** the decks and layout go back to how they were, and the tutorial is not marked done

#### Scenario: Controller tutorial without the controller
- **WHEN** no DDJ-FLX4 is connected
- **THEN** the DDJ-FLX4 tutorial is listed but greyed, with "Connect a DDJ-FLX4 to start"
