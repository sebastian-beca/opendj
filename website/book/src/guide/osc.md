# OSC

Open Sound Control (OSC) lets tablet apps such as TouchOSC and Open Stage
Control, visuals software such as Resolume, VDMX and TouchDesigner, and show
software such as QLab press any button in openDJ, move any fader, and follow
what changes.

OSC is off at first. Turn it on in **Settings › Controllers › OSC**.

| Setting | What it does |
| --- | --- |
| **Port** | The UDP port openDJ listens on. 9000 at first. |
| **Send to** | Apps to send changes and events to, as `address:port`, separated by commas, for example `192.168.1.20:8000`. |
| **Only this computer** | On at first: only apps on this computer can reach openDJ. Turn it off for a tablet. |
| **Send only** | Ignores incoming controls, so other apps can follow openDJ but not change it. |

The line under the switch shows the port and how many apps have talked to
openDJ.

## Addresses

Every control has an address made from its name: the group in lower case
without its brackets (spaces become `_`), then the key.

| Control | OSC address |
| --- | --- |
| `[Channel1],play` | `/channel1/play` |
| `[Channel2],volume` | `/channel2/volume` |
| `[Master],crossfader` | `/master/crossfader` |
| `[Channel1],hotcue_1_activate` | `/channel1/hotcue_1_activate` |
| `[EqualizerRack1_[Channel1]_Effect1],parameter1` | `/equalizerrack1_channel1_effect1/parameter1` |
| `[QuickEffectRack1_[Channel1]],super1` | `/quickeffectrack1_channel1/super1` |

Every name in the [list of supported controls](https://github.com/openwave-studio/opendj/blob/main/website/book/supported-controls.txt)
works. Addresses ignore upper and lower case.

- **Set a control:** send the address with one number (float, int or
  true/false). `/channel1/play 1` plays deck 1, exactly as pressing PLAY does.
  The number is the control's own value, not 0 to 1: the crossfader goes from
  −1 to 1, EQ knobs from 0 to 4 with 1 in the middle.
- **Ask for a value:** send the address with no number. openDJ answers with the
  address and the value.
- **Many at once:** OSC patterns work. `/channel*/play 0` stops every deck.

Use a toggle button, not a momentary one, for `play`, `pfl` and other on/off
controls: a momentary button sends 1 then 0 and would start and stop the deck.

## Following openDJ

Once an app has set or asked for a control, openDJ sends it the new value each
time it changes, at most 30 times a second. To follow controls without
touching them, send `/subscribe` with a pattern:

```
/subscribe "/channel1/*"
/subscribe "/master/crossfader"
```

openDJ sends the current values at once, then every change. `/unsubscribe`
with the same pattern stops them. Controls made later, for example when a
fourth deck opens, are followed too.

openDJ doesn't send an app's own move back to it, so a fader doesn't jump
while you drag it. If a control can't move (the crossfader while Automix holds
it), openDJ answers with where it stayed.

## Apps that listen on another port

Many apps, TouchOSC among them, send from one port and listen on another. Put
the app's address and listening port in **Send to**: openDJ then answers
messages from that computer there.

## Events for visuals and lights

While OSC is on, openDJ sends these to every app in **Send to**, and to any app
that subscribed to them (`/subscribe "/beat"`, `/subscribe "/deck/*/track"`):

| Address | Arguments | When |
| --- | --- | --- |
| `/beat` | beat in the bar (1–4), bar in the phrase (from 1), BPM | each beat of the master deck |
| `/phrase` | phrase name, length in bars | the master deck starts a new phrase |
| `/deck/N/track` | title, artist, BPM, key | a track loads on deck N |
| `/deck/N/cue` | hot cue number | a hot cue is pressed on deck N |

The master deck is the one playing; with several playing, the one that started
first, as for [Ableton Link](link.md). Beats and phrases come from the beatgrid
and the [phrase analysis](decks.md), the same ones the beat counter in the deck
header shows. Phrase names are Intro, Build, Drop, Breakdown, Chorus, Verse and Outro;
a track without phrase analysis counts phrases of eight bars, named Phrase.
Nothing is sent for a jump or a scratch backwards, only for beats as they play.

In Resolume, for example, map `/phrase` to a clip trigger to change the visuals
on each drop, and `/beat` to a flash on the downbeat.

## Finding openDJ on the network

With **Only this computer** off, openDJ announces itself over Bonjour as an OSC
service named openDJ. In TouchOSC's connection settings, openDJ is listed under
the browse button, so you don't have to type the address.

## A two-deck layout

To build a TouchOSC layout for two decks, add these controls and set each one's
OSC address and value range. Then send `/subscribe "/channel*/*"` and
`/subscribe "/master/crossfader"` when the layout starts (TouchOSC's
**On Start** script, or a button) so its faders follow openDJ.

| Control | Type | Address | Range |
| --- | --- | --- | --- |
| Play | toggle button | `/channel1/play` | 0 to 1 |
| Cue | momentary button | `/channel1/cue_default` | 0 to 1 |
| Sync | toggle button | `/channel1/sync_enabled` | 0 to 1 |
| Volume | vertical fader | `/channel1/volume` | 0 to 1 |
| Filter | knob | `/quickeffectrack1_channel1/super1` | 0 to 1 |
| High, Mid, Low | knobs | `/equalizerrack1_channel1_effect1/parameter3`, `parameter2`, `parameter1` | 0 to 4 |
| Hot cues 1–8 | momentary buttons | `/channel1/hotcue_1_activate` … `hotcue_8_activate` | 0 to 1 |
| Headphones | toggle button | `/channel1/pfl` | 0 to 1 |
| Crossfader | horizontal fader | `/master/crossfader` | −1 to 1 |

Use `channel2` for deck 2.

## Performance

Nothing OSC does runs on the audio thread. With OSC off, no port is open.
