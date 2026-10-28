//! openDJscript: short action lines in the style of VirtualDJ's VDJscript.
//!
//! ```text
//! rating 3                          rate the tracks picked in the browser
//! deck 1 play_pause                 a verb on one deck
//! sync & play                       a chain runs left to right
//! loop_exit ? loop_exit : loop 4    condition ? when true : when false
//! deck 2 (key_lock & quantize)      parentheses group, and pass the deck in
//! toggle 'slip_enabled'             any control by its Mixxx name
//! ```
//!
//! A verb on its own targets the master deck. In a condition a verb is
//! asked rather than done: `play ? …` asks whether the deck is playing.
//! A chain runs every action even when one fails, and reports the first
//! error. Parsing checks every verb and its arguments, so an editor can
//! show a mistake as the DJ types; nothing runs until the whole line parses.
//!
//! The verbs live in [`VERBS`]. The ones backed by a control run here
//! ([`run_control`], [`query_control`]); the rest, such as `rating`, need
//! the app and go through its [`Host`].

mod controls;
mod parse;
mod verbs;

pub use controls::{query_control, run_control};
pub use parse::{parse, Arg, Deck, Expr, Script};
pub use verbs::{verb, ArgKind, ArgSpec, Kind, TakesDeck, Verb, VERBS};

use std::fmt;

/// A mistake in a line, or an action that couldn't be done.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub message: String,
    /// Where in the line, in bytes, when it's a mistake in the text.
    pub at: Option<usize>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            at: None,
        }
    }

    fn at(message: impl Into<String>, at: usize) -> Self {
        Self {
            message: message.into(),
            at: Some(at),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// One verb to do or ask, with its deck worked out (1 to 4) when the verb
/// takes one.
#[derive(Debug)]
pub struct Call<'a> {
    pub verb: &'static Verb,
    pub deck: Option<u8>,
    pub args: &'a [Arg],
}

/// What runs the verbs: the app, or a test.
pub trait Host {
    /// The deck a verb with no `deck` in front of it acts on, 1 to 4.
    fn master_deck(&self) -> u8;
    fn run(&mut self, call: &Call) -> Result<(), Error>;
    fn query(&mut self, call: &Call) -> Result<bool, Error>;
}

/// Parses and runs a line.
pub fn run(src: &str, host: &mut dyn Host) -> Result<(), Error> {
    parse(src)?.run(host)
}
