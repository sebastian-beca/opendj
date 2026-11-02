# Loudness and broadcasting

Internet radio stations, podcasts and livestream platforms all play music at a
set loudness, measured in LUFS (loudness units, the way EBU R128 measures it).
openDJ measures the loudness of your mix as it plays, so you can hit the level
your station or platform asks for.

## The loudness meter

Open the **MASTER** button in the top bar. Under the master meter, **Loudness**
shows:

| Reading | What it is |
|---|---|
| The big number | **Short-term** loudness: the last 3 seconds. Steer by this one. |
| **MOMENTARY** | The last 0.4 seconds. Jumps with every kick and break. |
| **INTEGRATED** | The whole mix since the last reset: what a platform measures for your set. |
| **RANGE** | Loudness range in LU: how far quiet and loud parts are apart. 5 to 10 LU is usual for a DJ set. |
| **TRUE PEAK** | The highest peak since the reset, including peaks between samples, in dB. Red above −1 dB. |

The readings are green within 1 LU of your target, amber within 3 and red
further off. **Reset** starts integrated loudness, the range and the true peak
again. They also start again when you press **REC**.

While recording, the **REC** button in the top bar shows the short-term
loudness next to the time.

The master meter above it shows peaks in dB, marked from −40 to 0. It reads
after the limiter, so it stops just under 0.

## Choosing a target

Pick the target in **Settings › Audio › Loudness**:

| Target | For |
|---|---|
| **Internet radio · −16 LUFS** (the default) | Icecast and Shoutcast stations, mix shows |
| **Podcast or talk · −18 LUFS** | Shows with a lot of talking |
| **Livestream · −14 LUFS** | Twitch, YouTube, Mixcloud, and uploads to streaming services |
| **Broadcast, EBU R128 · −23 LUFS** | FM and DAB radio, TV |

The target only colours the meter: it doesn't change any level. To move your
mix, turn the master down (or up) until the short-term reading sits green most
of the time. With **Auto gain** on (**Settings › Mixer**), every track already
starts at the same loudness, so the mix stays close once it's set.

## The limiter while recording

The master always runs through a limiter, so the mix never clips. While you
record, it also keeps the peaks between samples under **−1 dB** true peak, which
MP3, AAC and Opus need so they don't distort. For streams at low bitrates
(96 kb/s or less), choose **−2 dBTP** in **Settings › Audio › Loudness**.

## For controllers

| Control | Value |
|---|---|
| `[Master],loudness_momentary`, `loudness_short_term`, `loudness_integrated` | LUFS, or −100 with nothing to measure |
| `[Master],loudness_range` | LU |
| `[Master],true_peak` | dBTP, highest since the reset |
| `[Master],loudness_reset` | Set to 1 to reset |
| `[Master],loudness_target` | The target, in LUFS |
| `[Master],air_ceiling` | The limiter's ceiling while recording, in dBTP |
