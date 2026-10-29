//! The verbs that are controls underneath.

use opendj_control::{ControlHandle, ControlRegistry};

use crate::verbs::Kind;
use crate::{Call, Error};

/// A control of the call's deck, or one named in full as `[Group],item`.
/// `{n}` in a full name stands for the deck:
/// `[EqualizerRack1_[Channel{n}]_Effect1],parameter3`.
fn control(reg: &ControlRegistry, call: &Call, item: &str) -> Result<ControlHandle, Error> {
    let item = if item.contains("{n}") {
        let d = call
            .deck
            .ok_or_else(|| Error::new(format!("'{}' needs a deck", call.verb.name)))?;
        item.replace("{n}", &d.to_string())
    } else {
        item.to_string()
    };
    let (group, item) = match item.strip_prefix('[').and_then(|r| r.split_once("],")) {
        Some((group, item)) => (format!("[{group}]"), item.to_string()),
        None => match call.deck {
            Some(d) => (format!("[Channel{d}]"), item),
            None => return Err(Error::new(format!("'{item}' needs a deck"))),
        },
    };
    reg.get(&group, &item)
        .ok_or_else(|| Error::new(format!("There's no control {group},{item}")))
}

/// A level argument, or the controller's when none was written.
fn level(call: &Call, i: usize) -> Result<f64, Error> {
    match call.args.get(i).map(|a| a.number()) {
        Some(n) if !n.is_nan() => Ok(n),
        _ => call
            .input
            .ok_or_else(|| Error::new(format!("'{}' needs a level here", call.verb.name))),
    }
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
        Kind::Level(item) | Kind::Value(item) => {
            control(reg, call, item)?.set_parameter(level(call, 0)?)
        }
        Kind::Set => control(reg, call, named())?.set(arg(1)),
        Kind::Adjust => control(reg, call, named())?.set_parameter(level(call, 1)?),
        Kind::ToggleNamed => control(reg, call, named())?.toggle(),
        Kind::PressNamed => press(&control(reg, call, named())?),
        Kind::Get => {
            return Err(Error::new(
                "'get' only asks, in a condition such as get 'keylock' ? … : …",
            ))
        }
        Kind::Reading(_) | Kind::Var | Kind::Input => {
            return Err(Error::new(format!(
                "'{}' only asks, in a condition",
                call.verb.name
            )))
        }
        Kind::SetVar | Kind::ToggleVar | Kind::Light | Kind::App => {
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
        Kind::Adjust => control(reg, call, named())?.get_parameter() > 0.0,
        Kind::Value(item) | Kind::Reading(item) => control(reg, call, item)?.get_parameter() > 0.0,
        Kind::Input => call.input.is_some_and(|v| v > 0.0),
        Kind::Press(_)
        | Kind::Level(_)
        | Kind::Set
        | Kind::PressNamed
        | Kind::Var
        | Kind::SetVar
        | Kind::ToggleVar
        | Kind::Light
        | Kind::App => {
            return Err(Error::new(format!(
                "'{}' can't be asked in a condition",
                call.verb.name
            )))
        }
    })
}

/// How far up a control-backed verb is, 0 to 1, for comparisons such as
/// `vu_meter > 90%` or `get 'rate' > 0.5`.
pub fn level_control(reg: &ControlRegistry, call: &Call) -> Result<f64, Error> {
    let named = || call.args.first().map(|a| a.text()).unwrap_or("");
    match call.verb.kind {
        Kind::Level(item) | Kind::Value(item) | Kind::Reading(item) => {
            Ok(control(reg, call, item)?.get_parameter())
        }
        Kind::Get | Kind::ToggleNamed | Kind::Set => Ok(control(reg, call, named())?.get()),
        Kind::Adjust => Ok(control(reg, call, named())?.get_parameter()),
        Kind::Input => call
            .input
            .ok_or_else(|| Error::new("'value' is only there when a controller runs the line")),
        _ => Ok(if query_control(reg, call)? { 1.0 } else { 0.0 }),
    }
}
