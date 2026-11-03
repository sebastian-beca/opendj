//! Navidrome, and any other server that speaks the Subsonic API (Gonic,
//! Airsonic, Ampache, LMS), as a source in the rail: the DJ's own music
//! searched by album, artist, song or playlist, and the original files
//! downloaded into the collection.
//!
//! Calls go to `<server>/rest/<method>` with token authentication: the
//! password never travels, only md5(password + salt) with the salt. See
//! <https://www.subsonic.org/pages/api.jsp> and <https://opensubsonic.netlify.app>.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::server::{self, encode, matches, Results, Server, ServerSource, Song};
use crate::source::{Found, QueryKind, SearchSource, SourceInfo, SourceQuery, DOWNLOAD_FOLDER};
use crate::{Manifest, Plugin, PluginConfig, Setting, SettingKind};

const MANIFEST: Manifest = Manifest {
    id: "navidrome",
    name: "Navidrome",
    // The DJ's own server, at the address they fill in.
    hosts: &[],
    downloads: true,
};

/// The API version asked for: 1.16.1 is what Navidrome speaks.
const VERSION: &str = "1.16.1";
const CLIENT: &str = "openDJ";

/// How big list covers are fetched, in pixels.
const THUMB: u32 = 96;

/// Navidrome's mark: a disc with a note, as a generic rail icon.
const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><rect width="30" height="30" rx="7" fill="#1f5f99"/><circle cx="15" cy="15" r="9" fill="none" stroke="#e8f1fa" stroke-width="2"/><circle cx="15" cy="15" r="2.4" fill="#e8f1fa"/><path d="M19 7.5v6.2a2 2 0 1 1-1.2-1.8V7.5z" fill="#e8f1fa"/></svg>"##;

pub struct Navidrome {
    source: ServerSource<Subsonic>,
}

impl Default for Navidrome {
    fn default() -> Self {
        Self::new()
    }
}

impl Navidrome {
    pub fn new() -> Self {
        Self {
            source: ServerSource::new("navidrome", Subsonic::default()),
        }
    }
}

impl Plugin for Navidrome {
    fn manifest(&self) -> &Manifest {
        &MANIFEST
    }

    fn source(&mut self) -> Option<&mut dyn SearchSource> {
        Some(&mut self.source)
    }

    fn settings(&self) -> Vec<Setting> {
        vec![
            Setting::new(
                "",
                SettingKind::Note,
                "Your own music server",
                "Navidrome, or any server that speaks the Subsonic API (Gonic, Airsonic, \
                 Ampache, LMS). Searches open its albums, artists and playlists, and \
                 downloads bring the original files into your collection.",
            ),
            Setting::new(
                "server",
                SettingKind::Text,
                "Server address",
                "For example http://192.168.1.20:4533 or https://music.example.com",
            ),
            Setting::new("user", SettingKind::Text, "Username", ""),
            Setting::new(
                "password",
                SettingKind::Secret,
                "Password",
                "Sent only as a salted token, never as it is.",
            ),
            Setting::new(
                DOWNLOAD_FOLDER,
                SettingKind::Folder,
                "Download folder",
                "Left empty, downloads go to Downloaded/Navidrome in your music folder.",
            ),
        ]
    }

    fn configure(&mut self, config: &PluginConfig) {
        let s = &mut self.source.server;
        s.url = server::server(config.get("server"));
        s.user = config.get("user").trim().to_string();
        s.password = config.get("password").to_string();
        s.salt = salt();
    }
}

#[derive(Default)]
pub(crate) struct Subsonic {
    url: String,
    user: String,
    password: String,
    salt: String,
}

fn salt() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{:x}", md5::compute(nanos.to_le_bytes()))[..12].to_string()
}

impl Subsonic {
    /// The query that signs every call.
    fn auth(&self) -> String {
        let token = md5::compute(format!("{}{}", self.password, self.salt));
        format!(
            "u={}&t={token:x}&s={}&v={VERSION}&c={CLIENT}&f=json",
            encode(&self.user),
            self.salt
        )
    }

    fn url(&self, method: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!("{}/rest/{method}?{}", self.url, self.auth());
        for (k, v) in params {
            url.push_str(&format!("&{k}={}", encode(v)));
        }
        url
    }

    fn call(&self, method: &str, params: &[(&str, &str)]) -> Result<Value, String> {
        let mut response = server::agent()
            .get(&self.url(method, params))
            .call()
            .map_err(|e| format!("Couldn't reach the server: {e}"))?;
        let status = response.status().as_u16();
        let v: Value = response
            .body_mut()
            .read_json()
            .map_err(|_| format!("The server answered {status}, not as a Subsonic server."))?;
        answer(v)
    }

    fn song(&self, v: &Value) -> Option<Song> {
        let id = text(&v["id"]);
        if id.is_empty()
            || v["isDir"].as_bool() == Some(true)
            || v["isVideo"].as_bool() == Some(true)
        {
            return None;
        }
        let cover = text(&v["coverArt"]);
        Some(Song {
            key: format!("song:{id}"),
            title: text(&v["title"]),
            artist: text(&v["artist"]),
            album_artist: album_artist(v),
            album: text(&v["album"]),
            year: text(&v["year"]),
            number: text(&v["track"]),
            duration: v["duration"].as_f64().unwrap_or(0.0),
            suffix: text(&v["suffix"]),
            bitrate: v["bitRate"].as_u64().unwrap_or(0),
            size: v["size"].as_u64().unwrap_or(0),
            file_url: self.url("download", &[("id", &id)]),
            cover_url: self.cover(&cover, None),
            thumb_url: self.cover(&cover, Some(THUMB)),
        })
    }

    fn cover(&self, id: &str, size: Option<u32>) -> String {
        match (id.is_empty(), size) {
            (true, _) => String::new(),
            (false, Some(s)) => self.url("getCoverArt", &[("id", id), ("size", &s.to_string())]),
            (false, None) => self.url("getCoverArt", &[("id", id)]),
        }
    }

    fn album(&self, v: &Value) -> Option<Found> {
        let id = text(&v["id"]);
        if id.is_empty() {
            return None;
        }
        let name = match text(&v["name"]) {
            n if n.is_empty() => text(&v["title"]),
            n => n,
        };
        Some(Found {
            key: format!("album:{id}"),
            folder: true,
            artist: text(&v["artist"]),
            title: name.clone(),
            album: name,
            year: text(&v["year"]),
            duration: v["duration"].as_f64().unwrap_or(0.0),
            track_count: v["songCount"].as_u64().unwrap_or(0) as usize,
            from: MANIFEST.name.into(),
            cover_url: self.cover(&text(&v["coverArt"]), Some(THUMB)),
            ..Default::default()
        })
    }

    fn playlist(&self, v: &Value) -> Option<Found> {
        let id = text(&v["id"]);
        if id.is_empty() {
            return None;
        }
        let name = text(&v["name"]);
        Some(Found {
            key: format!("playlist:{id}"),
            folder: true,
            artist: text(&v["owner"]),
            title: name.clone(),
            album: name,
            duration: v["duration"].as_f64().unwrap_or(0.0),
            track_count: v["songCount"].as_u64().unwrap_or(0) as usize,
            from: "Playlist".into(),
            cover_url: self.cover(&text(&v["coverArt"]), Some(THUMB)),
            ..Default::default()
        })
    }

    fn search3(&self, text: &str, artists: u32, albums: u32, songs: u32) -> Result<Value, String> {
        let v = self.call(
            "search3",
            &[
                ("query", text),
                ("artistCount", &artists.to_string()),
                ("albumCount", &albums.to_string()),
                ("songCount", &songs.to_string()),
            ],
        )?;
        Ok(v["searchResult3"].clone())
    }
}

impl Server for Subsonic {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            name: MANIFEST.name,
            icon_svg: ICON,
            hint: "An artist, an album, a song, or a playlist's name",
        }
    }

    fn describe(&self, query: &SourceQuery) -> Vec<String> {
        let text = query.text.trim();
        let line = match query.kind {
            QueryKind::Playlist if matches("", text) => "All your playlists".to_string(),
            QueryKind::Playlist => format!("Playlists called {text}"),
            _ if text.is_empty() => return Vec::new(),
            QueryKind::Artist => format!("Albums by artists called {text}"),
            QueryKind::Album => format!("Albums called {text}"),
            QueryKind::Track => format!("Songs called {text}"),
            QueryKind::Auto => format!("Albums and songs matching {text}"),
        };
        vec![line, "Downloads are the original files.".into()]
    }

    fn missing(&self) -> Option<String> {
        (self.url.is_empty() || self.user.is_empty())
            .then(|| "Fill in your server in Settings › Navidrome first.".into())
    }

    fn search(&self, query: &SourceQuery) -> Result<Results, String> {
        let q = query.text.trim();
        let list = |v: &Value, key: &str| v[key].as_array().cloned().unwrap_or_default();
        let mut out = Results::default();
        match query.kind {
            QueryKind::Playlist => {
                let v = self.call("getPlaylists", &[])?;
                out.folders = list(&v["playlists"], "playlist")
                    .iter()
                    .filter(|p| matches(&text_of(p, "name"), q))
                    .filter_map(|p| self.playlist(p))
                    .collect();
            }
            QueryKind::Artist => {
                let found = self.search3(q, 5, 0, 0)?;
                for a in list(&found, "artist") {
                    let v = self.call("getArtist", &[("id", &text(&a["id"]))])?;
                    out.folders.extend(
                        list(&v["artist"], "album")
                            .iter()
                            .filter_map(|a| self.album(a)),
                    );
                }
            }
            kind => {
                let (albums, songs) = match kind {
                    QueryKind::Album => (100, 0),
                    QueryKind::Track => (0, 200),
                    _ => (30, 100),
                };
                let found = self.search3(q, 0, albums, songs)?;
                out.folders = list(&found, "album")
                    .iter()
                    .filter_map(|a| self.album(a))
                    .collect();
                out.songs = list(&found, "song")
                    .iter()
                    .filter_map(|s| self.song(s))
                    .collect();
            }
        }
        Ok(out)
    }

    fn folder(&self, key: &str) -> Result<Vec<Song>, String> {
        let (v, songs) = match key.split_once(':') {
            Some(("album", id)) => (
                self.call("getAlbum", &[("id", id)])?["album"].clone(),
                "song",
            ),
            Some(("playlist", id)) => (
                self.call("getPlaylist", &[("id", id)])?["playlist"].clone(),
                "entry",
            ),
            _ => return Err("Not a folder.".into()),
        };
        Ok(v[songs]
            .as_array()
            .map(|a| a.iter().filter_map(|s| self.song(s)).collect())
            .unwrap_or_default())
    }
}

/// The body of a Subsonic answer, or why it failed.
fn answer(v: Value) -> Result<Value, String> {
    let r = &v["subsonic-response"];
    match r["status"].as_str() {
        Some("ok") => Ok(r.clone()),
        Some(_) => Err(match r["error"]["code"].as_u64() {
            Some(40) => "The server didn't accept the username or password.".into(),
            Some(41) => "The server doesn't take tokens: turn on token sign-in or use another \
                         Subsonic server."
                .into(),
            _ => match text(&r["error"]["message"]) {
                m if m.is_empty() => "The server refused.".into(),
                m => m,
            },
        }),
        None => Err("That address isn't a Subsonic server.".into()),
    }
}

/// Who an album is by: OpenSubsonic's display album artist, else the
/// first of its album artists.
fn album_artist(v: &Value) -> String {
    match text(&v["displayAlbumArtist"]) {
        a if !a.is_empty() => a,
        _ => text(&v["albumArtists"][0]["name"]),
    }
}

fn text_of(v: &Value, key: &str) -> String {
    text(&v[key])
}

/// A value as text: numbers as written.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn server() -> Subsonic {
        Subsonic {
            url: "http://nd.lan:4533".into(),
            user: "dj".into(),
            password: "sesame".into(),
            salt: "c19b2d".into(),
        }
    }

    #[test]
    fn signs_with_a_salted_token() {
        let url = server().url("ping", &[("id", "a b")]);
        // md5("sesamec19b2d")
        let token = format!("{:x}", md5::compute("sesamec19b2d"));
        assert_eq!(
            url,
            format!(
                "http://nd.lan:4533/rest/ping?u=dj&t={token}&s=c19b2d&v=1.16.1&c=openDJ&f=json&id=a%20b"
            )
        );
        assert!(!url.contains("sesame&"));
    }

    #[test]
    fn reads_answers() {
        let ok = json!({ "subsonic-response": { "status": "ok", "version": "1.16.1" } });
        assert!(answer(ok).is_ok());
        let bad = json!({ "subsonic-response": { "status": "failed",
            "error": { "code": 40, "message": "Wrong username or password" } } });
        assert!(answer(bad).unwrap_err().contains("username or password"));
        assert!(answer(json!({ "hello": 1 })).is_err());
    }

    #[test]
    fn reads_songs_and_albums() {
        let s = server();
        let song = s
            .song(&json!({
                "id": "tr-1", "title": "Kerala", "artist": "Bonobo", "album": "Migration",
                "year": 2017, "track": 3, "duration": 251, "suffix": "flac",
                "bitRate": 1011, "size": 31000000, "coverArt": "al-9",
                "displayAlbumArtist": "Bonobo"
            }))
            .unwrap();
        assert_eq!(song.key, "song:tr-1");
        assert_eq!((song.year.as_str(), song.number.as_str()), ("2017", "3"));
        assert!(song.file_url.contains("/rest/download?"));
        assert!(song.file_url.ends_with("&id=tr-1"));
        assert!(song.thumb_url.ends_with("&id=al-9&size=96"));
        let f = song.found("Navidrome");
        assert_eq!(
            (f.format.as_str(), f.quality.as_str()),
            ("FLAC", "1011 kbps")
        );
        let files = song.files(std::path::Path::new("/dl"));
        assert_eq!(files.len(), 2);
        assert!(files[0].cover);
        assert_eq!(
            files[1].to,
            std::path::PathBuf::from("/dl/Bonobo/Migration/03 Kerala.flac")
        );
        assert!(s.song(&json!({ "id": "d", "isDir": true })).is_none());

        let album = s
            .album(
                &json!({ "id": "al-9", "name": "Migration", "artist": "Bonobo",
                            "songCount": 12, "year": 2017, "coverArt": "al-9" }),
            )
            .unwrap();
        assert!(album.folder);
        assert_eq!(album.key, "album:al-9");
        assert_eq!(album.track_count, 12);
    }

    #[test]
    fn needs_a_server() {
        assert!(Subsonic::default().missing().is_some());
        assert!(server().missing().is_none());
    }
}
