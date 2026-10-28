//! Every verb openDJscript knows, for the parser, the runner and editors.

/// What a verb does, which says how it runs and what it answers in a
/// condition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Starts the deck; asks whether it plays.
    Play,
    /// Stops the deck; asks whether it's stopped.
    Pause,
    /// Flips an on/off control of the deck; asks whether it's on.
    Toggle(&'static str),
    /// Presses a button of the deck; can't be asked.
    Press(&'static str),
    /// Turns on a beat loop of the size given, or off when that size is on;
    /// asks whether any loop is on.
    Loop,
    /// Turns the loop off; asks whether one is on.
    LoopExit,
    /// Presses a hot cue; asks whether it's set.
    HotCue,
    /// Sets a 0..1 control of the deck to a level; can't be asked.
    Level(&'static str),
    /// `set 'control' value`; can't be asked.
    Set,
    /// `toggle 'control'`; asks whether the control is on.
    ToggleNamed,
    /// `press 'control'`; can't be asked.
    PressNamed,
    /// `get 'control'`: only asks, whether the control is above 0.
    Get,
    /// Done by the app, not a control: see [`crate::Host`].
    App,
}

/// Whether a verb acts on a deck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TakesDeck {
    No,
    /// The deck in front of it, else the master deck.
    Yes,
    /// Only the deck in front of it; with none the verb acts elsewhere
    /// (`rating` rates the tracks picked in the browser).
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgKind {
    /// A plain number in this range.
    Number { min: f64, max: f64 },
    /// 0 to 1, or 0% to 100%.
    Level,
    /// A control's name: `'keylock'` on the deck, or `'[Master],gain'`.
    Control,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    /// What a left-out argument counts as; `None` means it's needed.
    pub default: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub struct Verb {
    pub name: &'static str,
    pub kind: Kind,
    pub deck: TakesDeck,
    pub args: &'static [ArgSpec],
    /// One line for editors.
    pub help: &'static str,
}

const fn def(
    name: &'static str,
    kind: Kind,
    deck: TakesDeck,
    args: &'static [ArgSpec],
    help: &'static str,
) -> Verb {
    Verb {
        name,
        kind,
        deck,
        args,
        help,
    }
}

const CONTROL: ArgSpec = ArgSpec {
    name: "control",
    kind: ArgKind::Control,
    default: None,
};

use Kind::*;
use TakesDeck::{Optional, Yes};

pub static VERBS: &[Verb] = &[
    def("play", Play, Yes, &[], "Start the deck"),
    def("pause", Pause, Yes, &[], "Stop the deck"),
    def(
        "play_pause",
        Toggle("play"),
        Yes,
        &[],
        "Start or stop the deck",
    ),
    def(
        "goto_start",
        Press("start"),
        Yes,
        &[],
        "Jump to the start of the track",
    ),
    def(
        "sync",
        Toggle("sync_enabled"),
        Yes,
        &[],
        "Turn sync on or off",
    ),
    def(
        "key_lock",
        Toggle("keylock"),
        Yes,
        &[],
        "Turn key lock on or off",
    ),
    def(
        "quantize",
        Toggle("quantize"),
        Yes,
        &[],
        "Turn quantize on or off",
    ),
    def(
        "slip",
        Toggle("slip_enabled"),
        Yes,
        &[],
        "Turn slip mode on or off",
    ),
    def(
        "loop",
        Loop,
        Yes,
        &[ArgSpec {
            name: "beats",
            kind: ArgKind::Number {
                min: 0.03125,
                max: 512.0,
            },
            default: Some(4.0),
        }],
        "Loop this many beats, or end that loop",
    ),
    def("loop_exit", LoopExit, Yes, &[], "End the loop"),
    def(
        "hot_cue",
        HotCue,
        Yes,
        &[ArgSpec {
            name: "number",
            kind: ArgKind::Number {
                min: 1.0,
                max: 36.0,
            },
            default: None,
        }],
        "Play from a hot cue, or set it",
    ),
    def("eject", Press("eject"), Yes, &[], "Unload the deck"),
    def(
        "volume",
        Level("volume"),
        Yes,
        &[ArgSpec {
            name: "level",
            kind: ArgKind::Level,
            default: None,
        }],
        "Set the channel fader",
    ),
    def(
        "crossfader",
        Toggle("[Mixer],crossfader_enabled"),
        TakesDeck::No,
        &[],
        "Turn the crossfader on or off; off, every deck plays as if set to thru",
    ),
    def(
        "rating",
        App,
        Optional,
        &[ArgSpec {
            name: "number of stars",
            kind: ArgKind::Number { min: 0.0, max: 5.0 },
            default: None,
        }],
        "Rate the tracks picked in the browser, or the deck's track; 0 clears",
    ),
    def(
        "set",
        Set,
        Yes,
        &[
            CONTROL,
            ArgSpec {
                name: "value",
                kind: ArgKind::Number {
                    min: f64::MIN,
                    max: f64::MAX,
                },
                default: None,
            },
        ],
        "Set any control to a value",
    ),
    def(
        "toggle",
        ToggleNamed,
        Yes,
        &[CONTROL],
        "Turn any on/off control on or off",
    ),
    def(
        "press",
        PressNamed,
        Yes,
        &[CONTROL],
        "Press any button control",
    ),
    def(
        "get",
        Get,
        Yes,
        &[CONTROL],
        "In a condition: whether a control is above 0",
    ),
];

/// The verb with this name.
pub fn verb(name: &str) -> Option<&'static Verb> {
    VERBS.iter().find(|v| v.name == name)
}
