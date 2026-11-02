//! The phone remote, on the app's side: switches the server in
//! `opendj-remote` on and off, carries out what phones ask for on the UI
//! thread, and pushes Automix, the decks and the main levels to them.
//!
//! Everything a phone does goes through the same functions and controls as
//! the Automix panel and the MASTER panel, so the app and the phone always
//! show the same thing.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::{deck_group, Track, NUM_DECKS};
use opendj_remote::{Event, Server};
use serde_json::{json, Value};
use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::automix::{AutoMix, LENGTHS, LENGTH_NAMES, STYLES, STYLE_NAMES};
use crate::automix_order::ORDERS;
use crate::browser::Browser;
use crate::{AppWindow, AudioSettings, RemoteDevice, RemoteUi};

/// How often each kind of state is looked at for changes. Deck positions
/// move all the time, so they go a little less often.
const AUTOMIX_EVERY: Duration = Duration::from_millis(100);
const DECKS_EVERY: Duration = Duration::from_millis(250);
const PANEL_EVERY: Duration = Duration::from_millis(500);
/// Automix plays on decks 1 and 2.
const DECKS: usize = 2;

struct DeckHandles {
    play: ControlHandle,
    playposition: ControlHandle,
    volume: ControlHandle,
    bpm: ControlHandle,
    rate: ControlHandle,
    rate_range: ControlHandle,
    rate_dir: ControlHandle,
}

/// The main level sliding to `to`, from Fade out or Bring back.
struct Fade {
    from: f64,
    to: f64,
    seconds: f64,
    done: f64,
}

pub struct Remote {
    server: Option<Server>,
    decks: [DeckHandles; DECKS],
    crossfader: ControlHandle,
    master: ControlHandle,
    booth: ControlHandle,
    fade: Option<Fade>,
    /// The main level before Fade out, for Bring back.
    faded_from: Option<f64>,
    last_automix: Instant,
    last_decks: Instant,
    last_panel: Instant,
    /// The QR code's generation last drawn, and the address in it.
    drawn: (u64, String),
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// When a phone was last seen, for the device list.
fn seen(last: u64) -> String {
    let ago = now_unix().saturating_sub(last);
    match ago {
        0..=59 => "Seen just now".into(),
        60..=3599 => format!("Seen {} min ago", ago / 60),
        3600..=86_399 => format!("Seen {} h ago", ago / 3600),
        _ => format!("Seen {} days ago", ago / 86_400),
    }
}

fn qr_image(text: &str) -> Image {
    match opendj_remote::qr::rgba(text, 4) {
        Some((side, px)) => {
            let buf =
                SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&px, side as u32, side as u32);
            Image::from_rgba8(buf)
        }
        None => Image::default(),
    }
}

impl Remote {
    pub fn new(reg: &ControlRegistry) -> Self {
        let get = |group: &str, key: &str| {
            reg.get(group, key)
                .unwrap_or_else(|| panic!("engine control {group},{key}"))
        };
        Self {
            server: None,
            decks: std::array::from_fn(|d| {
                let g = deck_group(d);
                DeckHandles {
                    play: get(&g, "play"),
                    playposition: get(&g, "playposition"),
                    volume: get(&g, "volume"),
                    bpm: get(&g, "bpm"),
                    rate: get(&g, "rate"),
                    rate_range: get(&g, "rateRange"),
                    rate_dir: get(&g, "rate_dir"),
                }
            }),
            crossfader: get("[Master]", "crossfader"),
            master: get("[Master]", "gain"),
            booth: get("[Master]", "booth_gain"),
            fade: None,
            faded_from: None,
            last_automix: Instant::now(),
            last_decks: Instant::now(),
            last_panel: Instant::now(),
            drawn: (0, String::new()),
        }
    }

    /// Wires up the REMOTE panel, and starts the remote if it was on last
    /// time.
    pub fn install(remote: &std::rc::Rc<std::cell::RefCell<Remote>>, app: &AppWindow) {
        let ui = app.global::<RemoteUi>();
        {
            let r = remote.clone();
            let weak = app.as_weak();
            ui.on_set_on(move |on| {
                if let Some(app) = weak.upgrade() {
                    r.borrow_mut().set_on(on, &app);
                }
            });
        }
        {
            let r = remote.clone();
            let weak = app.as_weak();
            ui.on_revoke(move |id| {
                let r = r.borrow_mut();
                if let Some(s) = &r.server {
                    s.revoke(&id);
                    if let Some(devices) = s.take_changed_devices() {
                        crate::settings::set_remote_devices(&devices);
                    }
                } else {
                    let mut devices = crate::settings::remote_devices();
                    devices.retain(|d| d.id != id.as_str());
                    crate::settings::set_remote_devices(&devices);
                }
                if let Some(app) = weak.upgrade() {
                    r.show_panel(&app);
                }
            });
        }
        {
            let r = remote.clone();
            let weak = app.as_weak();
            ui.on_opened(move || {
                if let Some(app) = weak.upgrade() {
                    let mut r = r.borrow_mut();
                    if let Some(s) = &r.server {
                        s.refresh_code();
                    }
                    r.drawn = (0, String::new());
                    r.show_panel(&app);
                }
            });
        }
        if crate::settings::remote_on() {
            remote.borrow_mut().set_on(true, app);
        } else {
            remote.borrow().show_panel(app);
        }
    }

    pub fn set_on(&mut self, on: bool, app: &AppWindow) {
        let ui = app.global::<RemoteUi>();
        ui.set_error("".into());
        if on && self.server.is_none() {
            match Server::start(
                opendj_remote::DEFAULT_PORT,
                crate::settings::remote_devices(),
            ) {
                Ok(s) => self.server = Some(s),
                Err(e) => ui.set_error(format!("The remote couldn't start: {e}").into()),
            }
        } else if !on {
            self.server = None;
            self.fade = None;
        }
        crate::settings::set_remote_on(self.server.is_some());
        self.drawn = (0, String::new());
        self.show_panel(app);
    }

    /// Fills the REMOTE panel: on or off, the QR code, PIN and addresses,
    /// and the paired phones.
    fn show_panel(&self, app: &AppWindow) {
        let ui = app.global::<RemoteUi>();
        let Some(server) = &self.server else {
            ui.set_on(false);
            ui.set_connected(false);
            let devices: Vec<RemoteDevice> = crate::settings::remote_devices()
                .iter()
                .map(|d| RemoteDevice {
                    id: d.id.clone().into(),
                    name: d.name.clone().into(),
                    connected: false,
                    seen: seen(d.last_seen).into(),
                })
                .collect();
            ui.set_devices(ModelRc::new(VecModel::from(devices)));
            return;
        };
        ui.set_on(true);
        let connected = server.connected();
        ui.set_connected(!connected.is_empty());
        let devices: Vec<RemoteDevice> = server
            .devices()
            .iter()
            .map(|d| {
                let on = connected.contains(&d.id);
                RemoteDevice {
                    id: d.id.clone().into(),
                    name: d.name.clone().into(),
                    connected: on,
                    seen: if on {
                        "Connected".into()
                    } else {
                        seen(d.last_seen).into()
                    },
                }
            })
            .collect();
        ui.set_devices(ModelRc::new(VecModel::from(devices)));
        ui.set_pin(format!("{} {}", &server.pin()[..3], &server.pin()[3..]).into());
        ui.set_code_left(server.code_left().as_secs() as i32);
    }

    /// Draws the QR code again when its code or the address changed.
    fn draw_qr(&mut self, app: &AppWindow) {
        let Some(server) = &self.server else {
            return;
        };
        let (code, generation) = server.pair_code();
        let addresses = opendj_remote::local_addresses();
        let Some((_, ip)) = addresses.first() else {
            let ui = app.global::<RemoteUi>();
            ui.set_url("".into());
            ui.set_error("This computer isn't on a network. Join Wi-Fi to use the remote.".into());
            return;
        };
        let base = opendj_remote::base_url(*ip, server.port());
        if self.drawn == (generation, base.clone()) {
            return;
        }
        let ui = app.global::<RemoteUi>();
        if ui.get_error().starts_with("This computer isn't") {
            ui.set_error("".into());
        }
        ui.set_qr(qr_image(&opendj_remote::pair_url(
            *ip,
            server.port(),
            &code,
        )));
        ui.set_url(base.trim_end_matches('/').into());
        let others: Vec<String> = addresses[1..]
            .iter()
            .map(|(_, ip)| {
                opendj_remote::base_url(*ip, server.port())
                    .trim_end_matches('/')
                    .to_string()
            })
            .collect();
        ui.set_other_urls(others.join("  ·  ").into());
        self.drawn = (generation, base);
    }

    /// Runs once a UI frame: carries out what phones asked, moves a fade,
    /// and pushes what changed.
    pub fn tick(
        &mut self,
        app: &AppWindow,
        automix: &mut AutoMix,
        browser: &mut Browser,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
        dt: f64,
    ) {
        self.step_fade(dt);
        let Some(server) = &self.server else {
            return;
        };
        let mut devices_changed = false;
        for event in server.poll() {
            match event {
                Event::Request {
                    client,
                    id,
                    method,
                    params,
                } => {
                    let result = self.handle(&method, &params, app, automix, browser);
                    if let Some(s) = &self.server {
                        s.reply(client, id, result);
                    }
                    // Show the change on the phone straight away.
                    self.last_automix = Instant::now() - AUTOMIX_EVERY;
                }
                Event::Connected { .. } | Event::Disconnected { .. } => devices_changed = true,
            }
        }
        let Some(server) = &self.server else {
            return;
        };
        if let Some(devices) = server.take_changed_devices() {
            crate::settings::set_remote_devices(&devices);
        }
        let now = Instant::now();
        if now.duration_since(self.last_automix) >= AUTOMIX_EVERY {
            self.last_automix = now;
            server.publish("automix", self.automix_state(app, automix, browser, tracks));
            server.publish("mixer", self.mixer_state(app));
        }
        if now.duration_since(self.last_decks) >= DECKS_EVERY {
            self.last_decks = now;
            server.publish("decks", self.decks_state(automix, browser, tracks));
        }
        if devices_changed || now.duration_since(self.last_panel) >= PANEL_EVERY {
            self.last_panel = now;
            self.show_panel(app);
            self.draw_qr(app);
        }
    }

    /// Carries out one request from a phone.
    fn handle(
        &mut self,
        method: &str,
        params: &Value,
        app: &AppWindow,
        automix: &mut AutoMix,
        browser: &mut Browser,
    ) -> Result<Value, String> {
        let index = || {
            params["index"]
                .as_u64()
                .map(|i| i as usize)
                .ok_or_else(|| "Missing index".to_string())
        };
        let track = || {
            params["id"]
                .as_u64()
                .map(|i| i as usize)
                .ok_or_else(|| "Missing track".to_string())
        };
        match method {
            "automix.set" => {
                let on = params["on"].as_bool().ok_or("Missing on")?;
                automix.set_enabled(on);
            }
            "automix.toggle" => automix.toggle(),
            "automix.mix_now" => automix.mix_now(),
            "automix.skip" => automix.skip(),
            "automix.length" => {
                let i = index()?;
                if i >= LENGTHS.len() {
                    return Err("No such length".into());
                }
                automix.set_length(i);
            }
            "automix.style" => {
                let i = index()?;
                if i >= STYLES.len() {
                    return Err("No such style".into());
                }
                automix.set_style(i);
            }
            "automix.order" => {
                let order = *ORDERS.get(index()?).ok_or("No such order")?;
                let note = browser.order_automix_for(automix, order);
                app.set_automix_order_note(note.into());
            }
            "automix.undo" => {
                browser.undo_automix_order();
                app.set_automix_order_note("".into());
            }
            "automix.move" => {
                let id = track()?;
                let to = params["to"].as_u64().ok_or("Missing place")? as usize;
                let to = to.max(held_count(automix, browser));
                if !browser.move_in_automix(id, to) {
                    return Err("That track isn't on the Automix list any more.".into());
                }
            }
            "automix.play_next" => {
                let id = track()?;
                let to = held_count(automix, browser);
                if !browser.move_in_automix(id, to) {
                    return Err("That track isn't on the Automix list any more.".into());
                }
            }
            "automix.remove" => {
                let id = track()?;
                if automix.held().0 == Some(id) {
                    return Err("That track is playing.".into());
                }
                browser.remove_from_automix(id);
            }
            "automix.add" => {
                let id = track()?;
                let at = params["next"]
                    .as_bool()
                    .unwrap_or(false)
                    .then(|| held_count(automix, browser));
                if !browser.add_to_automix_at(id, at) {
                    return Err("That track isn't in the collection any more.".into());
                }
            }
            "library.search" => return Ok(search(browser, params["text"].as_str().unwrap_or(""))),
            "mixer.set" => {
                let value = params["value"]
                    .as_f64()
                    .ok_or("Missing value")?
                    .clamp(0.0, 1.0);
                match params["level"].as_str() {
                    Some("master") => {
                        self.fade = None;
                        self.faded_from = None;
                        self.master.set(value);
                    }
                    Some("booth") => self.booth.set(value),
                    _ => return Err("No such level".into()),
                }
            }
            "mixer.fade_out" => {
                let seconds = params["seconds"].as_f64().unwrap_or(8.0).clamp(1.0, 60.0);
                let from = self.master.get();
                if self.faded_from.is_none() {
                    self.faded_from = Some(from);
                }
                self.fade = Some(Fade {
                    from,
                    to: 0.0,
                    seconds,
                    done: 0.0,
                });
            }
            "mixer.fade_in" => {
                let to = self.faded_from.take().unwrap_or(0.5);
                self.fade = Some(Fade {
                    from: self.master.get(),
                    to,
                    seconds: 2.0,
                    done: 0.0,
                });
            }
            _ => return Err(format!("Unknown request {method}")),
        }
        Ok(Value::Bool(true))
    }

    fn step_fade(&mut self, dt: f64) {
        let Some(f) = &mut self.fade else {
            return;
        };
        f.done = (f.done + dt.max(0.0) / f.seconds).min(1.0);
        self.master.set(f.from + (f.to - f.from) * f.done);
        if f.done >= 1.0 {
            self.fade = None;
        }
    }

    fn automix_state(
        &self,
        app: &AppWindow,
        automix: &AutoMix,
        browser: &Browser,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) -> Value {
        let camelot = browser.camelot();
        let (playing, waiting) = automix.held();
        let queue: Vec<Value> = (0..browser.automix_len())
            .filter_map(|i| browser.automix_at(i))
            .map(|t| {
                let state = if Some(t.id) == playing {
                    "playing"
                } else if Some(t.id) == waiting {
                    "waiting"
                } else {
                    ""
                };
                json!({
                    "id": t.id,
                    "title": t.title,
                    "artist": t.artist,
                    "bpm": t.bpm,
                    "key": crate::settings::show_key(&t.key, camelot),
                    "time": t.duration.round(),
                    "state": state,
                })
            })
            .collect();
        let total: f64 = (0..browser.automix_len())
            .filter_map(|i| browser.automix_at(i))
            .map(|t| t.duration)
            .sum();
        json!({
            "on": automix.is_on(),
            "status": automix.status(tracks),
            "length": automix.shown_length(),
            "style": automix.style(),
            "lengths": LENGTH_NAMES,
            "styles": STYLE_NAMES,
            "orders": ORDERS.iter().map(|o| o.name()).collect::<Vec<_>>(),
            "note": app.get_automix_order_note().to_string(),
            "can_undo": browser.can_undo_automix_order() && !app.get_automix_order_note().is_empty(),
            "fixed": held_count(automix, browser),
            "queue": queue,
            "total": total.round(),
        })
    }

    fn decks_state(
        &self,
        automix: &AutoMix,
        browser: &Browser,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
    ) -> Value {
        let camelot = browser.camelot();
        let spans = automix.spans(tracks);
        // How loud each deck is in the mix: its volume and play, through
        // the crossfader.
        let xf = self.crossfader.get();
        let weight = |d: usize| {
            let h = &self.decks[d];
            let side = if d == 0 {
                (1.0 - xf).min(1.0)
            } else {
                (1.0 + xf).min(1.0)
            };
            if h.play.get_bool() && tracks[d].is_some() {
                h.volume.get() * side
            } else {
                0.0
            }
        };
        let w = [weight(0), weight(1)];
        let live = if w[0] <= 0.0 && w[1] <= 0.0 {
            None
        } else if w[0] >= w[1] {
            Some(0)
        } else {
            Some(1)
        };
        let decks: Vec<Value> = (0..DECKS)
            .map(|d| {
                let h = &self.decks[d];
                let Some(t) = tracks[d].as_deref() else {
                    return json!({"title": ""});
                };
                let pos = h.playposition.get().clamp(0.0, 1.0);
                let speed = crate::speed(&h.rate, &h.rate_range, &h.rate_dir).max(0.1);
                let left = t.duration * (1.0 - pos) / speed;
                let bpm = h.bpm.get();
                let mut v = json!({
                    "title": t.title,
                    "artist": t.artist,
                    "bpm": if bpm > 0.0 { (bpm * 10.0).round() / 10.0 } else { t.bpm },
                    "key": crate::settings::show_key(&t.key, camelot),
                    "duration": t.duration.round(),
                    "position": (pos * 1000.0).round() / 1000.0,
                    "left": left.round(),
                    "playing": h.play.get_bool(),
                    "live": live == Some(d),
                });
                if let Some((from, to)) = spans[d] {
                    v["mix_from"] = json!((from * 1000.0).round() / 1000.0);
                    v["mix_to"] = json!((to * 1000.0).round() / 1000.0);
                }
                v
            })
            .collect();
        json!({ "decks": decks })
    }

    fn mixer_state(&self, app: &AppWindow) -> Value {
        let round = |v: f64| (v * 100.0).round() / 100.0;
        json!({
            "master": round(self.master.get()),
            "booth": round(self.booth.get()),
            "booth_on": app.global::<AudioSettings>().get_booth(),
            "fading": self.fade.as_ref().is_some_and(|f| f.to < f.from),
            "faded": self.fade.is_none() && self.faded_from.is_some(),
        })
    }
}

/// Most search results sent to a phone at once.
const SEARCH_LIMIT: usize = 100;

/// Collection tracks that `text` finds, for the phone's search: at most
/// [`SEARCH_LIMIT`], with whether each is on the Automix list already.
fn search(browser: &Browser, text: &str) -> Value {
    let text = text.trim();
    if text.is_empty() {
        return json!({"tracks": [], "total": 0});
    }
    let camelot = browser.camelot();
    let queued: std::collections::HashSet<usize> = (0..browser.automix_len())
        .filter_map(|i| browser.automix_at(i))
        .map(|t| t.id)
        .collect();
    let (tracks, total) = browser.find_tracks(text, SEARCH_LIMIT);
    let tracks: Vec<Value> = tracks
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "title": t.title,
                "artist": t.artist,
                "bpm": t.bpm,
                "key": crate::settings::show_key(&t.key, camelot),
                "time": t.duration.round(),
                "queued": queued.contains(&t.id),
            })
        })
        .collect();
    json!({"tracks": tracks, "total": total})
}

/// How many tracks at the top of the Automix list Automix holds (playing,
/// or waiting on a deck); the phone can't move these or put others above.
fn held_count(automix: &AutoMix, browser: &Browser) -> usize {
    let (playing, waiting) = automix.held();
    (0..browser.automix_len())
        .map_while(|i| browser.automix_at(i))
        .take_while(|t| Some(t.id) == playing || Some(t.id) == waiting)
        .count()
}
