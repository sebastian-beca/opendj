# Keyboard shortcuts

| Key | Does |
|---|---|
| <kbd>Space</kbd> | Play or pause the master deck |
| <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>5</kbd> (Linux: <kbd>Ctrl</kbd>) | Rate the tracks picked in the browser 1 to 5 stars |
| <kbd>⌘</kbd> <kbd>,</kbd> (Linux: <kbd>Ctrl</kbd> <kbd>,</kbd>) | Open or close Settings |
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

In a condition a verb asks rather than does: `play ? …` asks whether the
deck is playing. You can't change the shortcuts in the app yet; that comes
with the shortcuts editor.
