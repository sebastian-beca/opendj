# Keyboard shortcuts

| Key | Does |
|---|---|
| <kbd>Space</kbd> | Play or pause the master deck |
| <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>5</kbd> (Linux and Windows: <kbd>Ctrl</kbd>) | Rate the tracks picked in the browser 1 to 5 stars |
| <kbd>⌘</kbd> <kbd>,</kbd> (Linux and Windows: <kbd>Ctrl</kbd> <kbd>,</kbd>) | Open or close Settings |
| <kbd>Esc</kbd> | Close Settings |

## openDJscript

Each shortcut, apart from Space, ⌘, and Esc, runs a line of openDJscript,
a short action language in the style of VirtualDJ's VDJscript. ⌘3 runs
`rating 3`.

```text
rating 3                          rate the tracks picked in the browser
deck 1 rating 5                   rate the track on deck 1
deck 2 play_pause                 a verb on one deck; alone, it's the master deck
sync & play                       & runs one action after another
play ? pause : play               condition ? when true : when false
(sync & key_lock) ? play : pause  parentheses group actions
toggle 'slip_enabled'             any control by its Mixxx name
set '[Master],gain' 1             a control of another group
```

| Verb | Does |
|---|---|
| `play`, `pause`, `play_pause` | Start or stop the deck |
| `goto_start` | Jump to the start of the track |
| `sync`, `key_lock`, `quantize`, `slip` | Turn that on or off |
| `loop 4`, `loop_exit` | Loop that many beats (4 if left out), end the loop |
| `hot_cue 1` | Play from a hot cue, or set it |
| `eject` | Unload the deck |
| `volume 80%` | Set the channel fader |
| `crossfader` | Turn the crossfader on or off |
| `rating 0` to `rating 5` | Rate tracks; 0 clears the rating |
| `set`, `toggle`, `press`, `get` | Any control; `get` only asks, in a condition |
| `adjust 'rate' 50%` | Any control by its 0 to 1 position; a controller's knob gives the level when none is written |
| `value` | What the controller sent, 0 to 1; only in a controller's line, such as `value > 50% ? play : pause` |

In a condition a verb asks rather than does: `play ? …` asks whether the
deck is playing.

## Changing shortcuts

Open **Settings → Keyboard**. Each row is a key and the line it runs:

- Click the key, then press the new one (Esc leaves it as it was). If that
  key already did something, this shortcut takes it over and the page says
  what it replaced.
- Type in the line, or pick a verb from **Actions**; it's added with `&`
  when the line already has something. A mistake shows in red under it.
- **Reset** puts a changed default back, **Turn Off** stops a default key,
  and **Remove** deletes one of your own.
- **Add a key…** at the end starts a new shortcut, and **Reset All** goes
  back to openDJ's own.

Every verb is listed under ACTIONS at the bottom of the page.

## Shortcut sets

Shortcuts are kept in sets, chosen from the **Set** menu at the top of
**Settings → Keyboard**. openDJ starts with **Default**.

- **New set from this one** copies the set in use under a new name; type
  over the name to rename it.
- **Import…** reads a VirtualDJ keyboard mapping (`.xml` from
  `Documents/VirtualDJ/Mappers`) into a new set. Keys whose action openDJ
  can't do yet are left out, and the page says how many.
- **Delete this set** removes it and goes back to Default, which can't be
  deleted.
