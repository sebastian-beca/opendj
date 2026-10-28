use mixxx_control::{Behavior, ControlRegistry};
use opendj_script::{parse, query_control, run, run_control, Call, Error, Host, Kind};

/// Runs control verbs on a registry and records the app's ones.
struct Test {
    reg: std::sync::Arc<ControlRegistry>,
    master: u8,
    ratings: Vec<(Option<u8>, f64)>,
}

impl Test {
    fn new() -> Self {
        let reg = ControlRegistry::new();
        for d in 1..=4 {
            let g = format!("[Channel{d}]");
            for item in ["play", "sync_enabled", "keylock", "loop_enabled"] {
                reg.toggle(&g, item);
            }
            reg.push(&g, "beatloop_4_toggle");
            reg.push(&g, "reloop_toggle");
            reg.create(&g, "volume", 1.0, 0.0, 1.0, Behavior::Linear);
        }
        reg.create("[Master]", "gain", 1.0, 0.0, 5.0, Behavior::Linear);
        Self {
            reg,
            master: 1,
            ratings: Vec::new(),
        }
    }

    fn get(&self, group: &str, item: &str) -> f64 {
        self.reg.get(group, item).unwrap().get()
    }
}

impl Host for Test {
    fn master_deck(&self) -> u8 {
        self.master
    }

    fn run(&mut self, call: &Call) -> Result<(), Error> {
        if call.verb.kind == Kind::App {
            self.ratings.push((call.deck, call.args[0].number()));
            return Ok(());
        }
        run_control(&self.reg, call)
    }

    fn query(&mut self, call: &Call) -> Result<bool, Error> {
        query_control(&self.reg, call)
    }
}

#[test]
fn rating_goes_to_the_browser_unless_a_deck_is_named() {
    let mut t = Test::new();
    run("rating 3", &mut t).unwrap();
    run("deck 2 rating 5", &mut t).unwrap();
    assert_eq!(t.ratings, vec![(None, 3.0), (Some(2), 5.0)]);
}

#[test]
fn a_verb_alone_acts_on_the_master_deck() {
    let mut t = Test::new();
    t.master = 2;
    run("play", &mut t).unwrap();
    assert_eq!(t.get("[Channel1]", "play"), 0.0);
    assert_eq!(t.get("[Channel2]", "play"), 1.0);
    run("deck master pause & deck 3 play", &mut t).unwrap();
    assert_eq!(t.get("[Channel2]", "play"), 0.0);
    assert_eq!(t.get("[Channel3]", "play"), 1.0);
}

#[test]
fn conditions_ask_and_the_deck_carries_into_the_branches() {
    let mut t = Test::new();
    for _ in 0..2 {
        run("deck 1 play ? pause : play", &mut t).unwrap();
        assert_eq!(t.get("[Channel1]", "play"), 1.0);
        run("deck 1 play ? pause : play", &mut t).unwrap();
        assert_eq!(t.get("[Channel1]", "play"), 0.0);
    }
    // The otherwise branch runs to the end of the line.
    run("play ? pause : sync & key_lock", &mut t).unwrap();
    assert_eq!(t.get("[Channel1]", "sync_enabled"), 1.0);
    assert_eq!(t.get("[Channel1]", "keylock"), 1.0);
    // A group in parentheses is asked as a whole.
    run("(sync & key_lock) ? play : pause", &mut t).unwrap();
    assert_eq!(t.get("[Channel1]", "play"), 1.0);
}

#[test]
fn levels_and_any_control() {
    let mut t = Test::new();
    run("deck 2 volume 25%", &mut t).unwrap();
    assert_eq!(t.get("[Channel2]", "volume"), 0.25);
    run("set '[Master],gain' 2 & deck 4 toggle keylock", &mut t).unwrap();
    assert_eq!(t.get("[Master]", "gain"), 2.0);
    assert_eq!(t.get("[Channel4]", "keylock"), 1.0);
    run("get 'keylock' ? play : pause", &mut t).unwrap();
    assert_eq!(t.get("[Channel1]", "play"), 0.0);
}

#[test]
fn a_chain_runs_on_after_a_failure_and_reports_it() {
    let mut t = Test::new();
    let err = run("loop 8 & play", &mut t).unwrap_err();
    assert_eq!(err.message, "There's no 8-beat loop");
    assert_eq!(t.get("[Channel1]", "play"), 1.0);
}

#[test]
fn mistakes_are_caught_before_anything_runs() {
    let cases = [
        ("", "Nothing to do", 0),
        ("rating", "'rating' needs a number of stars", 6),
        (
            "rating 6",
            "The number of stars for 'rating' goes from 0 to 5",
            7,
        ),
        (
            "rating 2 3",
            "'rating' takes no more than that; 3 is extra",
            9,
        ),
        (
            "playpause",
            "No verb 'playpause'; did you mean 'play_pause'?",
            0,
        ),
        (
            "deck 5 play",
            "Expected 1 to 4 or 'master' after 'deck', found 5",
            5,
        ),
        ("play ? pause", "Expected ':' and what to do otherwise", 12),
        ("volume 120%", "A level goes from 0 to 1, or 0% to 100%", 7),
        ("(play & sync", "Expected ')'", 12),
        ("play & ", "Expected a verb", 7),
        ("set 'x", "This quote is never closed", 4),
        ("play ; sync", "';' isn't part of openDJscript", 5),
    ];
    for (src, message, at) in cases {
        let err = parse(src).unwrap_err();
        assert_eq!((err.message.as_str(), err.at), (message, Some(at)), "{src}");
    }
}

#[test]
fn loop_defaults_to_four_beats() {
    let mut t = Test::new();
    run("loop", &mut t).unwrap();
    run("Loop_Exit", &mut t).unwrap();
}
