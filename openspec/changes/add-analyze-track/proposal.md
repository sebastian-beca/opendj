# Analyze Track from the browser

## Why

Tracks are only analysed when they load on a deck, when they enter Triage, or from Collection Health. The DJ has no way to analyse a chosen track or list ahead of a set, or to redo a track whose analysis came from an older version or another key detector. Seba asked (2026-10-28) for a right-click "Analyze Track" that finds everything at once: BPM, beat grid, key, loudness, mini waveform and phrases. He also asked for the Colour row to leave the track menu.

## What changes

- The track menu gets **Analyze Track** ("Analyze N Tracks" for a shift-click selection). Each track is analysed again from its audio in the background, one at a time, even when it was analysed before: tempo, grid, key, loudness, waveform (and so its mini waveform) and phrases.
- Analysing again keeps what the DJ or an import decided: a beat grid edited by hand or brought in from another app, and a key from the tags, an import or typed in, stay as they are.
- Phrases are stored in the library (`markers`, kind `phrase`, source `analysis`) whenever a track is analysed, and found again when its grid changes. A deck shows the stored phrases until its waveform is ready. This is task 1.4 of add-phrase-analysis.
- The right-click menu on a list, folder, smart list, stage or crate in the tree gets **Analyze All**, for every track in it.
- While analysis runs, a track's row shows a dot (dim while waiting, pulsing lilac while being analysed), and the footer shows "Analyzing 3 of 12" with a ✕ that stops the rest.
- The track menu no longer offers a colour. Colours already given to tracks stay and still show.

The label uses Seba's spelling, "Analyze".

## Out of scope

- Stems. Spleeter on load and HTDemucs in the background come with improve-stem-separation (v0.5), which adds them to Analyze Track then.
- Auto cues from phrases, energy levels and phrase search (add-phrase-analysis).
- Analysing on several threads at once.

## Release

v0.4 (Your collection, cleaned up).
