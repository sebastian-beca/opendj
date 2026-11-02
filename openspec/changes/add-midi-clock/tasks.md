# Tasks

## 1. Clock out

- [ ] 1.1 engine: beat clock snapshot (heard-at host time, beat, tempo, playing) published per buffer through atomics after `sync`; shared reader for MIDI clock, OSC and the Groove Box
- [ ] 1.2 Clock sender thread: schedule pulses ahead from the snapshot; back ends Core MIDI with timestamps (macOS), ALSA sequencer queue (Linux), high-resolution timer + spin (Windows); virtual "openDJ Clock" port on macOS and Linux
- [ ] 1.3 Start / Stop / Song Position on the downbeat, RESTART ON BAR, keep-running setting, per-output offset
- [ ] 1.4 Jitter test harness (loopback port, measure pulse spacing), target < 0.5 ms spread idle
- [ ] 1.5 app: CLOCK OUT and RESTART ON BAR in the MASTER panel, Settings › Controllers › MIDI Clock (outputs, offsets, keep running); `[MidiClock]` controls; regenerate supported-controls.txt
- [ ] 1.6 Update the user guide (Sync page: MIDI clock out)

## 2. Clock in and Link follow

- [ ] 2.1 controllers: clock input port reader (stop dropping `0xF8`-`0xFC` there), Song Position, smoothing filter with tests
- [ ] 2.2 engine: `[Master],sync_leader_source` Deck / Link / Clock; SYNC follows Link's or the clock's tempo and phase through the existing lock-free path; Link stops leading while it is followed
- [ ] 2.3 app: leader choice in the MASTER panel; regenerate supported-controls.txt
- [ ] 2.4 Update the user guide (Sync page: following Link or a drum machine)

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
