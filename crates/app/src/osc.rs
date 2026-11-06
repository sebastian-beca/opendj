//! Settings › Controllers › OSC: starts and stops the OSC port
//! (`opendj_osc`) as the settings say.
//!
//! Off at first. The port runs on a thread of its own; this only opens it
//! again when a setting changes. Switches apply at once; typed settings
//! (the port, the send targets) once typing pauses, so a half-typed
//! port number isn't opened for every key.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use opendj_control::ControlRegistry;
use opendj_osc::{Config, Osc, DEFAULT_PORT};

use crate::settings;

/// The switch: off at first.
pub const ENABLED: &str = "osc";
/// The port to listen on.
pub const PORT: &str = "osc_port";
/// Listen only for this computer: on at first.
pub const LOCAL_ONLY: &str = "osc_local_only";
/// Ignore incoming messages but subscriptions.
pub const SEND_ONLY: &str = "osc_send_only";
/// Where feedback and events go: `host:port`, separated by commas.
pub const TARGETS: &str = "osc_targets";

static REGISTRY: OnceLock<Arc<ControlRegistry>> = OnceLock::new();
static RUNNING: Mutex<Option<(Config, Osc)>> = Mutex::new(None);
/// Why the port couldn't open, if it couldn't.
static PROBLEM: Mutex<String> = Mutex::new(String::new());
/// When a typed setting last changed, until it's applied.
static CHANGED: Mutex<Option<Instant>> = Mutex::new(None);
/// How long typing must pause before a new port or target list applies.
const SETTLE: Duration = Duration::from_millis(800);

/// Starts OSC if it's switched on. Call once at startup.
pub fn install(registry: &Arc<ControlRegistry>) {
    let _ = REGISTRY.set(registry.clone());
    apply();
}

/// Marks a typed setting changed; [`tick`] applies it.
pub fn settings_changed() {
    if let Ok(mut c) = CHANGED.lock() {
        *c = Some(Instant::now());
    }
}

/// Applies typed settings that changed. Call a couple of times a second.
pub fn tick() {
    let due = CHANGED.lock().is_ok_and(|mut c| {
        let due = c.is_some_and(|t| t.elapsed() >= SETTLE);
        if due {
            *c = None;
        }
        due
    });
    if due {
        apply();
    }
}

/// The port number in Settings, or 9000.
pub fn port() -> u16 {
    let p = settings::number(PORT, DEFAULT_PORT as f64);
    if (1.0..=65535.0).contains(&p) {
        p as u16
    } else {
        DEFAULT_PORT
    }
}

/// The send targets as typed, and which of them can't be understood.
pub fn targets() -> (Vec<std::net::SocketAddr>, Vec<String>) {
    let mut good = Vec::new();
    let mut bad = Vec::new();
    for t in settings::string(TARGETS).split([',', ' ', '\n']) {
        if t.trim().is_empty() {
            continue;
        }
        match opendj_osc::parse_target(t) {
            Some(a) => good.push(a),
            None => bad.push(t.trim().to_string()),
        }
    }
    (good, bad)
}

fn config() -> Option<Config> {
    if !settings::flag(ENABLED, false) {
        return None;
    }
    Some(Config {
        port: port(),
        local_only: settings::flag(LOCAL_ONLY, true),
        send_only: settings::flag(SEND_ONLY, false),
        targets: targets().0,
    })
}

/// Opens, moves or closes the port to match the settings.
pub fn apply() {
    let Some(registry) = REGISTRY.get() else {
        return;
    };
    let wanted = config();
    let Ok(mut running) = RUNNING.lock() else {
        return;
    };
    if running.as_ref().map(|(c, _)| c) == wanted.as_ref() {
        return;
    }
    // Close the old port first, so the same port number can open again.
    *running = None;
    let mut problem = String::new();
    if let Some(config) = wanted {
        match Osc::start(registry.clone(), config.clone()) {
            Ok(osc) => *running = Some((config, osc)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                problem = format!(
                    "Port {} is used by another app. Pick another port.",
                    config.port
                );
            }
            Err(e) => problem = format!("OSC couldn't start: {e}"),
        }
    }
    if let Ok(mut p) = PROBLEM.lock() {
        *p = problem;
    }
}

/// The line under the switch: where it listens and who's connected, or
/// what went wrong.
pub fn status() -> String {
    let problem = PROBLEM.lock().map(|p| p.clone()).unwrap_or_default();
    if !problem.is_empty() {
        return problem;
    }
    let Ok(running) = RUNNING.lock() else {
        return String::new();
    };
    let Some((config, osc)) = running.as_ref() else {
        return String::new();
    };
    let whom = if config.local_only {
        "apps on this computer".to_string()
    } else {
        match opendj_remote::local_addresses().first() {
            Some((_, ip)) => format!("tablets and apps at {ip}"),
            None => "apps on the network".to_string(),
        }
    };
    let clients = match osc.clients() {
        0 => "no apps yet".to_string(),
        1 => "1 app".to_string(),
        n => format!("{n} apps"),
    };
    format!("Listening on port {} for {whom} · {clients}", osc.port())
}
