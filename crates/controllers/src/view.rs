//! The Controller Editor's picture of a mapping: every mapped control as a
//! row saying what it does, grouped by function, plus what each LED shows
//! and the mapping's own settings. Built from the XML, the probe (for script
//! bindings) and, where there is one, the controller's layout.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use mixxx_control::{ConfigKey, ControlRegistry};

use crate::catalog::{describe, describe_value, humanize, Function};
use crate::layout::Layout;
use crate::mapping::{Input, Mapping, Options, SettingDef};
use crate::probe::{is_shift_binding, message_key, probe, Effect, MessageKey, Probe};

/// One thing a control does.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub function: Function,
    /// The hardware control: "PLAY/PAUSE", "PAD 2", "ENCODER 5".
    pub control: String,
    /// Its id in the layout, when there is a picture.
    pub part: Option<String>,
    /// "SHIFT", "Hot cue", "Bank 2 press"; empty for the plain press.
    pub layer: String,
    /// "button", "pad", "knob", "encoder", "fader", "jog" or "switch".
    pub kind: String,
    /// The messages it comes in on (two for a 14-bit fader).
    pub keys: Vec<MessageKey>,
    /// "Note 0B · ch 1", "CC 13, 33 · ch 1".
    pub midi: String,
    /// The script function behind it, for script bindings.
    pub script: Option<String>,
    /// What the mapping's author wrote about it.
    pub description: String,
    /// The binding's options in words: "Pick-up, 14-bit".
    pub options: String,
}

/// What an LED shows.
#[derive(Clone, Debug, PartialEq)]
pub struct LightRow {
    pub function: Function,
    pub control: String,
    pub part: Option<String>,
    pub key: MessageKey,
    pub midi: String,
}

pub struct MappingView {
    pub name: String,
    pub author: String,
    pub file: String,
    pub rows: Vec<Row>,
    pub lights: Vec<LightRow>,
    pub options: Vec<SettingDef>,
    pub layout: Option<Layout>,
    /// Problems running the mapping in the sandbox, if any.
    pub notes: Vec<String>,
}

/// "Note 0B · ch 1", "CC 13 · ch 7", "Pitch bend · ch 2".
pub fn message_text(status: u8, number: u8) -> String {
    let ch = (status & 0x0F) + 1;
    match status & 0xF0 {
        0x80 | 0x90 => format!("Note {number:02X} · ch {ch}"),
        0xB0 => format!("CC {number:02X} · ch {ch}"),
        0xE0 => format!("Pitch bend · ch {ch}"),
        0xA0 => format!("Aftertouch {number:02X} · ch {ch}"),
        0xC0 => format!("Program {number:02X} · ch {ch}"),
        _ => format!("{status:02X} {number:02X}"),
    }
}

/// "CC 13 · ch 1" and "CC 33 · ch 1" → "CC 13, 33 · ch 1"; otherwise both,
/// with " / " between.
fn join_messages(a: &str, b: &str) -> String {
    if let Some((msg, ch)) = b.rsplit_once(" · ") {
        if let Some((kind, num)) = msg.rsplit_once(' ') {
            let head = format!("{kind} ");
            let tail = format!(" · {ch}");
            if let Some(nums) = a.strip_prefix(&head).and_then(|r| r.strip_suffix(&tail)) {
                if !nums.contains(' ') || nums.contains(", ") {
                    return format!("{head}{nums}, {num}{tail}");
                }
            }
        }
    }
    format!("{a} / {b}")
}

fn options_text(o: Options) -> String {
    let words = [
        (Options::SCRIPT, "Script"),
        (Options::INVERT, "Invert"),
        (Options::ROT64, "Relative (64 = centre)"),
        (Options::ROT64_INV, "Relative, reversed"),
        (Options::ROT64_FAST, "Relative, fast"),
        (Options::DIFF, "Relative (two's complement)"),
        (Options::SELECT_KNOB, "Endless (select knob)"),
        (Options::BUTTON, "Button"),
        (Options::SWITCH, "Switch"),
        (Options::HERC_JOG, "Jog"),
        (Options::HERC_JOG_FAST, "Jog, fast"),
        (Options::SPREAD64, "Centred at 64"),
        (Options::SOFT_TAKEOVER, "Pick-up"),
        (Options::FOURTEEN_BIT_MSB, "14-bit"),
        (Options::FOURTEEN_BIT_LSB, "14-bit"),
    ];
    let mut out: Vec<&str> = Vec::new();
    for (flag, w) in words {
        if o.has(flag) && !out.contains(&w) {
            out.push(w);
        }
    }
    out.join(", ")
}

fn guess_kind(status: u8, o: Options) -> &'static str {
    match status & 0xF0 {
        0x80 | 0x90 => "button",
        0xE0 => "fader",
        _ if o.has(Options::SELECT_KNOB)
            || o.has(Options::ROT64)
            || o.has(Options::ROT64_INV)
            || o.has(Options::ROT64_FAST)
            || o.has(Options::DIFF) =>
        {
            "encoder"
        }
        _ if o.has(Options::HERC_JOG) || o.has(Options::HERC_JOG_FAST) => "jog",
        _ => "knob",
    }
}

/// "PLAY/PAUSE +SHIFT (DECK1) - press - Reverse…" → ("PLAY/PAUSE (DECK1)",
/// "Reverse…", true).
fn split_description(d: &str) -> (String, String, bool) {
    let parts: Vec<&str> = d.split(" - ").map(str::trim).collect();
    let head = parts.first().copied().unwrap_or("");
    let shift = head.to_ascii_lowercase().contains("shift");
    let name = head
        .replace("+SHIFT", "")
        .replace("+ SHIFT", "")
        .replace("+ shift", "")
        .replace("SHIFT +", "")
        .replace("Shift +", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let what = if parts.len() > 1 {
        parts.last().copied().unwrap_or("").to_string()
    } else {
        String::new()
    };
    (name, what, shift)
}

/// The part of a script binding that names the control or what it does:
/// `XoneK2.controllers[0xE].columns[1].knobs[2].input` → "Columns 1 knobs 2";
/// `MidiFighterTwister.controller.toggleDeck` → "Toggle deck".
fn script_name(key: &str) -> String {
    let mut parts: Vec<&str> = key.split('.').collect();
    if parts.last() == Some(&"input") {
        parts.pop();
    }
    let tail: Vec<String> = parts
        .iter()
        .skip(1)
        .filter(|p| !p.starts_with("controller"))
        .map(|p| p.replace(['[', ']', '"'], " "))
        .collect();
    let last = tail.last().cloned().unwrap_or_else(|| key.to_string());
    humanize(last.trim())
}

/// Natural order: "Hot cue 2" before "Hot cue 10".
fn natural(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut v: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match v.last_mut() {
                Some((ld, t)) if *ld == d => t.push(c),
                _ => v.push((d, c.to_string())),
            }
        }
        v
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for ((da, ta), (db, tb)) in ca.iter().zip(cb.iter()) {
        let o = if *da && *db {
            ta.parse::<u64>()
                .unwrap_or(0)
                .cmp(&tb.parse::<u64>().unwrap_or(0))
        } else {
            ta.to_lowercase().cmp(&tb.to_lowercase())
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len())
}

/// Controls a script touches that say something to a DJ (the probe has
/// already dropped ones scripts make up for their own state).
fn meaningful(k: &ConfigKey) -> bool {
    !k.item.ends_with("_indicator") && !k.item.contains("VuMeter")
}

fn functions(touched: &[Effect]) -> Vec<Function> {
    let mut out: Vec<Function> = Vec::new();
    for (k, v) in touched {
        if !meaningful(k) {
            continue;
        }
        let f = describe_value(&k.group, &k.item, *v);
        if !out.contains(&f) {
            out.push(f);
        }
    }
    // Plumbing next to a real function (a mode flag the script also set, the
    // size it set before jumping) goes; it stays when it's all there is.
    if out.iter().any(|f| f.category != "Controller") {
        out.retain(|f| f.category != "Controller");
    }
    let setting = |f: &Function| f.name.contains(" size");
    if out.iter().any(|f| !setting(f)) {
        out.retain(|f| !setting(f));
    }
    out
}

struct Builder<'a> {
    mapping: &'a Mapping,
    layout: Option<&'a Layout>,
    probe: &'a Probe,
    rows: Vec<Row>,
}

impl Builder<'_> {
    fn add(&mut self, status: u8, number: u8, input: &Input) {
        let key = message_key(status, number);
        let (desc_control, desc_what, desc_shift) = split_description(&input.description);
        let found = self.layout.and_then(|l| l.find(status, number));
        let script = input
            .options
            .has(Options::SCRIPT)
            .then(|| input.key.clone());
        let control = match (found, &script) {
            (Some((p, _)), _) => p.label.clone(),
            _ if !desc_control.is_empty() => desc_control.clone(),
            (None, Some(s)) => script_name(s),
            _ => message_text(status, number),
        };
        let base_layer = match found {
            Some((_, m)) => m.layer.clone(),
            None if desc_shift => "SHIFT".into(),
            None => String::new(),
        };
        let kind = match found {
            Some((p, _)) => p.kind.clone(),
            None => guess_kind(status, input.options).into(),
        };
        let make = |function: Function, layer: String| Row {
            function,
            control: control.clone(),
            part: found.map(|(p, _)| p.id.clone()),
            layer,
            kind: kind.clone(),
            keys: vec![key],
            midi: message_text(status, number),
            script: script.clone(),
            description: input.description.clone(),
            options: options_text(input.options),
        };
        let fallback = |what: &str| {
            let target = describe(&input.group, "").target;
            let name = if is_shift_binding(&input.key) {
                "SHIFT".to_string()
            } else if !what.is_empty() {
                what.to_string()
            } else if let Some(s) = &script {
                script_name(s)
            } else {
                humanize(&input.key)
            };
            Function::new("Controller", name, target)
        };
        let mut new_rows = Vec::new();
        if script.is_none() {
            new_rows.push(make(describe(&input.group, &input.key), base_layer));
        } else {
            let normal = functions(self.probe.normal.get(&key).map_or(&[][..], |v| v));
            // A SHIFT button is just that, whatever its script resets.
            let shift = is_shift_binding(&input.key);
            if shift || normal.is_empty() {
                new_rows.push(make(fallback(&desc_what), base_layer.clone()));
            }
            for f in normal.into_iter().filter(|_| !shift) {
                new_rows.push(make(f, base_layer.clone()));
            }
            if let Some(shifted) = self.probe.shifted.get(&key).filter(|_| !shift) {
                let layer = if base_layer.is_empty() {
                    "SHIFT".to_string()
                } else {
                    format!("{base_layer} + SHIFT")
                };
                for f in functions(shifted) {
                    if !new_rows.iter().any(|r| r.function == f) {
                        new_rows.push(make(f, layer.clone()));
                    }
                }
            }
        }
        for r in new_rows {
            // The same function on the same control and layer from another
            // message (the other half of a 14-bit pair, the same button on
            // another channel) joins that row.
            match self.rows.iter_mut().find(|o| {
                o.function == r.function
                    && o.layer == r.layer
                    && o.control == r.control
                    && o.part == r.part
            }) {
                Some(o) if !o.keys.contains(&key) => {
                    o.keys.push(key);
                    o.midi = join_messages(&o.midi, &r.midi);
                }
                Some(_) => {}
                None => self.rows.push(r),
            }
        }
    }
}

/// Reads the mapping at `path` and works out what it does. `registry` must
/// hold the engine's controls and be used for nothing else (see
/// [`crate::probe::probe`]).
pub fn build(path: &Path, registry: Arc<ControlRegistry>) -> Result<MappingView, String> {
    let mapping = Mapping::load(path)?;
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let layout = Layout::for_file(&file);
    let mut notes = Vec::new();
    let probed = match probe(&mapping, registry.clone()) {
        Ok(p) => p,
        Err(e) => {
            notes.push(format!("The mapping's script didn't start: {e}"));
            Probe::default()
        }
    };
    notes.extend(probed.errors.iter().take(3).cloned());

    let mut messages: Vec<(u8, u8)> = mapping.inputs.keys().copied().collect();
    messages.sort();
    let mut b = Builder {
        mapping: &mapping,
        layout: layout.as_ref(),
        probe: &probed,

        rows: Vec::new(),
    };
    for (s, n) in messages {
        if layout.as_ref().is_some_and(|l| !l.shows_channel(s)) {
            continue;
        }
        if s & 0xF0 == 0x80 && b.mapping.inputs.contains_key(&(0x90 | (s & 0x0F), n)) {
            continue;
        }
        for input in b.mapping.inputs[&(s, n)].clone() {
            b.add(s, n, &input);
        }
    }
    let mut rows = b.rows;
    rows.sort_by(|a, b| {
        a.function
            .order()
            .cmp(&b.function.order())
            .then_with(|| natural(&a.function.target, &b.function.target))
            .then_with(|| natural(&a.function.name, &b.function.name))
            .then_with(|| a.layer.cmp(&b.layer))
            .then_with(|| a.keys.cmp(&b.keys))
    });

    let mut lights: Vec<LightRow> = Vec::new();
    let mut seen: HashSet<(String, MessageKey)> = HashSet::new();
    let mut add_light = |k: &ConfigKey, status: u8, number: u8| {
        if layout.as_ref().is_some_and(|l| !l.shows_channel(status)) {
            return;
        }
        let f = describe(&k.group, &k.item);
        let key = message_key(status, number);
        if !seen.insert((format!("{}{}{}", f.category, f.name, f.target), key)) {
            return;
        }
        let found = layout.as_ref().and_then(|l| l.find(status, number));
        lights.push(LightRow {
            control: found.map_or_else(|| message_text(status, number), |(p, _)| p.label.clone()),
            part: found.map(|(p, _)| p.id.clone()),
            function: f,
            key,
            midi: message_text(status, number),
        });
    };
    for (k, msgs) in &probed.lights {
        for &(s, n) in msgs {
            add_light(k, s, n);
        }
    }
    for o in &mapping.outputs {
        add_light(
            &ConfigKey::new(o.group.clone(), o.key.clone()),
            o.status,
            o.control,
        );
    }
    lights.sort_by(|a, b| {
        a.function
            .order()
            .cmp(&b.function.order())
            .then_with(|| natural(&a.function.target, &b.function.target))
            .then_with(|| natural(&a.function.name, &b.function.name))
            .then_with(|| a.key.cmp(&b.key))
    });

    Ok(MappingView {
        name: mapping.name.clone(),
        author: mapping.author.clone(),
        file,
        rows,
        lights,
        options: mapping.setting_defs.clone(),
        layout,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_descriptions() {
        assert_eq!(
            split_description("PLAY/PAUSE +SHIFT (DECK1) - press - Reverse playback"),
            ("PLAY/PAUSE (DECK1)".into(), "Reverse playback".into(), true)
        );
        assert_eq!(
            split_description("Sets the decks pregain."),
            ("Sets the decks pregain.".into(), String::new(), false)
        );
    }

    #[test]
    fn names_script_bindings() {
        assert_eq!(
            script_name("MidiFighterTwister.controller.toggleDeck"),
            "Toggle deck"
        );
        assert_eq!(
            script_name("PioneerDDJFLX4.cueLoopCallLeft"),
            "Cue loop call left"
        );
    }

    #[test]
    fn joins_messages() {
        assert_eq!(
            join_messages("CC 13 · ch 1", "CC 33 · ch 1"),
            "CC 13, 33 · ch 1"
        );
        assert_eq!(
            join_messages("Note 47 · ch 5", "Note 47 · ch 6"),
            "Note 47 · ch 5 / Note 47 · ch 6"
        );
    }

    #[test]
    fn sorts_naturally() {
        assert_eq!(natural("Hot cue 2", "Hot cue 10"), Ordering::Less);
        assert_eq!(natural("Deck 1", "Deck 1"), Ordering::Equal);
    }
}
