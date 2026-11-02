# Phone remote

Control Automix from your phone while you're away from the laptop: see
what's playing, change what comes next, skip a track or turn the music
down. Your phone opens a page openDJ serves, so there's no app to install.

## Pair your phone

1. Make sure your phone and the computer are on the same Wi-Fi.
2. Click **REMOTE** in the top bar and switch the remote on.
3. Scan the QR code with your phone's camera and open the link.

That's it: the phone shows the remote, and REMOTE turns green while a phone
is connected. Next time, just open the same page again (add it to your home
screen to make that one tap); the phone stays paired.

The QR code works once, for two minutes; a fresh one appears each time you
open the panel. If your phone can't scan, open the address shown under the
QR code in its browser and type the **PIN**.

The first time, macOS may ask whether openDJ may accept incoming network
connections. Click **Allow**.

## What the phone can do

- **Decks 1 and 2**: title, artist, tempo, key and time left. The deck you
  hear is outlined in green, and the next mix is shaded on the progress bar.
- **AUTOMIX** on and off, with the same status line as the Automix panel.
- **Mix now** and **Skip next**.
- **Length**, **Style** and **Order**, as in the Automix panel (see
  [Automix](automix.md)), with **Undo** after an Order pick.
- **Add tracks**: search your collection, with the same search as the
  browser (try `bpm:120-124`, `key:8A` or `crate:Warm-up`). Tap **+** to
  add a track to the end of the Automix list, or **Next** to play it next.
  Tracks already on the list show **✓ Added**.
- **Up next**: the Automix list. Drag a track by its handle to move it, or
  tap it for **Play next**, **Move up**, **Move down**, **Move to the end**
  and **Remove from Automix**. The track playing and the one waiting on a
  deck stay at the top.
- **Levels**: the master level, and the booth level when a booth output is
  set up. **Fade out** turns the master level down to silence over eight
  seconds; **Bring the sound back** puts it back.

Everything the phone does shows in openDJ straight away, and the other way
round.

## Phones and safety

- The remote is off until you switch it on, and only works on your local
  network.
- Only paired phones can control openDJ. Under **Paired phones** in the
  REMOTE panel you can see which are connected and **Remove** any of them.
- After five wrong codes or PINs in a row, pairing stops for a minute.
- The remote uses plain HTTP on your network. On a public network, anyone
  watching the traffic could see it; use a network you trust (see below).

## If your phone can't connect

Hotel, venue and café Wi-Fi often keeps devices apart, so the phone can't
reach the computer even on the same network. Either:

- share the computer's connection as its own Wi-Fi network (on a Mac:
  **System Settings › General › Sharing › Internet Sharing**) and join it
  with your phone, or
- turn on your phone's hotspot and join the computer to it.

If the computer is on more than one network, the REMOTE panel lists the
other addresses under **Other networks**.

A phone that locks its screen drops the connection; the page reconnects as
soon as you look at it again.
