## MODIFIED Requirements

### Requirement: Master limiter
The master SHALL pass through a look-ahead, stereo-linked limiter that catches true peaks (between samples, measured 4× oversampled) and leaves quieter audio untouched. Its ceiling SHALL be -1 dBTP while recording or on air (-2 dBTP when chosen in Settings › Audio › Loudness) and -0.5 dBFS otherwise.

#### Scenario: No clipping
- **WHEN** two loud decks play with every fader up
- **THEN** the master output never exceeds -0.5 dBFS

#### Scenario: No true-peak overs on air
- **WHEN** ON AIR is on and a loud track with many inter-sample peaks plays with the master fader up
- **THEN** the true peak of the master output never exceeds -1 dBTP

## ADDED Requirements

### Requirement: Loudness meter
The MASTER panel SHALL show the loudness of the master output as it plays: momentary (400 ms) and short-term (3 s) loudness, integrated loudness since the last reset and the loudness range in LUFS and LU, and the highest true peak in dBTP, all measured as ITU-R BS.1770 and EBU Tech 3341/3342 define them. While recording or on air, the top bar SHALL show the short-term loudness next to REC or ON AIR.

#### Scenario: Reference tone
- **WHEN** a 1 kHz sine at -23 dBFS plays on both channels through the master at unity
- **THEN** the meter shows -23.0 LUFS momentary, short-term and integrated, within 0.1 LU

### Requirement: Loudness reset
Integrated loudness, loudness range and the highest true peak SHALL start again when REC or ON AIR starts and when Reset is clicked.

#### Scenario: Reset on air
- **WHEN** ON AIR is switched on
- **THEN** integrated loudness and the highest true peak start again from that moment

### Requirement: Loudness controls
Controllers and scripts SHALL read the meter from `[Master],loudness_momentary`, `loudness_short_term`, `loudness_integrated`, `loudness_range` and `true_peak`, and reset it with `[Master],loudness_reset`.

#### Scenario: Reset from a controller
- **WHEN** a mapping sets `[Master],loudness_reset`
- **THEN** integrated loudness and the highest true peak start again

### Requirement: Loudness target
Settings › Audio › Loudness SHALL offer a loudness target: Internet radio (-16 LUFS, the default), Podcast / talk (-18), Livestream (-14) or Broadcast EBU R128 (-23). The loudness meter SHALL be green within 1 LU of the target, amber 1 to 3 LU away and red further away. The target SHALL not change any track or deck level.

#### Scenario: Too loud for radio
- **WHEN** the target is Internet radio and the short-term loudness is -12 LUFS
- **THEN** the short-term reading is red

### Requirement: Master meter scale
The master's level meters SHALL read sample peak on a dB scale from -40 to 0 dBFS with marks at -40, -20, -10, -6, -3 and 0, rise at once and fall at about 20 dB per second. The clip light SHALL show when the mix before the limiter goes over 0 dBFS.

#### Scenario: Readable fall
- **WHEN** a loud hit is followed by silence
- **THEN** the master meter falls from 0 to -20 dBFS in about one second
