//! Where things go on the stick: the audio under `/Contents/<artist>/<album>/`,
//! each track's analysis files in the `USBANLZ` folder the players work out
//! from the audio path, and the artwork thumbnails.
//!
//! The naming rules and the path hash follow Bake'n Deck's `expressport`
//! (MIT licence, Copyright (c) Masanari Higashi), which checked them against
//! every path of real rekordbox 7 exports.

use std::collections::{HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;

/// rekordbox cuts every name on the stick at this many characters: an artist
/// or album folder, and a file name with its extension and `-N` suffix.
const MAX_NAME: usize = 48;

/// Characters FAT32 and exFAT don't allow in names.
fn replace_forbidden(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect()
}

fn truncate_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// An artist or album folder: no space at either end, even after the cut,
/// and a dot at the end written as `_`, as Windows would drop it.
fn folder(name: &str, fallback: &str) -> String {
    let name = replace_forbidden(name.nfc().collect::<String>().trim());
    if name.is_empty() {
        return fallback.to_string();
    }
    let cut = truncate_chars(&name, MAX_NAME).trim_end();
    let kept = cut.trim_end_matches('.');
    format!("{kept}{}", "_".repeat(cut.len() - kept.len()))
}

/// A file name, cut so that stem, `suffix` and extension fit in [`MAX_NAME`].
fn file(name: &str, suffix: &str) -> String {
    let name: String = name.nfc().collect();
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name.as_str(), ""),
    };
    let stem = if stem.trim().is_empty() {
        "track"
    } else {
        stem
    };
    let ext = replace_forbidden(ext);
    let room = MAX_NAME.saturating_sub(ext.chars().count() + suffix.chars().count());
    format!(
        "{}{suffix}{ext}",
        truncate_chars(&replace_forbidden(stem), room)
    )
}

/// `/Contents/<Artist>/<Album>/<file>` as rekordbox lays a stick out.
pub fn stick_path(artist: &str, album: &str, file_name: &str, suffix: &str) -> String {
    format!(
        "/Contents/{}/{}/{}",
        folder(artist, "UnknownArtist"),
        folder(album, "UnknownAlbum"),
        file(file_name, suffix)
    )
}

/// Hands out stick paths, telling apart files that would share one with a
/// `-1`, `-2` suffix. FAT32 ignores case, so neither does this.
#[derive(Debug, Default)]
pub struct Paths {
    used: HashSet<String>,
}

impl Paths {
    pub fn assign(&mut self, artist: &str, album: &str, file_name: &str) -> String {
        let mut candidate = stick_path(artist, album, file_name, "");
        let mut n = 1;
        while !self.used.insert(candidate.to_lowercase()) {
            candidate = stick_path(artist, album, file_name, &format!("-{n}"));
            n += 1;
        }
        candidate
    }
}

/// `(P, hash)` for a stick path such as `/Contents/Artist/Album/x.flac`.
/// Players ignore the analysis path the database gives and work it out from
/// the audio path with this hash, which was recovered from rekordbox.
fn anlz_hash(stick_path: &str) -> (u32, u32) {
    let mut h: u32 = 0;
    for unit in stick_path.encode_utf16() {
        let c = unit as u32;
        h = h.wrapping_mul(0x5BC9).wrapping_add(c);
        h = h.wrapping_mul(0x93B5).wrapping_add(c);
    }
    let r = h % 200003;
    let p = (r & 1)
        | ((r >> 1) & 2)
        | ((r >> 4) & 4)
        | ((r >> 4) & 8)
        | ((r >> 5) & 0x10)
        | ((r >> 8) & 0x20)
        | ((r >> 10) & 0x40);
    (p, r)
}

/// The folder that holds a track's analysis files.
fn anlz_dir(stick_path: &str) -> String {
    let (p, r) = anlz_hash(stick_path);
    format!("/PIONEER/USBANLZ/P{p:03X}/{r:08X}")
}

/// Hands out analysis file stems (`.../ANLZ0000`). Two tracks can hash to
/// the same folder; rekordbox then numbers the second `ANLZ0001`.
#[derive(Debug, Default)]
pub struct AnlzSlots(HashMap<String, u16>);

impl AnlzSlots {
    pub fn assign(&mut self, stick_path: &str) -> String {
        let dir = anlz_dir(stick_path);
        let taken = self.0.entry(dir.clone()).or_default();
        let n = *taken;
        *taken += 1;
        format!("{dir}/ANLZ{n:04}")
    }
}

/// `path` (`/PIONEER/...`) under the stick's export folder, which is
/// `.PIONEER` on sticks rekordbox wrote hidden.
pub fn rooted(path: &str, export_dir: &str) -> String {
    match path.strip_prefix("/PIONEER/") {
        Some(rest) => format!("/{export_dir}/{rest}"),
        None => path.to_string(),
    }
}

/// `/PIONEER/Artwork/00001/a1.jpg` and its 240-pixel `_m` sibling. Folders
/// hold twenty images. `export.pdb` names the `a` files and OneLibrary the
/// `b` ones, which hold the same pictures.
pub fn artwork(id: u32, copy: char, medium: bool) -> String {
    let size = if medium { "_m" } else { "" };
    format!("/PIONEER/Artwork/{:05}/{copy}{id}{size}.jpg", id / 20 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_folders_match_rekordbox() {
        assert_eq!(
            anlz_dir("/Contents/XamarA/UnknownAlbum/Unreal.mp3"),
            "/PIONEER/USBANLZ/P060/00012938"
        );
        assert_eq!(
            anlz_dir(
                "/Contents/Szeir/Stolperfalle EP/Szeir - Stolperfalle EP - 01 Oort Cloud.flac"
            ),
            "/PIONEER/USBANLZ/P074/0001B742"
        );
        // Two more, from rbxport's reference (checked on CDJ-3000 firmware).
        assert_eq!(
            anlz_dir("/Contents/A/B/one.mp3"),
            "/PIONEER/USBANLZ/P03B/0002F797"
        );
        assert_eq!(
            anlz_dir("/Contents/Hosanna, Westend/Drum Death - Extended Mix/20130688_drum_death_(extended_mix).mp3"),
            "/PIONEER/USBANLZ/P002/0002583E"
        );
    }

    #[test]
    fn a_second_track_in_a_folder_gets_the_next_number() {
        let mut slots = AnlzSlots::default();
        let a = slots.assign("/Contents/A/B/one.mp3");
        let b = slots.assign("/Contents/A/B/one.mp3");
        assert!(a.ends_with("ANLZ0000"));
        assert!(b.ends_with("ANLZ0001"));
    }

    #[test]
    fn names_are_safe_and_cut() {
        assert_eq!(
            stick_path("AC/DC", "", "Back: In Black?.mp3", ""),
            "/Contents/AC_DC/UnknownAlbum/Back_ In Black_.mp3"
        );
        let long = "x".repeat(60);
        let p = stick_path(&long, "turan.", &format!("{long}.flac"), "-1");
        let parts: Vec<&str> = p.split('/').collect();
        assert_eq!(parts[2].chars().count(), 48);
        assert_eq!(parts[3], "turan_");
        assert_eq!(parts[4].chars().count(), 48);
        assert!(parts[4].ends_with("-1.flac"));
    }

    #[test]
    fn clashing_paths_get_a_suffix() {
        let mut paths = Paths::default();
        assert_eq!(paths.assign("A", "B", "x.mp3"), "/Contents/A/B/x.mp3");
        assert_eq!(paths.assign("a", "b", "X.mp3"), "/Contents/a/b/X-1.mp3");
    }

    #[test]
    fn artwork_paths_follow_rekordbox() {
        assert_eq!(artwork(1, 'a', false), "/PIONEER/Artwork/00001/a1.jpg");
        assert_eq!(artwork(20, 'b', false), "/PIONEER/Artwork/00002/b20.jpg");
        assert_eq!(artwork(39, 'a', true), "/PIONEER/Artwork/00002/a39_m.jpg");
        assert_eq!(
            rooted(&artwork(1, 'a', false), ".PIONEER"),
            "/.PIONEER/Artwork/00001/a1.jpg"
        );
    }
}
