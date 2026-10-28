//! The verbs that are controls underneath.

use mixxx_control::{ControlHandle, ControlRegistry};

use crate::verbs::Kind;
use crate::{Call, Error};

/// A control of the call's deck, or one named in full as `[Group],item`.
fn control(reg: &ControlRegistry, call: &Call, item: &str) -> Result<ControlHandle, Error> {
    let (group, item) = match item.strip_prefix('[').and_then(|r| r.split_once("],")) {
        Some((group, item)) => (format!("[{group}]"), item.to_string()),
        None => match call.deck {
            Some(d) => (format!("[Channel{d}]"), item.to_string()),
            None => return Err(Error::new(format!("'{item}' needs a deck"))),
        },
    };
    reg.get(&group, &item)
        .ok_or_else(|| Error::new(format!("There's no control {group},{item}")))
}

fn press(c: &ControlHandle) {
    c.set_bool(true);
    c.set_bool(false);
}

fn beats(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Does a control-backed verb. [`Kind::App`] verbs are the host's.
pub fn run_control(reg: &ControlRegistry, call: &Call) -> Result<(), Error> {
    let arg = |i: usize| call.args.get(i).map(|a| a.number()).unwrap_or(0.0);
    let named = || call.args.first().map(|a| a.text()).unwrap_or("");
    match call.verb.kind {
        Kind::Play => control(reg, call, "play")?.set_bool(true),
        Kind::Pause => control(reg, call, "play")?.set_bool(false),
        Kind::Toggle(item) => control(reg, call, item)?.toggle(),
        Kind::Press(item) => press(&control(reg, call, item)?),
        Kind::Loop => {
            let size = beats(arg(0));
            let c = control(reg, call, &format!("beatloop_{size}_toggle"))
                .map_err(|_| Error::new(format!("There's no {size}-beat loop")))?;
            press(&c);
        }
        Kind::LoopExit => {
            if control(reg, call, "loop_enabled")?.get_bool() {
                press(&control(reg, call, "reloop_toggle")?);
            }
        }
        Kind::HotCue => press(&control(
            reg,
            call,
            &format!("hotcue_{}_activate", beats(arg(0))),
        )?),
        Kind::Level(item) => control(reg, call, item)?.set_parameter(arg(0)),
        Kind::Set => control(reg, call, named())?.set(arg(1)),
        Kind::ToggleNamed => control(reg, call, named())?.toggle(),
        Kind::PressNamed => press(&control(reg, call, named())?),
        Kind::Get => {
            return Err(Error::new(
                "'get' only asks, in a condition such as get 'keylock' ? … : …",
            ))
        }
        Kind::App => {
            return Err(Error::new(format!(
                "'{}' isn't available here",
                call.verb.name
            )))
        }
    }
    Ok(())
}

/// Answers a control-backed verb used as a condition.
pub fn query_control(reg: &ControlRegistry, call: &Call) -> Result<bool, Error> {
    let named = || call.args.first().map(|a| a.text()).unwrap_or("");
    Ok(match call.verb.kind {
        Kind::Play => control(reg, call, "play")?.get_bool(),
        Kind::Pause => !control(reg, call, "play")?.get_bool(),
        Kind::Toggle(item) => control(reg, call, item)?.get_bool(),
        Kind::Loop | Kind::LoopExit => control(reg, call, "loop_enabled")?.get_bool(),
        Kind::HotCue => {
            let n = beats(call.args.first().map(|a| a.number()).unwrap_or(0.0));
            control(reg, call, &format!("hotcue_{n}_enabled"))?.get_bool()
        }
        Kind::ToggleNamed | Kind::Get => control(reg, call, named())?.get() > 0.0,
        Kind::Press(_) | Kind::Level(_) | Kind::Set | Kind::PressNamed | Kind::App => {
            return Err(Error::new(format!(
                "'{}' can't be asked in a condition",
                call.verb.name
            )))
        }
    })
}
