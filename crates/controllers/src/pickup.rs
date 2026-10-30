//! Pick-up for knobs and faders. After a control moved without the
//! hardware (the mouse, a reset on load, sync), the hardware does nothing
//! until it reaches the control's value, so nothing jumps. Meanwhile the
//! screen shows where the hardware is, from [`hardware_position`].
//!
//! Each controller can use Scale instead ([`set_scale`]): the hardware acts
//! at once and the value moves in proportion, meeting it at the end of its
//! travel. The crossfader always follows the hardware, for scratch cuts.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use opendj_control::ConfigKey;

/// Closer than this, a knob counts as where the hardware is (3 of 128 steps,
/// as in Mixxx).
pub const THRESHOLD: f64 = 3.0 / 128.0;
/// A value this soon after the last one taken is always taken: a fast move
/// can jump past the value between two messages.
const OVERRIDE: Duration = Duration::from_millis(50);

/// Where a controller's knob or fader is, by the control it moves.
struct Position {
    parameter: f64,
    port: String,
}

static POSITIONS: Mutex<Option<HashMap<ConfigKey, Position>>> = Mutex::new(None);
static VERSION: AtomicU64 = AtomicU64::new(0);
/// Controllers using Scale, by [`crate::device_key`].
static SCALE: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Where the hardware moving `key` is, as a 0..1 parameter, while it is
/// known. Compare it with the control's own parameter.
pub fn hardware_position(key: &ConfigKey) -> Option<f64> {
    POSITIONS
        .lock()
        .unwrap()
        .as_ref()?
        .get(key)
        .map(|p| p.parameter)
}

/// Goes up whenever a hardware position changes.
pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

/// Whether the controller (by [`crate::device_key`]) uses Scale.
pub fn scale(device: &str) -> bool {
    SCALE
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|s| s.contains(device))
}

/// Switches a controller (by [`crate::device_key`]) between Pick-up and
/// Scale. Takes effect at its next message.
pub fn set_scale(device: &str, on: bool) {
    let mut s = SCALE.lock().unwrap();
    let s = s.get_or_insert_with(HashSet::new);
    if on {
        s.insert(device.to_string());
    } else {
        s.remove(device);
    }
}

pub(crate) fn note(key: &ConfigKey, parameter: f64, port: &str) {
    let mut p = POSITIONS.lock().unwrap();
    let p = p.get_or_insert_with(HashMap::new);
    let changed = p
        .get(key)
        .is_none_or(|old| old.parameter != parameter || old.port != port);
    if changed {
        p.insert(
            key.clone(),
            Position {
                parameter,
                port: port.to_string(),
            },
        );
        VERSION.fetch_add(1, Ordering::Relaxed);
    }
}

/// Forgets `key`'s position, if `port` noted it.
pub(crate) fn forget(key: &ConfigKey, port: &str) {
    let mut p = POSITIONS.lock().unwrap();
    if let Some(p) = p.as_mut() {
        if p.get(key).is_some_and(|old| old.port == port) {
            p.remove(key);
            VERSION.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Forgets every position a controller noted, when it goes away.
pub(crate) fn forget_port(port: &str) {
    let mut p = POSITIONS.lock().unwrap();
    if let Some(p) = p.as_mut() {
        let before = p.len();
        p.retain(|_, old| old.port != port);
        if p.len() != before {
            VERSION.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Whether a control always follows the hardware.
pub(crate) fn always_jumps(key: &ConfigKey) -> bool {
    key.item == "crossfader"
}

/// One knob or fader's pick-up (Mixxx's soft takeover, plus Scale).
pub(crate) struct Takeover {
    time: Option<Instant>,
    /// The hardware's last position, once it sent one.
    prev: Option<f64>,
    /// Set by `softTakeoverIgnoreNextValue` and initially.
    pub(crate) ignore_next: bool,
    /// The message that moves it, while known: for drawing its position.
    pub(crate) source: Option<(u8, u8)>,
    /// The control it moved last, resolved through the deck switches.
    pub(crate) moved: Option<ConfigKey>,
}

impl Takeover {
    pub(crate) fn new() -> Self {
        Self {
            time: None,
            prev: None,
            ignore_next: true,
            source: None,
            moved: None,
        }
    }

    /// The hardware's last position.
    pub(crate) fn hardware(&self) -> Option<f64> {
        self.prev
    }

    pub(crate) fn will_ignore(&self, current: f64, new: f64, now: Instant) -> bool {
        let (Some(time), Some(prev)) = (self.time, self.prev) else {
            return true;
        };
        if self.ignore_next {
            return true;
        }
        if now < time + OVERRIDE {
            return false;
        }
        let difference = current - new;
        let prev_diff = current - prev;
        if difference.is_sign_negative() != prev_diff.is_sign_negative() {
            return false;
        }
        !(difference.abs() <= THRESHOLD || prev_diff.abs() <= THRESHOLD)
    }

    /// The hardware moved to `new` while the control is at `current` (both
    /// parameters): what to set the control to, or `None` to leave it.
    pub(crate) fn take(&mut self, current: f64, new: f64, scale: bool) -> Option<f64> {
        let now = Instant::now();
        let prev = self.prev;
        let out = if self.ignore_next {
            self.ignore_next = false;
            // The first value after enabling (or after a jump) is only taken
            // where the control already is, so it does not leap; afterwards
            // the usual rules apply.
            self.time = Some(now - OVERRIDE * 4);
            ((current - new).abs() <= THRESHOLD).then_some(new)
        } else if scale {
            prev.map(|p| scaled(current, p, new))
        } else if self.will_ignore(current, new, now) {
            None
        } else {
            Some(new)
        };
        if out.is_some() {
            self.time = Some(now);
        }
        self.prev = Some(new);
        out
    }
}

/// Scale: the hardware went from `prev` to `new`; the value closes the gap in
/// proportion, so both reach the end of the travel together.
fn scaled(current: f64, prev: f64, new: f64) -> f64 {
    if (current - prev).abs() <= THRESHOLD {
        return new;
    }
    let v = if new < prev {
        current * new / prev
    } else if new > prev {
        1.0 - (1.0 - current) * (1.0 - new) / (1.0 - prev)
    } else {
        current
    };
    if (v - new).abs() <= THRESHOLD {
        new
    } else {
        v.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_meets_the_hardware_at_the_end() {
        // Fader at the top, value at half: moving down halves it in step.
        assert!((scaled(0.5, 1.0, 0.8) - 0.4).abs() < 1e-9);
        assert_eq!(scaled(0.5, 1.0, 0.0), 0.0);
        // Moving up closes the gap towards the top.
        assert!((scaled(0.5, 0.2, 0.6) - 0.75).abs() < 1e-9);
        // Together already: follows.
        assert_eq!(scaled(0.3, 0.3, 0.5), 0.5);
    }

    #[test]
    fn scale_acts_from_the_second_value() {
        let mut t = Takeover::new();
        // Far from the value: not taken.
        assert_eq!(t.take(0.5, 1.0, true), None);
        let v = t.take(0.5, 0.9, true).unwrap();
        assert!((v - 0.45).abs() < 1e-9);
    }

    #[test]
    fn pickup_waits_until_the_hardware_reaches_the_value() {
        let mut t = Takeover::new();
        // The first value is taken only where the control already is.
        assert_eq!(t.take(0.5, 0.51, false), Some(0.51));
        let mut t = Takeover::new();
        assert_eq!(t.take(0.5, 1.0, false), None);
        std::thread::sleep(OVERRIDE * 2);
        assert_eq!(t.take(0.5, 0.8, false), None);
        // Crossing the value between two messages picks it up.
        assert_eq!(t.take(0.5, 0.45, false), Some(0.45));
        assert_eq!(t.take(0.45, 0.2, false), Some(0.2));
    }
}
