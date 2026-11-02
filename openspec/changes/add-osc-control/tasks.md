# Tasks

## 1. OSC

- [ ] 1.1 controllers: `osc.rs` UDP thread with `rosc`; address ↔ control table built from the registry; set and query; local-only and send-only modes; tests with a real socket
- [ ] 1.2 Feedback: per-client subscriptions with pattern matching, throttled to 30 Hz per control, send targets list
- [ ] 1.3 Events: `/beat` and `/phrase` from the beat clock snapshot (add-midi-clock 1.1) and phrase changes; `/deck/N/track` on load; `/deck/N/cue` on hot cue
- [ ] 1.4 Bonjour advert `_osc._udp` (shared with the phone remote's mDNS step)
- [ ] 1.5 app: Settings › Controllers › OSC (switch, port, targets, local only, send only, status line)
- [ ] 1.6 User guide: new OSC page (address rules, events, generated address list, starter TouchOSC layout for two decks)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
