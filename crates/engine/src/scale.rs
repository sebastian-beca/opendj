//! Scale mode for the instrument: notes played move to the nearest note
//! of a scale, so any key played sounds in key with the mix.
//!
//! `[Instrument1],scale_mode` is 0 Off, 1 Auto (the master deck's key) or
//! 2 the root and scale picked (`scale_root` 0 to 11 from C, `scale_type`
//! an index into [`SCALES`]). `scale_mask` shows the notes the scale has
//! now, a bit per pitch class from C, and `scale_now` which scale that is,
//! 1 + type × 12 + root; both 0 while every note plays as it is.

use opendj_plugin_host::Midi;

use crate::key::Key;

/// The scales to pick from, as `scale_type` numbers them.
pub const SCALES: [&str; 4] = ["Major", "Minor", "Major pentatonic", "Minor pentatonic"];

/// Each scale's notes, in semitones from its root.
const STEPS: [&[u8]; 4] = [
    &[0, 2, 4, 5, 7, 9, 11],
    &[0, 2, 3, 5, 7, 8, 10],
    &[0, 2, 4, 7, 9],
    &[0, 3, 5, 7, 10],
];

/// `scale_mode` values.
pub const OFF: u8 = 0;
pub const AUTO: u8 = 1;
pub const PICKED: u8 = 2;

/// The notes of scale `scale` (see [`SCALES`]) on `root` (0 to 11 from C),
/// a bit per pitch class from C.
pub fn mask(root: u8, scale: usize) -> u16 {
    STEPS[scale.min(STEPS.len() - 1)]
        .iter()
        .fold(0, |m, s| m | 1 << ((root + s) % 12))
}

/// The scale a mode plays in, as (root, scale): none with Off, or with
/// Auto while no deck leads with a key (`master`).
pub fn scale_for(mode: u8, root: u8, scale: usize, master: Option<Key>) -> Option<(u8, usize)> {
    match mode {
        AUTO => master.map(|k| (k.tonic % 12, usize::from(k.minor))),
        PICKED => Some((root % 12, scale.min(STEPS.len() - 1))),
        _ => None,
    }
}

/// [`scale_for`]'s notes; 0 for none.
pub fn mask_for(mode: u8, root: u8, scale: usize, master: Option<Key>) -> u16 {
    scale_for(mode, root, scale, master).map_or(0, |(r, s)| mask(r, s))
}

/// The note of `mask` nearest `key`, the lower one when two are as near;
/// `key` itself with no scale.
pub fn nearest(key: u8, mask: u16) -> u8 {
    let in_scale = |k: i32| (0..=127).contains(&k) && mask >> (k % 12) & 1 == 1;
    if mask & 0xfff == 0 || in_scale(key as i32) {
        return key;
    }
    (1..12)
        .flat_map(|d| [key as i32 - d, key as i32 + d])
        .find(|&k| in_scale(k))
        .map_or(key, |k| k as u8)
}

/// Moves notes to a scale as they come in, and ends each where it began
/// even if the scale changed while it was held. Two keys landing on the
/// same note hold it until both are let go.
pub struct ScaleMapper {
    /// Where each key's note went, by channel and key; 255 none.
    to: [[u8; 128]; 16],
    /// Keys holding each note, by channel and note.
    holding: [[u8; 128]; 16],
}

impl Default for ScaleMapper {
    fn default() -> Self {
        Self {
            to: [[255; 128]; 16],
            holding: [[0; 128]; 16],
        }
    }
}

impl ScaleMapper {
    /// `m` in scale `mask`, or none when it should not reach the
    /// instrument (a note-off for a note another key still holds).
    pub fn map(&mut self, mut m: Midi, mask: u16) -> Option<Midi> {
        if let Some((ch, key, _)) = m.as_note_on() {
            let (ch, key) = (ch as usize, key as usize);
            let out = nearest(key as u8, mask);
            if self.to[ch][key] != 255 {
                // Played again without a note-off: let go of the old one.
                let old = self.to[ch][key] as usize;
                self.holding[ch][old] = self.holding[ch][old].saturating_sub(1);
            }
            self.to[ch][key] = out;
            self.holding[ch][out as usize] = self.holding[ch][out as usize].saturating_add(1);
            m.data[1] = out;
            Some(m)
        } else if let Some((ch, key)) = m.as_note_off() {
            let (ch, key) = (ch as usize, key as usize);
            let out = std::mem::replace(&mut self.to[ch][key], 255);
            if out == 255 {
                return Some(m);
            }
            let held = &mut self.holding[ch][out as usize];
            *held = held.saturating_sub(1);
            if *held > 0 {
                return None;
            }
            m.data[1] = out;
            Some(m)
        } else {
            if m.data[0] & 0xf0 == 0xb0 && matches!(m.data[1], 120 | 123) {
                *self = Self::default();
            }
            Some(m)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_hold_their_notes() {
        assert_eq!(mask(0, 0), 0b1010_1011_0101, "C major: C D E F G A B");
        assert_eq!(mask(9, 1), mask(0, 0), "A minor has C major's notes");
        assert_eq!(mask(0, 2).count_ones(), 5);
        assert_eq!(mask(9, 3), mask(0, 2), "A minor pentatonic is C major's");
    }

    #[test]
    fn notes_move_to_the_nearest_in_the_scale() {
        let c_major = mask(0, 0);
        assert_eq!(nearest(60, c_major), 60);
        assert_eq!(nearest(61, c_major), 60, "C# goes down to C");
        assert_eq!(nearest(66, c_major), 65, "F# to F");
        assert_eq!(nearest(61, 0), 61, "no scale");
        // C major pentatonic has no F: F goes to E, the lower of E and G.
        let penta = mask(0, 2);
        assert_eq!(nearest(65, penta), 64);
        assert_eq!(nearest(71, penta), 72, "B up to C");
    }

    #[test]
    fn auto_follows_the_master_key_and_waits_without_one() {
        let a_minor = Key {
            tonic: 9,
            minor: true,
        };
        assert_eq!(mask_for(AUTO, 0, 0, Some(a_minor)), mask(9, 1));
        assert_eq!(mask_for(AUTO, 0, 0, None), 0);
        assert_eq!(mask_for(PICKED, 2, 3, Some(a_minor)), mask(2, 3));
        assert_eq!(mask_for(OFF, 2, 3, Some(a_minor)), 0);
    }

    #[test]
    fn a_note_ends_where_it_began() {
        let mut s = ScaleMapper::default();
        let c_major = mask(0, 0);
        let on = s.map(Midi::note_on(0, 0, 61, 100), c_major).unwrap();
        assert_eq!(on.data[1], 60);
        // The scale changes while it is held: its note-off still ends 60.
        let off = s.map(Midi::note_off(0, 0, 61), mask(2, 0)).unwrap();
        assert_eq!(off.data[1], 60);
        // C and C# both play C: it sounds until both are let go.
        s.map(Midi::note_on(0, 0, 60, 100), c_major).unwrap();
        s.map(Midi::note_on(0, 0, 61, 100), c_major).unwrap();
        assert!(s.map(Midi::note_off(0, 0, 61), c_major).is_none());
        assert_eq!(
            s.map(Midi::note_off(0, 0, 60), c_major).unwrap().data[1],
            60
        );
    }
}
