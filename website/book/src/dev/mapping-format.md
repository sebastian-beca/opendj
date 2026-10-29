# Controller mapping format

openDJ keeps a DJ's own controller mappings, and the ones it imports from
other DJ apps, in one documented format: a JSON file with the extension
`.opendj-mapping`. This page is the reference for version 1.

The format is a **superset of Mixxx's** `.midi.xml`: anything a Mixxx
mapping says can be written in it without loss, and it adds what other apps
have and Mixxx doesn't (one-line openDJscript actions, shift layers, long
press, LED scripts with colours, variables). A mapping can stand alone or be
**based on** a Mixxx mapping and hold only what it changes.

## A small example

```json
{
  "format": "opendj-mapping",
  "version": 1,
  "name": "DDJ-FLX4 Nahuel 2026",
  "author": "Seba",
  "controller": { "name": "Pioneer DDJ-FLX4", "ports": ["DDJ-FLX4"], "layout": "flx4" },
  "based_on": "Pioneer-DDJ-FLX4.midi.xml",
  "imported": { "app": "VirtualDJ", "file": "Pioneer DDJ-FLX4 - DDJ-FLX4 Nahuel 2026.xml", "date": "2026-10-29" },
  "rows": [
    { "midi": "97 04", "name": "HOTCUE_PAD5", "script": "deck 1 play_pause" },
    { "midi": "98 05", "name": "SHIFT_HOTCUE_PAD6", "script": "deck 1 pitch_zero" },
    { "midi": "90 58", "name": "SYNC", "when": "long", "script": "deck 1 master_deck" },
    { "midi": "B0 21", "name": "JOG", "off": true, "original": "touchwheel",
      "note": "kept the jog from openDJ's own FLX4 mapping" }
  ],
  "lights": [
    { "midi": "97 04", "script": "deck 1 play ? on : off" }
  ]
}
```

## Top level

| Field | | What it is |
|---|---|---|
| `format` | required | Always `"opendj-mapping"`. |
| `version` | required | `1`. A reader refuses a higher major version it doesn't know. |
| `name` | required | Shown in the Controller Editor's Mapping menu. |
| `author`, `description` | optional | Free text. |
| `controller` | optional | `name` (shown), `ports` (MIDI port names it matches, as Mixxx's `<info>`), `layout` (openDJ's drawing of the controller: `flx4`, `xone-k2`, `twister`). |
| `based_on` | optional | A Mixxx mapping file name. Its bindings, lights, scripts and settings run first; `rows` and `lights` here replace or add to them. Without it the file is the whole mapping. |
| `imported` | optional | Where it came from: `app` (`VirtualDJ`, `Traktor`, `rekordbox`, `Mixxx`), `file`, `date`. |
| `scripts` | optional | JavaScript files run as Mixxx runs them: `[{ "file": "...", "prefix": "..." }]`. A shared file carries them in `sources` (below). |
| `settings` | optional | The mapping's options: Mixxx's `<settings>` definitions as `[{ "variable", "type", "label", "default", "choices" }]`, plus `values` with the DJ's picks. |
| `variables` | optional | Variables the scripts use: `{ "k2mode": { "default": 0, "saved": true } }`. `saved` keeps the value across restarts (VirtualDJ's `@$`). |
| `rows` | optional | What the controls do, below. |
| `lights` | optional | What the LEDs show, below. |
| `sources` | optional | Only in a shared file: `{ "file name": "file text" }` for each script and for `based_on` when it isn't one openDJ ships. |

## Rows

A row says what one MIDI message does, on one layer.

| Field | | What it is |
|---|---|---|
| `midi` | required | Status and number in hex: `"90 0B"` (note 0B, channel 1), `"B0 13"` (CC 13). A 14-bit pair is `"B0 00+B0 20"` (MSB then LSB). Pitch bend is `"E0"`. |
| `name` | optional | The control's name in the app it came from (`PLAY_PAUSE`). Shown in the editor. |
| `layer` | optional | Variables the row needs, joined with `&`: `"shift"` acts only while the variable `shift` is above 0, `"m1=2 & m2=0"` only while `m1` is 2 and `m2` is 0 (Traktor's modifiers). Hardware shift (controllers that send other notes with SHIFT held) needs no layer: it's another `midi`. |
| `when` | optional | `"press"` (default; for a fader or knob, every move), `"release"`, `"long"` (held 500 ms), `"tap"` (let go before 500 ms), `"double"` (two presses within 300 ms). VirtualDJ's `holding ? a : b` becomes a `long` row and a `tap` row. |
| one action | required unless `off` | `bind`: a control, `"[Channel1],play"`, with Mixxx's `options` (`["button"]`, `["soft-takeover", "invert"]`, `["rot64"]` …). `js`: a script function, `"PioneerDDJFLX4.jog"`. `script`: an openDJscript line, `"deck 1 loop 4"`. |
| `takeover` | optional | For a `bind` to a fader or knob: `"jump"` (default), `"pickup"` (waits for the control to reach the value, Mixxx's soft takeover) or `"scale"` (moves towards the value as you turn). |
| `range` | optional | For a `bind`: the part of the control's range the hardware covers, `[0.25, 0.75]` (0 to 1). |
| `off` | optional | `true`: the row is kept but does nothing (greyed in the editor). Imports use it for actions openDJ can't do yet. |
| `original` | optional | The action as the other app wrote it (`"touchwheel 'search'"`). |
| `note`, `description` | optional | Why it's off; what the author wrote about it. |
| `remove` | optional | `true`: take the `based_on` mapping's bindings for this message away. |

**Merging with `based_on`.** Rows are matched to the base mapping by `midi`,
`layer` and `when`. A row replaces every base binding of that message on that
layer; a row with `remove` deletes them; anything else is added. An `off`
row replaces nothing: the base binding keeps working underneath, so an
import can never make a control go dead that worked before.

**Values.** A `script` row on a fader, knob or encoder gets the control's
value: verbs that set a level (`volume`, `eq_high`, `filter`) take it when
no level is written, as a 0 to 1 number (7-bit or 14-bit), or, for an
encoder (`options: ["rot64"]` or `["diff"]`), as steps.

## Lights

| Field | | What it is |
|---|---|---|
| `midi` | required | The message sent: `"90 0B"`. |
| `bind` + `min`, `max`, `on_value`, `off_value` | one of | Mixxx's `<output>`: send `on_value` (127 unless given) while the control is between `min` and `max`, else `off_value` (0). |
| `js` | one of | Lights driven by the scripts (nothing to do here; listed so the editor can show them). |
| `script` | one of | An openDJscript line asked several times a second; it ends in `on`, `off`, `blink` (with a time: `blink 500ms`), `color 'red'` or a number 0 to 127. A condition picks between them: `get_vu_meter > 75% ? off : color 'orange'`. |
| `colors` | optional | What `color` sends on this controller: `{ "red": 15, "orange": 51, "green": 60 }`. |
| `name`, `off`, `original`, `note` | optional | As for rows. |

## How it maps to Mixxx

| Mixxx `.midi.xml` | openDJ mapping |
|---|---|
| `<info><name>`, `<author>`, `<description>` | `name`, `author`, `description` |
| `<info>` device matching | `controller.ports` |
| `<settings>` options | `settings` |
| `<scriptfiles><file filename functionprefix>` | `scripts` |
| `<control>` with a control `key` and `<options>` | row with `bind` and `options` |
| `<control>` with `<Script-Binding/>` | row with `js` |
| `<control>` pair with `fourteen-bit-msb/lsb` | one row, `midi: "B0 00+B0 20"` |
| `<output>` | light with `bind`, `min`, `max`, `on_value`, `off_value` |
| — | rows with `script`, `layer`, `when`, `off`; lights with `script`; `variables`; `based_on`; `imported` |

So a Mixxx mapping converts to an openDJ mapping with nothing lost. Going
back, **Export as Mixxx** writes every `bind` and `js` row and every `bind`
light; an openDJscript row that is just one control (`deck 1 sync`) becomes a
plain binding; the rest are left out and listed in the export report. A
`based_on` mapping is exported with its base merged in and its script files
copied next to it.

## Files and sharing

- The DJ's mappings live in `<config>/controller-mappings/<id>.json`, in this
  format. Files from openDJ 0.4 (`name`, `based_on` and `changes`, no
  `format`) are read as version 1 with `changes` as `rows`.
- **Export** writes one self-contained `.opendj-mapping`: scripts that don't
  ship with openDJ go in `sources`, so the file works on another computer.
- A reader keeps fields it doesn't know when it writes a file back, so newer
  files survive older versions of openDJ.
