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
pick another at the top. On the right, its controls are listed by function (Transport, Hot
cues, Loops, Mixer, Effects and so on), each with the control it's on, the
layer it works in (SHIFT, a pad mode, a bank) and the MIDI message it sends.
Search the list by any of those.

- **Picture.** For the Pioneer DDJ-FLX4, Allen & Heath Xone K2 and DJ
  TechTools Midi Fighter Twister, a picture of the controller is on the left.
  Point at a part to name it, and click it to list only what it does.
- **Press to find.** With the controller plugged in, press or turn a control
  and the list jumps to it.
- **Lights** lists what each LED shows, and **Options** the mapping's own
  settings with their defaults.

Mappings that run a script are tried out in a sandbox to see what each
control does, so even those read in plain words. For now the editor only
shows the mapping you're using; changing it comes later.

[Supported controllers](../controllers/README.md) lists every controller with
a mapping, and what each control does where the mapping describes it.

If your controller doesn't work right,
[tell us which one](https://github.com/sebastian-beca/opendj/issues/new?template=controller.yml).
Beta testers' controllers are fixed first.
