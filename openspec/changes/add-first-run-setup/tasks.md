# Tasks

## 1. Tour engine

- [x] 1.1 Tour overlay: highlight a control, tip, Next/Skip/Close, steps that wait for the DJ to do something (`tour.rs`, `tour.slint`, `tourmark.slint`; a lesson is data, so `add-tutorials` adds lessons)

## 2. First-run tour

- [x] 2.1 Audio (with a test tone, `[Master],test_tone`), controller (if present, with a "press a button" check) and library import steps
- [x] 2.2 Simple playback and mixing steps on 2-Deck; screenshot each step
- [x] 2.3 Settings › Help (was Feedback) runs it again, whole or one part; a DJ upgrading from 0.4 is offered it once
- [x] 2.4 Update the matching user guide page in website/book/src/guide (install.md)
- [ ] 2.5 Ship the two CC0 demo tracks in `res/demo` and copy them into the bundles (until then the lesson uses two built-in synthesised tracks)
- [ ] 2.6 Quick map in the controller step for a controller with no mapping, once the controller mapper (`add-controller-mapper`, #133) has it

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
