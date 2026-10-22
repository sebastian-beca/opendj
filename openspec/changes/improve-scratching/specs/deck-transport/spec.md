## ADDED Requirements

### Requirement: Jog latency
Jog moves SHALL be heard within one audio buffer plus 5 ms.

#### Scenario: Latency
- **WHEN** the jog moves with a 128-frame buffer
- **THEN** the change is audible within about 8 ms

### Requirement: Crossfader cut-in
Settings › Mixer SHALL offer a crossfader cut-in distance when the curve is Scratch.

#### Scenario: Short cut-in
- **WHEN** cut-in is set to the shortest
- **THEN** the incoming side opens within the first 1 % of travel
