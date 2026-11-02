# Loudness and broadcasting

openDJ streams your mix to internet radio (Icecast and Shoutcast servers) and
measures its loudness as it plays, so you can hit the level your station or
platform asks for.

## Streaming to a radio station

Your station or streaming host gives you a server address, a port, a mount
point and a source password. Put them into openDJ once:

1. Open **Settings › Broadcast** and click **Add** next to **Add a station**.
2. Fill in **Address**, **Port**, **Mount point** (Icecast) and **Password**.
   Leave **User** as `source` unless your host says otherwise.
3. Choose the **Format** and **Bitrate**. **MP3 at 128 kb/s** plays everywhere
   and shows listeners each track's title. **Opus** sounds better at low
   bitrates (96 kb/s is plenty).
4. Click **Test**. openDJ connects, checks the password and hangs up, without
   going on air.

The password is kept in your system's keychain (Keychain on macOS, Credential
Manager on Windows, the Secret Service on Linux), never in openDJ's settings
file. Where there's no keychain, openDJ remembers it until you quit.

You can add several stations and switch each on or off: **ON AIR** streams to
every station that's switched on, at once.

### Going on air

Click **ON AIR** in the top bar, then **Go on air**. The button turns red and
counts the time on air. Each station gets a light:

| Light | Means |
|---|---|
| Red | On air |
| Amber, pulsing | Connecting, or reconnecting after a drop |
| Grey | Gave up: the message says why (wrong password, mount point in use, no server) |

If the connection drops, openDJ reconnects at once and then every 5 seconds, for
up to 10 minutes. A slow upload never makes the music stutter: if it falls more
than about 10 seconds behind, openDJ reconnects that station and carries on from
now. Icecast servers also report how many people are listening, next to the
time.

Click **Go off air** to stop. Controllers with a broadcast button (mapped to
Mixxx's `[Shoutcast],enabled`) go on and off air too.

### What listeners see

Listeners see the track that's heard most (playing, with its channel fader up,
on the crossfader's live side), a couple of seconds after it takes over, so a
quick cut back and forth doesn't flicker. Change the text in **Settings ›
Broadcast › Title sent to listeners**: `{artist}`, `{title}` and `{dj}` are
filled in, and plain text such as `Live from the club` is sent as it is.

MP3 streams update the title on the server, which every player shows. Opus
carries titles inside the stream: the first title always goes out, and later
ones only with **Send each new title in the stream**, because starting the
stream again at each track stops some players.

### Licences

Playing other people's music to the public needs licences in most countries,
from bodies such as PRS, GEMA, SACEM, ASCAP, BMI and SoundExchange. Many radio
hosts include them; check yours before you go on air.

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
again. They also start again when you press **REC** or go on air.

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

## The limiter while recording or on air

The master always runs through a limiter, so the mix never clips. While you
record or are on air, it also keeps the peaks between samples under **−1 dB**
true peak, which MP3, AAC and Opus need so they don't distort. For streams at
low bitrates (96 kb/s or less), choose **−2 dBTP** in **Settings › Audio ›
Loudness**.

## For controllers

| Control | Value |
|---|---|
| `[Master],loudness_momentary`, `loudness_short_term`, `loudness_integrated` | LUFS, or −100 with nothing to measure |
| `[Master],loudness_range` | LU |
| `[Master],true_peak` | dBTP, highest since the reset |
| `[Master],loudness_reset` | Set to 1 to reset |
| `[Master],loudness_target` | The target, in LUFS |
| `[Master],air_ceiling` | The limiter's ceiling while recording or on air, in dBTP |
| `[Shoutcast],enabled` | 1 goes on air, 0 off air |
| `[Shoutcast],status` | 0 off air, 1 connecting, 2 on air to every station, 3 failed, 4 on air to some |
| `[Broadcast],on_air_seconds` | Time on air, in seconds |
