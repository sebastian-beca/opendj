## MODIFIED Requirements

### Requirement: Crossfader
The crossfader SHALL blend the left and right sides with the curve chosen in Settings › Mixer: Smooth (constant power, default), Linear, or Scratch (both sides full until the last stretch of travel set by Cut-in). "Reverse crossfader" SHALL swap its sides (`[Mixer Profile],xFaderReverse`). With the Scratch curve, Settings › Mixer SHALL offer Cut-in: 0.5 %, 1 %, 2 % (default) or 5 % of the travel (`[Mixer Profile],xFaderCutIn`, as a share of it), the distance from the end over which a side opens fully.

#### Scenario: Scratch curve
- **WHEN** the curve is Scratch and the crossfader is moved 10 % from the left end
- **THEN** both sides are at full level

#### Scenario: Short cut-in
- **WHEN** the curve is Scratch, cut-in is set to the shortest and the crossfader is moved 1 % from the left end
- **THEN** the incoming right side is at full level
