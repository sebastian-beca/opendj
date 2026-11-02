# Tasks

## 1. Broadcast meter (v0.5, size S–M)

- [ ] 1.1 Running loudness meter off the audio thread, fed from the master tap: momentary, short-term, integrated, loudness range, true peak (4× oversampled)
- [ ] 1.2 Tests against the EBU Tech 3341 and 3342 test signals
- [ ] 1.3 True-peak detector in the master limiter; -1 dBTP ceiling while recording or on air, -2 dBTP option
- [ ] 1.4 Loudness meter in the MASTER panel with targets and colours; short-term reading in the top bar while recording
- [ ] 1.5 Master meter scale marks and 20 dB/s fall
- [ ] 1.6 `[Master],loudness_*` and `true_peak` controls; regenerate website/book/supported-controls.txt
- [ ] 1.7 User guide: new Broadcasting page (meter and targets)

## 2. Icecast radio (v0.5, size M)

- [ ] 2.1 `crates/broadcast`: per-output thread and bounded queue from the master tap; resampling to 48 kHz
- [ ] 2.2 Encoders: Opus in Ogg, MP3 (LAME), AAC through AudioToolbox and Media Foundation
- [ ] 2.3 Icecast 2 and Shoutcast 1 source client with TLS; Test button; reconnect rules and error messages
- [ ] 2.4 Now-playing from the audible track, template and fixed text; Opus metadata decision (design open question 1) tested with the main players
- [ ] 2.5 Settings › Broadcast stations, keychain storage for passwords
- [ ] 2.6 ON AIR pill and panel: time, lights, listener counts; `[Shoutcast],enabled` and `status`; regenerate supported-controls.txt
- [ ] 2.7 Test against a local Icecast 2.5 server in CI
- [ ] 2.8 User guide: Broadcasting page (stations, going on air)

## 3. Go live to platforms (v0.6, size L)

- [ ] 3.1 Test what Twitch, YouTube, Mixcloud Live and Kick accept (audio-only, Enhanced RTMP with Opus) and fix the presets
- [ ] 3.2 FLV muxer and RTMP client
- [ ] 3.3 H.264 encoding: VideoToolbox, Media Foundation, OpenH264 on Linux
- [ ] 3.4 Now-playing picture drawn with Slint's software renderer
- [ ] 3.5 Platform presets with stream key only; several destinations at once
- [ ] 3.6 User guide: going live on Twitch, YouTube and Mixcloud

## 4. WebRTC (v0.6, size M–L)

- [ ] 4.1 WHIP client with Opus audio
- [ ] 4.2 Test with Cloudflare Stream and any platform that accepts WHIP
- [ ] 4.3 User guide page

## 5. Radio show tools (v0.6, size M)

- [ ] 5.1 Show panel: timer, top-of-hour countdown, station ID and jingle bank
- [ ] 5.2 Mic ON AIR light
- [ ] 5.3 Now-playing feed on the remote's server (read-only, off until switched on)
- [ ] 5.4 Chapters in the show's recording from its tracklist
- [ ] 5.5 User guide page

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
