//! Changing a row of the DJ's own mapping from the Controller Editor: what a
//! control does, when it acts and how its value is read. A change is a row
//! of the own mapping's file (see [`crate::format`]) put over the Mixxx
//! mapping it's based on; going back to the original takes the rows away.

use opendj_control::{Behavior, ControlRegistry};

use crate::catalog::{describe, Function};
use crate::format::{Action, Midi, OpenDjMapping, Row};
use crate::mapping::{Options, When};
use crate::view;

/// What a row is set to: its function, when it acts and how the value is
/// read. Read from a row with [`Binding::of`] and written with [`set`].
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// `[Channel1],play`; `None` when the control does nothing.
    pub control: Option<String>,
    pub when: When,
    pub buttons: Buttons,
    pub encoder: Encoder,
    pub invert: bool,
    /// Soft takeover: a knob or fader only takes over once it passes the
    /// value on screen.
    pub pickup: bool,
}

/// How a button's press is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Buttons {
    /// As the function does it: hold for momentary ones, a press toggles
    /// on/off ones.
    #[default]
    Normal,
    /// Any press is on, let go is off (Mixxx's `button`).
    Hold,
    /// Every message is a press (Mixxx's `switch`): for controls that send
    /// one message per press.
    Trigger,
}

/// How a knob's value is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoder {
    /// The value is where the knob is (0 to 127).
    #[default]
    Absolute,
    /// An endless encoder sending 64 ± steps.
    Relative,
    /// The same, turning the other way.
    RelativeReversed,
    /// An endless encoder sending steps as two's complement (1 up, 127 down).
    TwosComplement,
    /// An endless browse knob: each step is one move.
    Steps,
}

impl Buttons {
    pub const ALL: [Buttons; 3] = [Buttons::Normal, Buttons::Hold, Buttons::Trigger];

    pub fn label(self) -> &'static str {
        match self {
            Buttons::Normal => "As the function does it",
            Buttons::Hold => "On while held",
            Buttons::Trigger => "Every message is a press",
        }
    }

    fn option(self) -> Option<&'static str> {
        match self {
            Buttons::Normal => None,
            Buttons::Hold => Some("button"),
            Buttons::Trigger => Some("switch"),
        }
    }
}

impl Encoder {
    pub const ALL: [Encoder; 5] = [
        Encoder::Absolute,
        Encoder::Relative,
        Encoder::RelativeReversed,
        Encoder::TwosComplement,
        Encoder::Steps,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Encoder::Absolute => "Absolute (knob or fader)",
            Encoder::Relative => "Endless, 64 is still",
            Encoder::RelativeReversed => "Endless, 64 is still, reversed",
            Encoder::TwosComplement => "Endless, 1 up and 127 down",
            Encoder::Steps => "Endless, one step per click",
        }
    }

    fn option(self) -> Option<&'static str> {
        match self {
            Encoder::Absolute => None,
            Encoder::Relative => Some("rot64"),
            Encoder::RelativeReversed => Some("rot64inv"),
            Encoder::TwosComplement => Some("diff"),
            Encoder::Steps => Some("selectknob"),
        }
    }
}

/// When a row acts, as the editor lists them.
pub const WHENS: [When; 5] = [
    When::Press,
    When::Release,
    When::Long,
    When::Tap,
    When::Double,
];

pub fn when_label(w: When) -> &'static str {
    match w {
        When::Press => "Press (or move)",
        When::Release => "Release",
        When::Long => "Long press",
        When::Tap => "Tap (let go quickly)",
        When::Double => "Double press",
    }
}

impl Binding {
    /// What `row` is set to now.
    pub fn of(row: &view::Row) -> Self {
        let Some(s) = row.sources.first() else {
            return Binding::nothing(When::Press);
        };
        let o = s.options;
        Binding {
            control: (s.is_control() && !s.key.is_empty())
                .then(|| format!("{},{}", s.group, s.key)),
            when: s.when,
            buttons: if o.has(Options::BUTTON) {
                Buttons::Hold
            } else if o.has(Options::SWITCH) {
                Buttons::Trigger
            } else {
                Buttons::Normal
            },
            encoder: if o.has(Options::ROT64) {
                Encoder::Relative
            } else if o.has(Options::ROT64_INV) {
                Encoder::RelativeReversed
            } else if o.has(Options::DIFF) {
                Encoder::TwosComplement
            } else if o.has(Options::SELECT_KNOB) {
                Encoder::Steps
            } else {
                Encoder::Absolute
            },
            invert: o.has(Options::INVERT),
            pickup: o.has(Options::SOFT_TAKEOVER),
        }
    }

    fn nothing(when: When) -> Self {
        Binding {
            control: None,
            when,
            buttons: Buttons::Normal,
            encoder: Encoder::Absolute,
            invert: false,
            pickup: false,
        }
    }

    fn options(&self) -> Vec<String> {
        let mut out: Vec<String> = [self.buttons.option(), self.encoder.option()]
            .into_iter()
            .flatten()
            .map(String::from)
            .collect();
        if self.invert {
            out.push("invert".into());
        }
        out
    }
}

/// `[("shift", None), ("m1", Some(2))]` → `"shift & m1=2"`.
pub fn layer_text(layer: &[(String, Option<f64>)]) -> String {
    layer
        .iter()
        .map(|(n, v)| match v {
            Some(v) => format!("{n}={v}"),
            None => n.clone(),
        })
        .collect::<Vec<_>>()
        .join(" & ")
}

/// The message a row's first binding comes in on, with the other half of a
/// 14-bit pair.
fn midi_of(row: &view::Row) -> Option<Midi> {
    let first = row.sources.first()?;
    let mut midi = Midi::new(first.status, first.number);
    if first.options.has(Options::FOURTEEN_BIT_MSB) {
        midi.lsb = row
            .sources
            .iter()
            .find(|s| s.options.has(Options::FOURTEEN_BIT_LSB))
            .map(|s| (s.status, s.number));
    }
    Some(midi)
}

/// The own mapping's rows for the message and layer of `row`.
fn own_rows<'a>(
    own: &'a OpenDjMapping,
    row: &'a view::Row,
) -> impl Iterator<Item = (usize, &'a Row)> + 'a {
    own.rows.iter().enumerate().filter(move |(_, r)| {
        !r.off
            && row.sources.iter().any(|s| {
                (r.midi.status, r.midi.number) == (s.status, s.number)
                    && r.layer_conditions() == s.layer
            })
    })
}

/// Whether the own mapping changes `row`.
pub fn is_edited(own: &OpenDjMapping, row: &view::Row) -> bool {
    own_rows(own, row).next().is_some()
}

/// Sets what `row` does in the own mapping. The row's message then does only
/// this on its layer, whatever the base mapping or its script did there
/// (SHIFT functions a script gave the same button go too).
pub fn set(own: &mut OpenDjMapping, row: &view::Row, b: &Binding) {
    let (Some(midi), Some(first)) = (midi_of(row), row.sources.first()) else {
        return;
    };
    let layer = layer_text(&first.layer);
    let same = |r: &Row, when: When| {
        (r.midi.status, r.midi.number) == (midi.status, midi.number)
            && r.layer == layer
            && r.when == when
            && !r.off
    };
    // This file's row for the binding shown goes; its stand-in for the
    // base's binding at another time stays.
    own.rows
        .retain(|r| !(same(r, first.when) && !r.remove) && !(same(r, b.when) && r.remove));
    if b.when != first.when && !own.rows.iter().any(|r| same(r, first.when)) {
        own.rows.push(Row {
            layer: layer.clone(),
            when: first.when,
            remove: true,
            ..Row::new(midi, Action::Nothing)
        });
    }
    let mut new = match &b.control {
        Some(c) => Row::new(
            midi,
            Action::Bind {
                control: c.clone(),
                options: b.options(),
            },
        ),
        None => Row {
            remove: true,
            ..Row::new(midi, Action::Nothing)
        },
    };
    new.layer = layer;
    new.when = b.when;
    new.name = row.control.clone();
    if b.pickup && b.control.is_some() {
        new.takeover = Some("pickup".into());
    }
    own.rows.push(new);
}

/// Puts `row` back as the base mapping has it.
pub fn reset(own: &mut OpenDjMapping, row: &view::Row) {
    let gone: Vec<usize> = own_rows(own, row).map(|(i, _)| i).collect();
    let mut i = 0;
    own.rows.retain(|_| {
        let keep = !gone.contains(&i);
        i += 1;
        keep
    });
}

/// A function a control can be given, on each target that has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub category: &'static str,
    pub name: String,
    /// The targets ("Deck 1", "FX 2") with the control for each.
    pub targets: Vec<(String, String)>,
    /// A knob or fader function (a level, a position) rather than a button.
    pub continuous: bool,
}

/// Every function there is to choose from, by category in the editor's
/// order: the engine's controls in `registry`, by their plain names.
pub fn choices(registry: &ControlRegistry) -> Vec<Choice> {
    let mut out: Vec<Choice> = Vec::new();
    for k in registry.keys() {
        let Some(c) = registry.get(&k.group, &k.item) else {
            continue;
        };
        let behavior = c.behavior();
        if matches!(behavior, Behavior::Indicator | Behavior::Free)
            || k.item.ends_with("_indicator")
            || k.item.contains("VuMeter")
            || k.group.starts_with("[PreviewDeck")
        {
            continue;
        }
        let f: Function = describe(&k.group, &k.item);
        let control = format!("{},{}", k.group, k.item);
        match out
            .iter_mut()
            .find(|c| c.category == f.category && c.name == f.name)
        {
            Some(c) => {
                if !c.targets.iter().any(|(t, _)| *t == f.target) {
                    c.targets.push((f.target, control));
                }
            }
            None => out.push(Choice {
                category: f.category,
                continuous: matches!(behavior, Behavior::Linear | Behavior::CenteredOnDefault),
                targets: vec![(f.target.clone(), control)],
                name: f.name.clone(),
            }),
        }
    }
    let order = |c: &Choice| Function::new(c.category, "", "").order();
    out.sort_by(|a, b| {
        order(a)
            .cmp(&order(b))
            .then_with(|| natural(&a.name, &b.name))
    });
    for c in &mut out {
        c.targets.sort_by(|a, b| natural(&a.0, &b.0));
    }
    out
}

/// "Hot cue 2" before "Hot cue 10".
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let digits: String = s
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let head: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
        (
            head.to_lowercase(),
            digits.parse::<u64>().unwrap_or(0),
            s.to_lowercase(),
        )
    };
    key(a).cmp(&key(b))
}

/// A name for the DJ's first own copy of `name`: "My Pioneer DDJ-FLX4", or
/// with a number when that's taken.
pub fn my_name(name: &str, taken: &[String]) -> String {
    let base = format!("My {name}");
    (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} {n}")
            }
        })
        .find(|n| !taken.contains(n))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::Mapping;

    fn flx4() -> (Mapping, std::path::PathBuf) {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        let m = Mapping::load(&dir.join("Pioneer-DDJ-FLX4.midi.xml")).unwrap();
        (m, dir)
    }

    fn view_row(m: &Mapping, key: (u8, u8)) -> view::Row {
        let input = &m.inputs[&key][0];
        view::Row {
            function: describe(&input.group, &input.key),
            control: "PLAY".into(),
            part: None,
            layer: String::new(),
            kind: "button".into(),
            keys: vec![crate::probe::message_key(key.0, key.1)],
            midi: String::new(),
            script: None,
            description: String::new(),
            options: String::new(),
            sources: vec![view::Source {
                status: key.0,
                number: key.1,
                group: input.group.clone(),
                key: input.key.clone(),
                options: input.options,
                action: None,
                layer: Vec::new(),
                when: When::Press,
            }],
            edited: false,
            off: false,
            note: String::new(),
        }
    }

    #[test]
    fn a_change_goes_over_the_base_and_resets() {
        let (base, dir) = flx4();
        let key = (0x90, 0x0B);
        let row = view_row(&base, key);
        let mut own = OpenDjMapping::new("Mine");
        own.based_on = Some("Pioneer-DDJ-FLX4.midi.xml".into());
        let mut b = Binding::of(&row);
        b.control = Some("[Channel1],sync_enabled".into());
        b.buttons = Buttons::Hold;
        set(&mut own, &row, &b);
        assert!(is_edited(&own, &row));
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&key];
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].key, "sync_enabled");
        assert!(inputs[0].options.has(Options::BUTTON));
        // Setting it again replaces the change rather than adding one.
        b.control = Some("[Channel1],cue_default".into());
        set(&mut own, &row, &b);
        assert_eq!(own.rows.len(), 1);
        reset(&mut own, &row);
        assert!(own.rows.is_empty());
        assert!(!is_edited(&own, &row));
    }

    #[test]
    fn acting_at_another_time_takes_the_press_away() {
        let (base, dir) = flx4();
        let key = (0x90, 0x0B);
        let row = view_row(&base, key);
        let mut own = OpenDjMapping::new("Mine");
        let mut b = Binding::of(&row);
        b.when = When::Long;
        set(&mut own, &row, &b);
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&key];
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].when, When::Long);
        // Doing nothing is a row that only takes the base's away.
        let mut none = OpenDjMapping::new("Mine");
        set(&mut none, &row, &Binding::nothing(When::Press));
        assert!(!none.build(Some(&base), &dir).inputs.contains_key(&key));
    }

    #[test]
    fn functions_to_choose_from() {
        let registry = ControlRegistry::new();
        registry.toggle("[Channel1]", "play");
        registry.toggle("[Channel2]", "play");
        registry.create("[Channel1]", "volume", 1.0, 0.0, 1.0, Behavior::Linear);
        registry.indicator("[Channel1]", "play_indicator", 1.0);
        let c = choices(&registry);
        let play = c
            .iter()
            .find(|c| c.targets[0].1 == "[Channel1],play")
            .unwrap();
        assert_eq!(play.targets.len(), 2);
        assert!(!play.continuous);
        assert!(c.iter().any(|c| c.continuous));
        assert!(!c
            .iter()
            .any(|c| c.targets.iter().any(|t| t.1.contains("indicator"))));
        assert_eq!(my_name("FLX4", &["My FLX4".into()]), "My FLX4 2");
        assert_eq!(
            layer_text(&[("shift".into(), None), ("m1".into(), Some(2.0))]),
            "shift & m1=2"
        );
    }
}
