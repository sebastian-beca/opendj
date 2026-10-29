# Controllers

openDJ uses [Mixxx](https://mixxx.org)'s controller mappings, so most MIDI
controllers that Mixxx supports load their mapping as soon as they're plugged
in. There's nothing to set up.

**Settings › Controllers** lists the controllers plugged in that openDJ has a
mapping for. Other MIDI devices, such as virtual ports or gear without a
mapping, wait under **Add a controller**; pick one to list it and choose its
mapping. **Remove** takes a controller off the list and stops using it until
you add it again.

HID controllers aren't supported yet.

## Controller Editor

To see what every control does, press **Edit** next to a controller in
**Settings › Controllers**. The editor opens on that controller's mapping;
pick another at the top. Its controls are listed in three columns: the control
you press (with the layer it works in, such as SHIFT, a pad mode or a bank),
what it does, and the MIDI message it sends. They're grouped into sections
(Deck, Pads, Mixer, Effects, Library, More) with smaller sections inside, such
as Hot cues and Loops under Pads. Click a heading to open or close it. Search
the list by any of those words.

- **Picture.** For the [popular controllers](../controllers/README.md#popular-controllers),
  a drawing of the controller is on the left, with every control in about the
  place it sits. Point at a part to name it, and click it to list only what
  it does, with the messages it sends and the lights it shows.
- **Press to find.** With the controller plugged in, press or turn a control
  and the list jumps to it.
- **Lights** lists what each LED shows, and **Options** the mapping's own
  settings with their defaults.
- **Your own mappings.** The **Mapping** menu next to the controller has the
  original mapping, which never changes, and your own copies of it: choose
  **Duplicate…** to make one, and **Rename…** or **Delete…** to tidy up. For
  now a copy works just like the original; changing what a control does comes
  next.

Mappings that run a script are tried out in a sandbox to see what each
control does, so even those read in plain words.

## Popular controllers

openDJ looks after a list of popular controllers itself: the DDJ-FLX4,
DDJ-400, DDJ-200, DDJ-SB3, Mixtrack Platinum FX, Mixtrack Pro FX,
DJControl Inpulse 500, Xone K2 and Midi Fighter Twister so far, with the
DDJ-FLX6, DDJ-REV1, DDJ-1000, DDJ-FLX10 and Traktor Kontrol S2 MK3 coming.
Each has a drawing in the Controller Editor and a guide page.

They're marked **Beta** in **Settings › Controllers** and the editor until
someone has tried the mapping in openDJ on the real controller, and
**Supported** once that's done and the mapping is settled. If you have one
of them, try it and
[tell us how it goes](https://github.com/sebastian-beca/opendj/issues/new?template=controller.yml).

[Supported controllers](../controllers/README.md) lists every controller with
a mapping, and what each control does where the mapping describes it.

If your controller doesn't work right,
[tell us which one](https://github.com/sebastian-beca/opendj/issues/new?template=controller.yml).
Beta testers' controllers are fixed first.
