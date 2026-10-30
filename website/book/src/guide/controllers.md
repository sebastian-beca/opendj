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

The **Pioneer DDJ-FLX4** has a page of its own: [Pioneer DDJ-FLX4](controllers/ddj-flx4.md).

## Controller Editor

To see what every control does, press **Edit** next to a controller in
**Settings › Controllers**. The editor opens on that controller's mapping;
pick another at the top. Its controls are listed in three columns: the control
you press (with the layer it works in, such as SHIFT, a pad mode or a bank),
what it does, and the MIDI message it sends. They're grouped into sections
(Deck, Pads, Mixer, Effects, Library, More) with smaller sections inside, such
as Hot cues and Loops under Pads. Click a heading to open or close it. Search
the list by any of those words.

- **Picture.** For the [popular controllers](../controllers/index.html#popular-controllers),
  a drawing of the controller is on the left, with every control in about the
  place it sits. Point at a part to name it, and click it to list only what
  it does, with the messages it sends and the lights it shows.
- **Press to find.** With the controller plugged in, press or turn a control
  and the list jumps to it.
- **Lights** lists what each LED shows, and **Options** the mapping's own
  settings with their defaults.
- **Your own mappings.** The **Mapping** menu next to the controller has the
  original mapping, which never changes, and your own copies of it: choose
  **Duplicate…** to make one, and **Rename…** or **Delete…** to tidy up. The
  controller runs the one picked.
- **Change a control.** Click a row and the panel under the list shows what it
  does. **Change…** opens every function by category, with a search; pick one
  and the controller uses it at once. Pick the deck or effect unit beside it.
  **Does nothing** turns the control off, and **Back to original** puts the
  row back as the original mapping has it. Changed rows are marked in lilac.
  Changing the original makes your own copy first, called "My" and the
  controller's name, so the original stays as it is.
- **Advanced**, folded under the function, sets when the row acts (press,
  release, long press, tap or double press, so one button can do more than one
  thing), how a button is read, the knob type (absolute, or one of the endless
  kinds), pick-up for knobs and faders, and invert. Each has a short hint.
  A row run by the mapping's script can be given a plain function too; the
  script keeps running the other controls.
- **Import…** in the same menu brings a mapping across from another DJ app:
  VirtualDJ (`.xml` from `Documents/VirtualDJ/Mappers`), Traktor (`.tsi`),
  rekordbox (`.midi.csv`), djay Pro (`.djayMidiMapping`) or an openDJ
  `.opendj-mapping`. Each file becomes a mapping of your own, built on the
  controller's original mapping. Controls openDJ can't do yet are kept but
  greyed, with the reason, and the original keeps working for them; a report
  says how many came across. A VirtualDJ keyboard file becomes a keyboard
  shortcut set instead (see [Keyboard shortcuts](shortcuts.md)).
- **2 or 4 decks.** A mapping with a layout for two decks and one for four,
  such as VirtualDJ's Xone K2, adds **Use with** under the controller in
  **Settings › Controllers**, and with four decks **Deck order**: which deck
  each column is from left to right (3 1 2 4 to start, as on the 4-Deck
  screen).
- **Export…** writes the mapping as one `.opendj-mapping` file that works on
  another computer, and **Export for Mixxx…** writes a `.midi.xml` with its
  scripts; a report lists anything Mixxx can't do.

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

[Supported controllers](../controllers/index.html) lists every controller with
a mapping, and what each control does where the mapping describes it.

If your controller doesn't work right,
[tell us which one](https://github.com/sebastian-beca/opendj/issues/new?template=controller.yml).
Beta testers' controllers are fixed first.
