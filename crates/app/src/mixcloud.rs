//! Mixcloud browsing: a DJ's shows, favourites, listening history and
//! playlists, by their Mixcloud username.
//!
//! Mixcloud's API is open to read without a key, but has no audio: a show
//! the collection doesn't hold opens on mixcloud.com to play there. Lists
//! are `https://api.mixcloud.com/<user>/<list>/`, paged by `paging.next`.
//!
//! The username is kept in `mixcloud.json` in the config folder. The worker
//! speaks the same [`Request`]s and [`Event`]s as Spotify's.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use serde_json::Value;

use crate::spotify::{encode, Event, Playlist, Request, Saved, Service, StreamTrack};

const API: &str = "https://api.mixcloud.com";
/// Where the username is kept, in the config folder.
const SAVED: &str = "mixcloud.json";
/// The most shows a list reads, newest first.
const MAX_SHOWS: usize = 500;

/// The lists every DJ has, by key and name.
const LISTS: [(&str, &str); 3] = [
    ("cloudcasts", "Shows"),
    ("favorites", "Favourites"),
    ("listens", "Listening History"),
];

/// Starts Mixcloud's worker. With a saved username it connects straight away.
pub fn start() -> Service {
    let (requests, rx) = channel();
    let (tx, events) = channel();
    let saved = Saved::load(SAVED).filter(|_| crate::browser::STREAMING_ENABLED);
    let user = saved
        .as_ref()
        .map(|s| s.client_id.clone())
        .unwrap_or_default();
    std::thread::Builder::new()
        .name("mixcloud".into())
        .spawn(move || Worker::new(tx).run(rx, saved))
        .expect("spawn mixcloud thread");
    Service::new(requests, events, user)
}

struct Worker {
    agent: ureq::Agent,
    events: Sender<Event>,
    /// The username signed in with.
    user: Option<String>,
}

impl Worker {
    fn new(events: Sender<Event>) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            events,
            user: None,
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn run(mut self, requests: Receiver<Request>, saved: Option<Saved>) {
        // A username kept from last time; "signed out" keeps none.
        if let Some(saved) = saved.filter(|s| !s.refresh_token.is_empty()) {
            self.emit(Event::Status("Connecting…".into()));
            if let Err(e) = self.connect(&saved.client_id) {
                // Kept for a refresh, so lists kept from last time can be
                // fetched once Mixcloud answers again.
                self.user = Some(saved.client_id.clone());
                self.emit(Event::SignInFailed(e));
            }
        }
        for request in requests {
            match request {
                Request::Connect(user) => {
                    self.emit(Event::Status("Connecting…".into()));
                    if let Err(e) = self.connect(&username(&user)) {
                        self.user = None;
                        self.emit(Event::Status(e));
                    }
                }
                Request::Tracks(key) => match self.shows(&key) {
                    Ok(tracks) => self.emit(Event::Tracks { key, tracks }),
                    Err(error) => self.emit(Event::TracksFailed { key, error }),
                },
                // Only playlists' tracks are fetched from here.
                Request::Lookup(_) => {}
                Request::Playlists => {
                    let result = match self.user.clone() {
                        Some(user) => self.connect(&user),
                        None => Err("Not connected".into()),
                    };
                    if let Err(e) = result {
                        self.emit(Event::SignInFailed(format!("Couldn't refresh: {e}")));
                    }
                }
                Request::Disconnect => {
                    if let Some(user) = self.user.take() {
                        Saved {
                            client_id: user,
                            refresh_token: String::new(),
                            ..Default::default()
                        }
                        .store(SAVED);
                    }
                    self.emit(Event::Disconnected);
                }
            }
        }
    }

    /// Checks the user, lists their lists and playlists and reports the
    /// connection.
    fn connect(&mut self, user: &str) -> Result<(), String> {
        if user.is_empty() {
            return Err("Enter your Mixcloud username first.".into());
        }
        let me = self.get(&format!("{API}/{}/", encode(user)))?;
        let name = me["name"]
            .as_str()
            .or(me["username"].as_str())
            .unwrap_or(user)
            .to_string();
        let mut playlists: Vec<Playlist> = LISTS
            .iter()
            .map(|(key, name)| Playlist {
                key: key.to_string(),
                name: name.to_string(),
                ..Default::default()
            })
            .collect();
        let own = self.get_all(&format!("{API}/{}/playlists/", encode(user)), MAX_SHOWS)?;
        playlists.extend(own.iter().filter_map(parse_playlist));
        // Remembered for next time; the "token" only says it's signed in.
        Saved {
            client_id: user.to_string(),
            refresh_token: "username".into(),
            ..Default::default()
        }
        .store(SAVED);
        self.user = Some(user.to_string());
        self.emit(Event::Connected {
            user: name,
            playlists,
        });
        Ok(())
    }

    /// The shows of a list, or of a playlist (whose key is its API path).
    fn shows(&mut self, key: &str) -> Result<Vec<StreamTrack>, String> {
        let user = self.user.clone().ok_or("Not connected to Mixcloud")?;
        let url = if key.starts_with('/') {
            format!("{API}{key}cloudcasts/")
        } else {
            format!("{API}/{}/{key}/", encode(&user))
        };
        let items = self.get_all(&url, MAX_SHOWS)?;
        Ok(items.iter().filter_map(parse_show).collect())
    }

    /// A GET against the API, waiting out rate limits.
    fn get(&mut self, url: &str) -> Result<Value, String> {
        for _ in 0..5 {
            let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let body: Value = response
                .body_mut()
                .read_to_string()
                .ok()
                .and_then(|b| serde_json::from_str(&b).ok())
                .unwrap_or(Value::Null);
            match status {
                200 => return Ok(body),
                // Mixcloud's rate limit answers 403 with a wait.
                403 if retry_after.is_some() => {
                    std::thread::sleep(Duration::from_secs(retry_after.unwrap_or(2).min(30)))
                }
                404 => return Err("Mixcloud doesn't know that username.".into()),
                _ => {
                    let detail = body["error"]["message"].as_str().unwrap_or_default();
                    return Err(if detail.is_empty() {
                        format!("Mixcloud answered {status}")
                    } else {
                        format!("Mixcloud said: {detail}")
                    });
                }
            }
        }
        Err("Mixcloud kept refusing the request".into())
    }

    /// Follows `paging.next` until the end or `max` items.
    fn get_all(&mut self, url: &str, max: usize) -> Result<Vec<Value>, String> {
        let mut items = Vec::new();
        let mut next = Some(format!("{url}?limit=100"));
        while let Some(url) = next {
            let page = self.get(&url)?;
            let data = page["data"].as_array().cloned().unwrap_or_default();
            if data.is_empty() {
                break;
            }
            items.extend(data);
            if items.len() >= max {
                items.truncate(max);
                break;
            }
            next = page["paging"]["next"].as_str().map(str::to_string);
        }
        Ok(items)
    }
}

/// A username as typed, or from a link to the DJ's page.
fn username(typed: &str) -> String {
    let t = typed.trim().trim_end_matches('/');
    t.rsplit('/')
        .next()
        .unwrap_or(t)
        .trim_start_matches('@')
        .to_string()
}

fn parse_playlist(v: &Value) -> Option<Playlist> {
    Some(Playlist {
        key: v["key"].as_str()?.to_string(),
        name: v["name"].as_str().unwrap_or("Playlist").to_string(),
        total: 0,
        version: v["updated_time"].as_str().unwrap_or_default().to_string(),
    })
}

/// A show as a list row: its name, who made it and how long it is, with its
/// page on mixcloud.com to play it there.
fn parse_show(v: &Value) -> Option<StreamTrack> {
    let key = v["key"].as_str()?.to_string();
    let user = &v["user"];
    Some(StreamTrack {
        page: v["url"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://www.mixcloud.com{key}")),
        isrc: String::new(),
        title: v["name"].as_str().unwrap_or_default().to_string(),
        artists: user["name"]
            .as_str()
            .or(user["username"].as_str())
            .map(|n| vec![n.to_string()])
            .unwrap_or_default(),
        album: v["created_time"]
            .as_str()
            .map(|t| t.chars().take(10).collect())
            .unwrap_or_default(),
        duration: v["audio_length"].as_f64().unwrap_or(0.0),
        id: key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usernames_from_links() {
        assert_eq!(username(" spartacus "), "spartacus");
        assert_eq!(username("https://www.mixcloud.com/spartacus/"), "spartacus");
        assert_eq!(username("@spartacus"), "spartacus");
    }

    #[test]
    fn reads_shows_and_playlists() {
        let show = parse_show(&json!({
            "key": "/spartacus/party-time/", "url": "https://www.mixcloud.com/spartacus/party-time/",
            "name": "Party Time", "audio_length": 3600, "created_time": "2011-08-26T13:47:41Z",
            "user": { "username": "spartacus", "name": "Spartacus" }
        }))
        .unwrap();
        assert_eq!(
            (show.title.as_str(), show.artist()),
            ("Party Time", "Spartacus".into())
        );
        assert_eq!((show.duration, show.album.as_str()), (3600.0, "2011-08-26"));
        assert_eq!(show.page, "https://www.mixcloud.com/spartacus/party-time/");
        assert_eq!(show.match_key(), "/spartacus/party-time/");
        let p =
            parse_playlist(&json!({ "key": "/spartacus/playlists/warm-up/", "name": "Warm up" }))
                .unwrap();
        assert_eq!(
            (p.key.as_str(), p.name.as_str()),
            ("/spartacus/playlists/warm-up/", "Warm up")
        );
    }
}
