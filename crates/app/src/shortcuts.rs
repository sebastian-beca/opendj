//! Keyboard shortcuts: a key, written like `cmd+1`, runs an openDJscript
//! line. The defaults are here; the DJ's own replace them key by key
//! (settings `keyboard_shortcuts`). Space, ⌘, and Esc stay fixed in
//! main.slint and never reach this.

use mixxx_control::ControlRegistry;
use opendj_script::{query_control, run_control, Call, Error, Host, Kind};

use crate::browser::Browser;
use crate::settings;

pub struct Shortcut {
    pub key: &'static str,
    pub script: &'static str,
}

pub const DEFAULTS: &[Shortcut] = &[
    Shortcut {
        key: "cmd+1",
        script: "rating 1",
    },
    Shortcut {
        key: "cmd+2",
        script: "rating 2",
    },
    Shortcut {
        key: "cmd+3",
        script: "rating 3",
    },
    Shortcut {
        key: "cmd+4",
        script: "rating 4",
    },
    Shortcut {
        key: "cmd+5",
        script: "rating 5",
    },
];

/// Modifiers in the order keys are written.
const MODIFIERS: [&str; 3] = ["cmd", "alt", "shift"];

/// Keys without a character of their own, as Slint reports them.
fn special(name: &str) -> Option<char> {
    use slint::platform::Key;
    let key = match name {
        "space" => return Some(' '),
        "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ => return None,
    };
    Some(char::from(key))
}

const SPECIAL_NAMES: [&str; 26] = [
    "space",
    "return",
    "tab",
    "escape",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

/// The key a press names, as shortcuts write it: `cmd+alt+shift+k`. `cmd`
/// is ⌘ on macOS and Ctrl elsewhere (Slint reports both as control).
pub fn key_name(text: &str, cmd: bool, alt: bool, shift: bool) -> Option<String> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let name = match SPECIAL_NAMES.iter().find(|n| special(n) == Some(c)) {
        Some(n) => n.to_string(),
        // Shift turns digits into symbols on a US keyboard; name the digit.
        None => match ")!@#$%^&*(".find(c).filter(|_| shift) {
            Some(digit) => digit.to_string(),
            None if c.is_control() => return None,
            None => c.to_lowercase().to_string(),
        },
    };
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .zip([cmd, alt, shift])
        .filter(|(_, on)| *on)
        .map(|(m, _)| *m)
        .collect();
    parts.push(&name);
    Some(parts.join("+"))
}

/// A key as someone wrote it ("Shift+Cmd+F1", "⌘1") in the form
/// [`key_name`] gives, so the two compare equal.
pub fn normalize(key: &str) -> Option<String> {
    let key = key
        .trim()
        .to_lowercase()
        .replace('⌘', "cmd+")
        .replace('⌥', "alt+")
        .replace('⇧', "shift+");
    let mut on = [false; 3];
    let mut name = None;
    for part in key.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let modifier = match part {
            "cmd" | "command" | "ctrl" | "control" => Some(0),
            "alt" | "option" | "opt" => Some(1),
            "shift" => Some(2),
            _ => None,
        };
        match modifier {
            Some(i) => on[i] = true,
            None if name.is_none() => name = Some(part.to_string()),
            None => return None,
        }
    }
    let name = name?;
    if name.chars().count() != 1 && !SPECIAL_NAMES.contains(&name.as_str()) {
        return None;
    }
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .zip(on)
        .filter(|(_, on)| *on)
        .map(|(m, _)| *m)
        .collect();
    parts.push(&name);
    Some(parts.join("+"))
}

/// The line a key runs: the DJ's own for it, else the default.
pub fn script_for(key: &str) -> Option<String> {
    let own = settings::keyboard_shortcuts()
        .into_iter()
        .find(|(k, _)| normalize(k).as_deref() == Some(key));
    match own {
        Some((_, script)) if script.trim().is_empty() => None,
        Some((_, script)) => Some(script),
        None => DEFAULTS
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.script.to_string()),
    }
}

/// Runs openDJscript in the app: control verbs on the registry, `rating`
/// on the browser.
pub struct AppHost<'a> {
    pub registry: &'a ControlRegistry,
    pub browser: &'a mut Browser,
    /// The rows picked in the track list, first and last; -1 for none.
    pub picked: (i32, i32),
    /// 1 to 4.
    pub master: u8,
}

impl Host for AppHost<'_> {
    fn master_deck(&self) -> u8 {
        self.master
    }

    fn run(&mut self, call: &Call) -> Result<(), Error> {
        if call.verb.kind != Kind::App {
            return run_control(self.registry, call);
        }
        match call.verb.name {
            "rating" => {
                let stars = call.args[0].number().round() as u8;
                match call.deck {
                    Some(deck) => {
                        if !self.browser.rate_deck(deck as usize - 1, stars) {
                            return Err(Error::new(format!("Deck {deck} has no track")));
                        }
                    }
                    None => {
                        let (first, last) = self.picked;
                        if first < 0 {
                            return Err(Error::new("Pick a track in the browser first"));
                        }
                        self.browser
                            .rate_rows(first as usize, last.max(first) as usize, stars);
                    }
                }
                Ok(())
            }
            other => Err(Error::new(format!("'{other}' isn't available here"))),
        }
    }

    fn query(&mut self, call: &Call) -> Result<bool, Error> {
        query_control(self.registry, call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_named_the_same_way_they_are_written() {
        assert_eq!(key_name("1", true, false, false).as_deref(), Some("cmd+1"));
        assert_eq!(
            key_name("!", true, false, true).as_deref(),
            Some("cmd+shift+1")
        );
        assert_eq!(
            key_name("K", false, true, true).as_deref(),
            Some("alt+shift+k")
        );
        let f1 = char::from(slint::platform::Key::F1).to_string();
        assert_eq!(key_name(&f1, false, false, false).as_deref(), Some("f1"));
        for (written, named) in [
            ("Cmd+1", "cmd+1"),
            ("Shift+Cmd+F1", "cmd+shift+f1"),
            ("⌘⇧2", "cmd+shift+2"),
            ("ctrl + alt + x", "cmd+alt+x"),
        ] {
            assert_eq!(normalize(written).as_deref(), Some(named), "{written}");
        }
        assert_eq!(normalize("cmd+ab"), None);
        assert_eq!(normalize("cmd"), None);
    }

    #[test]
    fn the_default_shortcuts_parse_and_are_written_normally() {
        for s in DEFAULTS {
            assert_eq!(normalize(s.key).as_deref(), Some(s.key));
            opendj_script::parse(s.script).unwrap();
        }
    }
}
