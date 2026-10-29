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

- **Picture.** For the Pioneer DDJ-FLX4, Allen & Heath Xone K2 and DJ
  TechTools Midi Fighter Twister, a picture of the controller is on the left.
  Point at a part to name it, and click it to list only what it does.
- **Press to find.** With the controller plugged in, press or turn a control
  and the list jumps to it.
- **Lights** lists what each LED shows, and **Options** the mapping's own
  settings with their defaults.
- **Your own mappings.** The **Mapping** menu next to the controller has the
  original mapping, which never changes, and your own copies of it: choose
  **Duplicate…** to make one, and **Rename…** or **Delete…** to tidy up. For
  now a copy works just like the original; changing what a control does comes
  next.
- **Import…** in the same menu brings a mapping across from another DJ app:
  VirtualDJ (`.xml` from `Documents/VirtualDJ/Mappers`), Traktor (`.tsi`),
  rekordbox (`.midi.csv`), djay Pro (`.djayMidiMapping`) or an openDJ
  `.opendj-mapping`. Each file becomes a mapping of your own, built on the
  controller's original mapping. Controls openDJ can't do yet are kept but
  greyed, with the reason, and the original keeps working for them; a report
  says how many came across. A VirtualDJ keyboard file becomes a keyboard
  shortcut set instead (see [Keyboard shortcuts](shortcuts.md)).
- **Export…** writes the mapping as one `.opendj-mapping` file that works on
  another computer, and **Export for Mixxx…** writes a `.midi.xml` with its
  scripts; a report lists anything Mixxx can't do.

Mappings that run a script are tried out in a sandbox to see what each
control does, so even those read in plain words.

[Supported controllers](../controllers/README.md) lists every controller with
a mapping, and what each control does where the mapping describes it.

If your controller doesn't work right,
[tell us which one](https://github.com/sebastian-beca/opendj/issues/new?template=controller.yml).
Beta testers' controllers are fixed first.
