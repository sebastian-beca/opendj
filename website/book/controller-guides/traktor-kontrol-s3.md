## Getting started

The S3 is a USB HID controller, not a MIDI one: openDJ talks to it
directly and runs Mixxx's HID mapping for it. Plug it in and it shows up in
**Settings › Controllers** within a second or so, running this mapping. It's
also a sound card: choose it in **Settings › Audio**, with the main output on
channels 1-2 and the headphones on 3-4 (check its manual if the sound comes out of the wrong place).

- **macOS:** quit Traktor first. If the controller still isn't picked up,
  press **Refresh** under the list.
- **Windows:** close any other DJ app that has the controller open.
- **Linux:** the system only lets root open HID devices until you add
  openDJ's udev rule, `70-opendj-hid-controllers.rules` (in `share/opendj`
  in the download, `res/linux` in the source). Copy it to
  `/etc/udev/rules.d/`, run
  `sudo udevadm control --reload-rules && sudo udevadm trigger`, and plug the
  controller in again.

HID mappings are all script, so the Controller Editor has no picture or list
for it, and it can't be remapped there. The mapping's own options (jog wheel
feel, pad colours and so on) follow Mixxx's defaults.

## When you try it

- Load a track with the browse encoder, play it, and scratch and nudge it with
  the jog wheels.
- Try the pad modes (**HOTCUES**, **SAMPLES**) with and without SHIFT.
- Move every fader and knob, and check the tempo faders and the deck C and D buttons.
- Check that the button and pad lights, and the level meters, follow what's
  on screen.
- Unplug it while openDJ runs, plug it back in, and check it comes back.
