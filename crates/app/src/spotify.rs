//! Spotify browsing: sign in, then list the playlists and liked songs.
//!
//! Spotify's Web API gives track details but no audio, and its terms forbid
//! mixing Spotify audio, so a Spotify track plays only when the collection
//! holds the same track (see [`matches`]). DJ apps that stream Spotify do it
//! under a partner deal with Spotify.
//!
//! Sign-in uses OAuth with PKCE, which needs only a client ID: the user
//! registers an app at developer.spotify.com with [`REDIRECT_URI`] as its
//! redirect URI. The browser comes back to a one-shot server on 127.0.0.1.
//! The client ID and refresh token are kept in `spotify.json` in the config
//! folder ([`crate::settings::config_dir`]), so the next start signs in by itself.
//!
//! Network calls run on a worker thread; the UI sends [`Request`]s and polls
//! [`Event`]s.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub(crate) const PORT: u16 = 8898;
pub const REDIRECT_URI: &str = "http://127.0.0.1:8898/callback";
const SCOPES: &str = "playlist-read-private playlist-read-collaborative user-library-read";
const API: &str = "https://api.spotify.com/v1";
const ACCOUNTS: &str = "https://accounts.spotify.com";
/// How long to wait for the user to finish signing in in the browser.
pub(crate) const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// The key of the liked songs list, which is not a playlist in the API.
pub const LIKED: &str = "liked";

#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    /// Spotify's playlist id, or [`LIKED`].
    pub key: String,
    pub name: String,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamTrack {
    /// The service's track id; empty for the DJ's own local files.
    pub id: String,
    /// The track's page on the service, or a search for it.
    pub page: String,
    /// The recording's ISRC, when Spotify knows it.
    pub isrc: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration: f64,
}

impl StreamTrack {
    pub fn artist(&self) -> String {
        self.artists.join(", ")
    }

    /// What a match set by hand is saved under: Spotify's id, or for the
    /// DJ's own local files (which have none) its artist and title.
    pub fn match_key(&self) -> String {
        if self.id.is_empty() {
            format!(
                "local:{}|{}",
                normalize(&self.artist()),
                normalize(&self.title)
            )
        } else {
            self.id.clone()
        }
    }

    /// The track's page on Spotify, or a search for it when it has no id.
    fn spotify_page(id: &str, artist: &str, title: &str) -> String {
        if id.is_empty() {
            format!(
                "https://open.spotify.com/search/{}",
                encode(&format!("{artist} {title}"))
            )
        } else {
            format!("https://open.spotify.com/track/{id}")
        }
    }
}

pub enum Request {
    /// Sign in with this client ID, opening the browser.
    Connect(String),
    /// Fetch the tracks of a playlist (or [`LIKED`]).
    Tracks(String),
    Disconnect,
}

pub enum Event {
    /// Progress or an error to show while not connected.
    Status(String),
    Connected {
        user: String,
        playlists: Vec<Playlist>,
    },
    Tracks {
        key: String,
        tracks: Vec<StreamTrack>,
    },
    /// Fetching a playlist failed.
    TracksFailed {
        key: String,
        error: String,
    },
    Disconnected,
}

/// A streaming service's worker, as the UI talks to it.
pub struct Service {
    requests: Sender<Request>,
    events: Receiver<Event>,
    /// The client ID last used, to fill in the sign-in form.
    pub client_id: String,
}

impl Service {
    pub(crate) fn new(
        requests: Sender<Request>,
        events: Receiver<Event>,
        client_id: String,
    ) -> Self {
        Self {
            requests,
            events,
            client_id,
        }
    }

    /// Starts Spotify's worker. With a saved sign-in it reconnects straight away.
    pub fn spotify() -> Self {
        let (requests, rx) = channel();
        let (tx, events) = channel();
        // Without streaming, no sign-in from last time either.
        let saved = Saved::load(SAVED).filter(|_| crate::browser::STREAMING_ENABLED);
        let client_id = std::env::var("OPENDJ_SPOTIFY_CLIENT_ID")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| saved.as_ref().map(|s| s.client_id.clone()))
            .unwrap_or_default();
        std::thread::Builder::new()
            .name("spotify".into())
            .spawn(move || Worker::new(tx).run(rx, saved))
            .expect("spawn spotify thread");
        Self {
            requests,
            events,
            client_id,
        }
    }

    pub fn send(&self, r: Request) {
        let _ = self.requests.send(r);
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

/// Where Spotify's sign-in is kept, in the config folder.
const SAVED: &str = "spotify.json";

/// What is kept between runs, in a file of the config folder.
#[derive(Default)]
pub(crate) struct Saved {
    pub client_id: String,
    pub refresh_token: String,
    /// The app's client secret, for a service that needs one (SoundCloud).
    pub secret: String,
}

impl Saved {
    fn path(file: &str) -> Option<PathBuf> {
        crate::settings::config_dir().map(|d| d.join(file))
    }

    pub fn load(file: &str) -> Option<Self> {
        let v: Value = serde_json::from_slice(&std::fs::read(Self::path(file)?).ok()?).ok()?;
        Some(Self {
            client_id: v["client_id"].as_str()?.to_string(),
            refresh_token: v["refresh_token"].as_str().unwrap_or_default().to_string(),
            secret: v["secret"].as_str().unwrap_or_default().to_string(),
        })
    }

    pub fn store(&self, file: &str) {
        let Some(path) = Self::path(file) else { return };
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let mut body = json!({ "client_id": self.client_id, "refresh_token": self.refresh_token });
        if !self.secret.is_empty() {
            body["secret"] = json!(self.secret);
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        // The refresh token is a credential: readable by this user only.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        if let Ok(mut f) = options.open(&path) {
            let _ = f.write_all(body.to_string().as_bytes());
        }
    }
}

struct Session {
    client_id: String,
    access_token: String,
    refresh_token: String,
    expires: Instant,
}

struct Worker {
    agent: ureq::Agent,
    events: Sender<Event>,
    session: Option<Session>,
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
            session: None,
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    fn run(mut self, requests: Receiver<Request>, saved: Option<Saved>) {
        if let Some(saved) = saved.filter(|s| !s.refresh_token.is_empty()) {
            self.emit(Event::Status("Signing in…".into()));
            let result = self
                .refresh(&saved.client_id, &saved.refresh_token)
                .and_then(|s| {
                    self.session = Some(s);
                    self.connected()
                });
            if let Err(e) = result {
                self.session = None;
                self.emit(Event::Status(format!("Couldn't sign in again: {e}")));
            }
        }
        for request in requests {
            match request {
                Request::Connect(client_id) => {
                    let result = self.login(client_id.trim()).and_then(|s| {
                        self.session = Some(s);
                        self.connected()
                    });
                    if let Err(e) = result {
                        self.session = None;
                        self.emit(Event::Status(e));
                    }
                }
                Request::Tracks(key) => match self.tracks(&key) {
                    Ok(tracks) => self.emit(Event::Tracks { key, tracks }),
                    Err(error) => self.emit(Event::TracksFailed { key, error }),
                },
                Request::Disconnect => {
                    if let Some(s) = self.session.take() {
                        Saved {
                            client_id: s.client_id,
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

    /// Fetches the user's name and playlists and reports the connection.
    fn connected(&mut self) -> Result<(), String> {
        let me = self.get(&format!("{API}/me"))?;
        let user = me["display_name"]
            .as_str()
            .or(me["id"].as_str())
            .unwrap_or_default()
            .to_string();
        let liked = self.get(&format!("{API}/me/tracks?limit=1"))?;
        let mut playlists = vec![Playlist {
            key: LIKED.into(),
            name: "Liked Songs".into(),
            total: liked["total"].as_u64().unwrap_or(0) as usize,
        }];
        let items = self.get_all(&format!("{API}/me/playlists?limit=50"))?;
        playlists.extend(items.iter().filter_map(parse_playlist));
        self.emit(Event::Connected { user, playlists });
        Ok(())
    }

    fn tracks(&mut self, key: &str) -> Result<Vec<StreamTrack>, String> {
        let items = if key == LIKED {
            self.get_all(&format!("{API}/me/tracks?limit=50"))?
        } else {
            // Newer API versions call a playlist's tracks its items.
            match self.get_all(&format!("{API}/playlists/{key}/items?limit=100")) {
                Ok(items) => items,
                Err(_) => self.get_all(&format!("{API}/playlists/{key}/tracks?limit=100"))?,
            }
        };
        Ok(items.iter().filter_map(parse_item).collect())
    }

    fn login(&mut self, client_id: &str) -> Result<Session, String> {
        if client_id.is_empty() {
            return Err("Enter your Spotify app's client ID first.".into());
        }
        let listener = TcpListener::bind(("127.0.0.1", PORT))
            .map_err(|e| format!("Couldn't listen on port {PORT} for Spotify's reply: {e}"))?;
        let verifier = random_token(48);
        let state = random_token(12);
        let url = authorize_url(client_id, &code_challenge(&verifier), &state);
        if !open_in_browser(&url) {
            eprintln!("Open this address to sign in to Spotify: {url}");
        }
        self.emit(Event::Status("Finish signing in in your browser…".into()));
        let code = wait_for_code(&listener, &state, "Spotify")?;
        let response = self.token(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", client_id),
            ("code_verifier", &verifier),
        ])?;
        self.session_from(client_id, response, None)
    }

    fn refresh(&self, client_id: &str, refresh_token: &str) -> Result<Session, String> {
        let response = self.token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ])?;
        self.session_from(client_id, response, Some(refresh_token))
    }

    fn token(&self, form: &[(&str, &str)]) -> Result<Value, String> {
        let mut response = self
            .agent
            .post(format!("{ACCOUNTS}/api/token"))
            .send_form(form.iter().copied())
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body: Value = response
            .body_mut()
            .read_to_string()
            .ok()
            .and_then(|b| serde_json::from_str(&b).ok())
            .unwrap_or(Value::Null);
        if status != 200 {
            return Err(error_message(status, &body));
        }
        Ok(body)
    }

    /// Spotify may hand out a new refresh token; the old one stays valid otherwise.
    fn session_from(
        &self,
        client_id: &str,
        v: Value,
        old_refresh: Option<&str>,
    ) -> Result<Session, String> {
        let access_token = v["access_token"]
            .as_str()
            .ok_or("Spotify sent no access token")?
            .to_string();
        let refresh_token = v["refresh_token"]
            .as_str()
            .or(old_refresh)
            .unwrap_or_default()
            .to_string();
        let lifetime = v["expires_in"].as_u64().unwrap_or(3600);
        Saved {
            client_id: client_id.to_string(),
            refresh_token: refresh_token.clone(),
            ..Default::default()
        }
        .store(SAVED);
        Ok(Session {
            client_id: client_id.to_string(),
            access_token,
            refresh_token,
            expires: Instant::now() + Duration::from_secs(lifetime),
        })
    }

    fn renew(&mut self) -> Result<(), String> {
        let s = self.session.as_ref().ok_or("Not signed in to Spotify")?;
        let fresh = self.refresh(&s.client_id, &s.refresh_token)?;
        self.session = Some(fresh);
        Ok(())
    }

    /// A GET against the Web API, renewing the token and waiting out rate limits.
    fn get(&mut self, url: &str) -> Result<Value, String> {
        let mut renewed = false;
        for _ in 0..5 {
            let s = self.session.as_ref().ok_or("Not signed in to Spotify")?;
            if !renewed && s.expires < Instant::now() + Duration::from_secs(60) {
                self.renew()?;
                renewed = true;
                continue;
            }
            let mut response = self
                .agent
                .get(url)
                .header("Authorization", &format!("Bearer {}", s.access_token))
                .call()
                .map_err(|e| e.to_string())?;
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
                401 if !renewed => {
                    self.renew()?;
                    renewed = true;
                }
                429 => std::thread::sleep(Duration::from_secs(retry_after.unwrap_or(2).min(30))),
                _ => return Err(error_message(status, &body)),
            }
        }
        Err("Spotify kept refusing the request".into())
    }

    /// Follows a paged list to its end and returns all its items.
    fn get_all(&mut self, url: &str) -> Result<Vec<Value>, String> {
        let mut items = Vec::new();
        let mut next = Some(url.to_string());
        while let Some(url) = next {
            let page = self.get(&url)?;
            if let Some(a) = page["items"].as_array() {
                items.extend(a.iter().cloned());
            }
            next = page["next"].as_str().map(str::to_string);
        }
        Ok(items)
    }
}

fn error_message(status: u16, body: &Value) -> String {
    let message = body["error"]["message"]
        .as_str()
        .or(body["error_description"].as_str())
        .or(body["error"].as_str())
        .unwrap_or("no details");
    format!("Spotify said {status}: {message}")
}

fn parse_playlist(p: &Value) -> Option<Playlist> {
    let total = p["items"]["total"]
        .as_u64()
        .or(p["tracks"]["total"].as_u64())
        .unwrap_or(0);
    Some(Playlist {
        key: p["id"].as_str()?.to_string(),
        name: p["name"].as_str().unwrap_or_default().to_string(),
        total: total as usize,
    })
}

/// A playlist entry or liked song; episodes and missing tracks give `None`.
fn parse_item(item: &Value) -> Option<StreamTrack> {
    let t = if item["item"].is_object() {
        &item["item"]
    } else {
        &item["track"]
    };
    if !t.is_object() || t["type"].as_str().is_some_and(|k| k != "track") {
        return None;
    }
    let id = t["id"].as_str().unwrap_or_default().to_string();
    let title = t["name"].as_str()?.to_string();
    let artists: Vec<String> = t["artists"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(StreamTrack {
        page: StreamTrack::spotify_page(&id, &artists.join(", "), &title),
        id,
        isrc: t["external_ids"]["isrc"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        title,
        artists,
        album: t["album"]["name"].as_str().unwrap_or_default().to_string(),
        duration: t["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0,
    })
}

/// Lower-case letters and digits only, so "Song - Extended Mix" and
/// "Song (Extended Mix)" compare equal.
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// An ISRC as letters and digits in upper case, so "US-RC1-76-07839" and
/// "usrc17607839" compare equal. Empty unless it has the 12 characters.
pub fn normalize_isrc(s: &str) -> String {
    let s: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if s.len() == 12 {
        s
    } else {
        String::new()
    }
}

/// Whether a collection track with this (normalized) title and artist is the
/// Spotify track. Untagged files whose title is "Artist - Title" match too.
pub fn matches(track: &StreamTrack, title: &str, artist: &str) -> bool {
    let want = normalize(&track.title);
    if want.is_empty() {
        return false;
    }
    track.artists.iter().map(|a| normalize(a)).any(|a| {
        !a.is_empty()
            && ((title == want && artist.contains(&a))
                || (artist.is_empty() && title == format!("{a}{want}")))
    })
}

pub(crate) fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("random bytes");
    URL_SAFE_NO_PAD.encode(buf)
}

/// PKCE S256: the base64url SHA-256 of the verifier.
pub(crate) fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(crate) fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn authorize_url(client_id: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge),
        ("scope", SCOPES),
        ("state", state),
    ];
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect();
    format!("{ACCOUNTS}/authorize?{}", query.join("&"))
}

pub fn open_in_browser(url: &str) -> bool {
    if cfg!(windows) {
        // `start` would cut the URL at its first "&".
        return std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .is_ok();
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(url).spawn().is_ok()
}

/// The query parameters of the request line "GET /callback?a=b HTTP/1.1",
/// or `None` for any other path.
fn callback_query(request_line: &str) -> Option<Vec<(String, String)>> {
    let target = request_line.split_whitespace().nth(1)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/callback" {
        return None;
    }
    Some(
        query
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (decode(k), decode(v))
            })
            .collect(),
    )
}

/// Waits for the browser to come back with the authorization code.
pub(crate) fn wait_for_code(
    listener: &TcpListener,
    state: &str,
    service: &str,
) -> Result<String, String> {
    wait_for_code_after(listener, state, service, None)
}

/// [`wait_for_code`], serving `page` at `/start` first: a sign-in page of
/// openDJ's own, for a service whose sign-in runs in the browser.
pub(crate) fn wait_for_code_after(
    listener: &TcpListener,
    state: &str,
    service: &str,
    page: Option<&str>,
) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + LOGIN_TIMEOUT;
    while Instant::now() < deadline {
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let line = read_request_line(&mut stream).unwrap_or_default();
        if let Some(page) = page.filter(|_| line.starts_with("GET /start ")) {
            send_html(&mut stream, "200 OK", page);
            continue;
        }
        let Some(query) = callback_query(&line) else {
            respond(&mut stream, "404 Not Found", "");
            continue;
        };
        let param = |k: &str| query.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        if param("state").as_deref() != Some(state) {
            respond(
                &mut stream,
                "400 Bad Request",
                "This sign-in link is out of date. Try again from openDJ.",
            );
            continue;
        }
        if let Some(code) = param("code") {
            respond(
                &mut stream,
                "200 OK",
                &format!("Signed in to {service}. You can close this tab and go back to openDJ."),
            );
            return Ok(code);
        }
        let error = param("error").unwrap_or_default();
        respond(
            &mut stream,
            "200 OK",
            &format!("{service} sign-in was cancelled. You can close this tab."),
        );
        return Err(if error == "access_denied" {
            format!("{service} sign-in was cancelled.")
        } else {
            format!("{service} sign-in failed: {error}")
        });
    }
    Err(format!("{service} sign-in timed out."))
}

fn read_request_line(stream: &mut TcpStream) -> Option<String> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(2).any(|w| w == b"\r\n") && buf.len() < 16 * 1024 {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buf);
    text.lines().next().map(str::to_string)
}

fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>openDJ</title>\
         <body style=\"font:16px -apple-system,sans-serif;background:#1c1c1e;color:#eee;\
         display:grid;place-items:center;height:90vh\"><p>{message}</p>"
    );
    send_html(stream, status, &body);
}

fn send_html(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_base64url_sha256() {
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let t = random_token(48);
        assert_eq!(t.len(), 64);
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn builds_the_authorize_address() {
        let url = authorize_url("abc", "xyz", "s1");
        assert!(url.starts_with("https://accounts.spotify.com/authorize?client_id=abc&"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8898%2Fcallback"));
        assert!(url.contains(
            "scope=playlist-read-private%20playlist-read-collaborative%20user-library-read"
        ));
        assert!(url.contains("code_challenge=xyz&") && url.ends_with("state=s1"));
    }

    #[test]
    fn reads_the_callback() {
        let q = callback_query("GET /callback?code=AQ%2Bx_1&state=s1 HTTP/1.1").unwrap();
        assert_eq!(
            q,
            [
                ("code".into(), "AQ+x_1".into()),
                ("state".into(), "s1".into())
            ]
        );
        assert!(callback_query("GET /favicon.ico HTTP/1.1").is_none());
        assert_eq!(decode("a%2"), "a%2");
    }

    #[test]
    fn parses_playlist_entries_old_and_new() {
        let old = json!({ "track": {
            "type": "track", "name": "Strings of Life", "duration_ms": 401000,
            "artists": [{ "name": "Rhythim Is Rhythim" }], "album": { "name": "Innovator" } } });
        let new = json!({ "item": {
            "type": "track", "name": "Xpander", "duration_ms": 600000,
            "artists": [{ "name": "Sasha" }], "album": { "name": "Xpander EP" } } });
        let episode = json!({ "item": { "type": "episode", "name": "A podcast" } });
        let removed = json!({ "track": null });
        let t = parse_item(&old).unwrap();
        assert_eq!(
            (t.title.as_str(), t.artist(), t.duration),
            ("Strings of Life", "Rhythim Is Rhythim".to_string(), 401.0)
        );
        assert_eq!(parse_item(&new).unwrap().album, "Xpander EP");
        assert!(parse_item(&episode).is_none() && parse_item(&removed).is_none());
        let p = parse_playlist(&json!({ "id": "p1", "name": "Peak", "items": { "total": 42 } }))
            .unwrap();
        assert_eq!((p.key.as_str(), p.total), ("p1", 42));
        let p = parse_playlist(&json!({ "id": "p2", "name": "Old", "tracks": { "total": 7 } }))
            .unwrap();
        assert_eq!(p.total, 7);
    }

    #[test]
    fn matches_collection_tracks() {
        let sp = StreamTrack {
            id: String::new(),
            page: String::new(),
            isrc: String::new(),
            title: "Insomnia - Extended Mix".into(),
            artists: vec!["Faithless".into(), "Rollo".into()],
            album: String::new(),
            duration: 0.0,
        };
        let m = |title: &str, artist: &str| matches(&sp, &normalize(title), &normalize(artist));
        assert!(m("Insomnia (Extended Mix)", "Faithless"));
        assert!(m("Insomnia (Extended Mix)", "Rollo & Sister Bliss"));
        assert!(m("Faithless - Insomnia (Extended Mix)", ""));
        assert!(!m("Insomnia", "Faithless"));
        assert!(!m("Insomnia (Extended Mix)", "Someone Else"));
    }

    #[test]
    fn reads_track_ids_and_isrcs() {
        let t = parse_item(&json!({ "track": {
            "type": "track", "id": "abc", "name": "Insomnia",
            "artists": [{ "name": "Faithless" }],
            "external_ids": { "isrc": "gb-arl-96-00049" },
        }}))
        .unwrap();
        assert_eq!(t.page, "https://open.spotify.com/track/abc");
        assert_eq!(normalize_isrc(&t.isrc), "GBARL9600049");
        assert_eq!(normalize_isrc("not one"), "");
        assert_eq!(t.match_key(), "abc");
        let own = StreamTrack {
            id: String::new(),
            ..t
        };
        assert_eq!(own.match_key(), "local:faithless|insomnia");
    }
}
