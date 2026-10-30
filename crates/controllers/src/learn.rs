//! Map mode's learning: what a hardware control sent while an on-screen
//! control was picked, and the row of the DJ's own mapping that binds the
//! two. Like the editor's changes (see [`crate::edit`]), a learned control
//! is a row put over the Mixxx mapping the own mapping is based on.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::catalog::describe;
use crate::format::{Midi, OpenDjMapping, Row};
use crate::mapping::{Mapping, Options, When};

/// What a hardware control sent, read as a binding: its message and how
/// its value is to be read.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub midi: Midi,
    /// Mixxx option names: `diff` or `rot64` for an endless encoder.
    pub options: Vec<String>,
}

/// Whether `msg` starts a control's message: a press, a turn or a move
/// (not a let-go, which follows every press).
pub fn starts(msg: &[u8]) -> bool {
    match msg {
        [s, _, v, ..] if s & 0xF0 == 0x90 => *v > 0,
        [s, _, _, ..] => matches!(s & 0xF0, 0xB0 | 0xE0),
        _ => false,
    }
}

/// Whether `msgs` (from one controller, oldest first) say enough to learn
/// from: a press does at once, a knob or fader after a few moves.
pub fn enough(msgs: &[Vec<u8>]) -> bool {
    match msgs.first() {
        Some(m) if m[0] & 0xF0 == 0x90 => true,
        Some(_) => msgs.len() >= 6,
        None => false,
    }
}

/// Reads what a control sent, oldest first: the first message that starts
/// one, the other half of a 14-bit pair if the next messages carry it, and
/// for a knob that keeps sending the same small steps, the endless mode.
pub fn hear(msgs: &[Vec<u8>]) -> Option<Heard> {
    let first = msgs.iter().find(|m| starts(m))?;
    let (status, number) = (first[0], first[1]);
    let mut midi = Midi::new(status, number);
    let mut options = Vec::new();
    if status & 0xF0 == 0xB0 {
        // Faders sending 14 bits: CC n with its fine half on CC n + 32.
        if number < 0x20
            && msgs
                .iter()
                .any(|m| m.len() >= 3 && m[0] == status && m[1] == number + 0x20)
        {
            midi.lsb = Some((status, number + 0x20));
        } else {
            let values: Vec<u8> = msgs
                .iter()
                .filter(|m| m.len() >= 3 && m[0] == status && m[1] == number)
                .map(|m| m[2])
                .collect();
            let distinct: HashSet<u8> = values.iter().copied().collect();
            // An absolute knob passes through values; an endless one sends
            // the same step again and again.
            if values.len() >= 3 && distinct.len() <= 2 {
                if values.iter().all(|&v| v <= 8 || v >= 120) {
                    options.push("diff".into());
                } else if values.iter().all(|&v| (56..=72).contains(&v) && v != 64) {
                    options.push("rot64".into());
                }
            }
        }
    }
    Some(Heard { midi, options })
}

/// Binds what was heard to `control` (`[Channel1],play`) in the own mapping,
/// pressing on no layer. What that message did there before goes, unless
/// `keep`, when it carries on doing it too. Returns what it did before, in
/// words ("Cue · Deck 1").
pub fn learn(
    own: &mut OpenDjMapping,
    base: Option<&Mapping>,
    heard: &Heard,
    control: &str,
    name: &str,
    keep: bool,
) -> Vec<String> {
    let key = (heard.midi.status, heard.midi.number);
    let built = own.build(base, Path::new(""));
    let before: Vec<_> = built
        .inputs
        .get(&key)
        .map(|list| {
            list.iter()
                .filter(|i| i.layer.is_empty() && i.when == When::Press)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let same = |r: &Row| {
        (r.midi.status, r.midi.number) == key
            && r.layer.is_empty()
            && r.when == When::Press
            && !r.off
    };
    let mine = own.rows.iter().any(same);
    if !keep {
        own.rows.retain(|r| !same(r));
    } else if !mine && !before.is_empty() {
        // The base's bindings become rows of the file, so the new row
        // doesn't push them out.
        let mut only = Mapping {
            name: String::new(),
            author: String::new(),
            path: Default::default(),
            scripts: Vec::new(),
            settings: Vec::new(),
            setting_defs: Vec::new(),
            inputs: HashMap::new(),
            outputs: Vec::new(),
            variables: Vec::new(),
        };
        only.inputs.insert(key, before.clone());
        own.rows.extend(OpenDjMapping::from_mixxx(&only).rows);
    }
    let option_names: Vec<&str> = heard.options.iter().map(String::as_str).collect();
    let mut row = Row::bind(heard.midi, control, &option_names);
    row.name = name.to_string();
    own.rows.push(row);
    let mut said: Vec<String> = Vec::new();
    for i in before {
        if format!("{},{}", i.group, i.key) == control {
            continue;
        }
        let what = if i.action.is_some() || i.options.has(Options::SCRIPT) {
            "what the mapping's script did".to_string()
        } else {
            let f = describe(&i.group, &i.key);
            if f.target.is_empty() {
                f.name
            } else {
                format!("{} · {}", f.name, f.target)
            }
        };
        if !said.contains(&what) {
            said.push(what);
        }
    }
    said
}

/// The controls `mapping` binds a message to directly (`[Channel1],play`),
/// for marking the ones already mapped.
pub fn bound_controls(mapping: &Mapping) -> HashSet<String> {
    mapping
        .inputs
        .values()
        .flatten()
        .filter(|i| i.action.is_none() && !i.options.has(Options::SCRIPT))
        .map(|i| format!("{},{}", i.group, i.key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hears_buttons_knobs_and_encoders() {
        let press = hear(&[vec![0x90, 0x0B, 0x7F], vec![0x80, 0x0B, 0]]).unwrap();
        assert_eq!(press.midi, Midi::new(0x90, 0x0B));
        assert!(press.options.is_empty());
        // A let-go alone starts nothing.
        assert!(hear(&[vec![0x90, 0x0B, 0]]).is_none());
        let knob: Vec<Vec<u8>> = (40..46).map(|v| vec![0xB0, 0x13, v]).collect();
        assert!(hear(&knob).unwrap().options.is_empty());
        let fourteen = [vec![0xB0, 0x13, 40], vec![0xB0, 0x33, 2]];
        assert_eq!(hear(&fourteen).unwrap().midi.lsb, Some((0xB0, 0x33)));
        let endless: Vec<Vec<u8>> = (0..5).map(|_| vec![0xB1, 0x10, 1]).collect();
        assert_eq!(hear(&endless).unwrap().options, ["diff"]);
        let rot: Vec<Vec<u8>> = (0..5).map(|_| vec![0xB1, 0x10, 65]).collect();
        assert_eq!(hear(&rot).unwrap().options, ["rot64"]);
        assert!(enough(&[vec![0x90, 1, 1]]));
        assert!(!enough(&[vec![0xB0, 1, 1]]));
    }

    #[test]
    fn learning_replaces_or_keeps_what_was_there() {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        let base = Mapping::load(&dir.join("Pioneer-DDJ-FLX4.midi.xml")).unwrap();
        let play = Heard {
            midi: Midi::new(0x90, 0x0B),
            options: Vec::new(),
        };
        let mut own = OpenDjMapping::new("Mine");
        let was = learn(
            &mut own,
            Some(&base),
            &play,
            "[Channel1],sync_enabled",
            "PLAY",
            false,
        );
        assert!(!was.is_empty());
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&(0x90, 0x0B)];
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].key, "sync_enabled");
        assert!(bound_controls(&built).contains("[Channel1],sync_enabled"));
        // Learning it again keeps one row.
        learn(
            &mut own,
            Some(&base),
            &play,
            "[Channel1],cue_default",
            "PLAY",
            false,
        );
        assert_eq!(own.rows.len(), 1);

        let mut both = OpenDjMapping::new("Both");
        learn(
            &mut both,
            Some(&base),
            &play,
            "[Channel1],sync_enabled",
            "PLAY",
            true,
        );
        let built = both.build(Some(&base), &dir);
        let inputs = &built.inputs[&(0x90, 0x0B)];
        assert_eq!(inputs.len(), base.inputs[&(0x90, 0x0B)].len() + 1);
    }
}
