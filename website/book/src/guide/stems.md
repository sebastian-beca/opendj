# Stems

Every track you load gets stems: openDJ separates it with Spleeter as it
loads, starting with the part around the cue point, so the stems pads work
within a second or two. Nothing needs installing. The first time, openDJ
downloads the stems model (about 80 MB).

The stems play over the original track: with every stem on you hear the track
exactly as it is, and turning a stem down only takes that stem out. Where a
part of the track isn't separated yet, the original plays.

## Stems pads

Open the **STEMS** pad page (right-click a pad tab to put it there). The pads
are laid out as in VirtualDJ:

| Vocal | Instru | Bass | Acapella |
|---|---|---|---|
| **Kick** | **HiHat** | **Stems FX** | **Instrumental** |

- **Vocal**, **Instru**, **Bass**, **Kick** and **HiHat** mute and unmute
  their stem. A pad is lit while its stem plays.
- **Acapella** plays the vocals alone; press it again for every stem.
- **Instrumental** plays everything but the vocals; press it again for every
  stem.
- **Stems FX** isn't used yet.

Set how loud each stem is with the mixer's knobs in stems mode (the **EQ**
menu above the mixer).

On the DDJ-FLX4, press **KEYBOARD** twice for the stems pads, in the same
layout. **SHIFT** + a stem's pad plays that stem alone.

## High quality stems

Spleeter is quick but lets a little of each stem bleed into the others. For a
cleaner split, right-click a track in the browser, or a stems pad, and choose
**Prepare High Quality Stems**. openDJ separates it with HTDemucs in the
background (on the graphics card; the first time it downloads the 84 MB model),
one track at a time. When it's done the deck switches to the new stems, and the
track loads with them from then on.

NI stem files (`.stem.mp4`) play their own stems.
