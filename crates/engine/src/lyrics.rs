//! A track's lyrics: plain text, timed by line, or timed by word.
//!
//! Lyrics are read from what the DJ already has: an `.lrc` file beside the
//! track (plain LRC, or enhanced LRC with a time before each word), or the
//! file's own tags (ID3 `SYLT` timed lyrics, ID3 `USLT`, Vorbis `LYRICS`,
//! MP4 `©lyr`, which often hold LRC text too). Nothing is fetched here.

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

/// One word, or one syllable, with the time it is sung from.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    /// Seconds into the track.
    pub start: f64,
    /// As written, with any space after it, so the words joined make the line.
    pub text: String,
}

/// One line of the lyrics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    /// Seconds into the track the line starts; None in plain lyrics.
    pub start: Option<f64>,
    pub text: String,
    /// Each word's time, when the lyrics have them.
    pub words: Vec<Word>,
    /// When the last word ends, when the lyrics say (enhanced LRC can close
    /// a line with a last time and no word).
    pub end: Option<f64>,
}

/// How finely lyrics are timed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Timing {
    /// No times: words only.
    Plain,
    /// A time for each line.
    Lines,
    /// A time for each word.
    Words,
}

/// Where lyrics were read from.
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    /// An `.lrc` file beside the track.
    Lrc(PathBuf),
    /// The track's own tags.
    Tag,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lyrics {
    pub lines: Vec<Line>,
    pub origin: Origin,
}

impl Lyrics {
    pub fn timing(&self) -> Timing {
        timing_of(&self.lines)
    }

    /// The line sung at `seconds`: the last one started by then. None
    /// before the first line, and for plain lyrics.
    pub fn line_at(&self, seconds: f64) -> Option<usize> {
        line_at(&self.lines, seconds)
    }

    /// The lyrics as LRC text: enhanced LRC when they are timed by word.
    pub fn to_lrc(&self) -> String {
        to_lrc(&self.lines)
    }
}

fn timing_of(lines: &[Line]) -> Timing {
    if lines.iter().any(|l| !l.words.is_empty()) {
        Timing::Words
    } else if lines.iter().any(|l| l.start.is_some()) {
        Timing::Lines
    } else {
        Timing::Plain
    }
}

/// The last line started by `seconds`, in lines sorted by start.
pub fn line_at(lines: &[Line], seconds: f64) -> Option<usize> {
    let timed = lines.partition_point(|l| l.start.is_some_and(|s| s <= seconds));
    if timed == 0 || lines[timed - 1].start.is_none() {
        None
    } else {
        Some(timed - 1)
    }
}

/// `[mm:ss.xx]` for a time in seconds.
fn stamp(seconds: f64) -> String {
    let cs = (seconds.max(0.0) * 100.0).round() as u64;
    format!("{:02}:{:02}.{:02}", cs / 6000, (cs / 100) % 60, cs % 100)
}

/// Lines as LRC text.
pub fn to_lrc(lines: &[Line]) -> String {
    let mut out = String::new();
    for l in lines {
        match l.start {
            Some(s) => {
                out.push('[');
                out.push_str(&stamp(s));
                out.push(']');
                if l.words.is_empty() {
                    out.push_str(&l.text);
                } else {
                    for w in &l.words {
                        out.push('<');
                        out.push_str(&stamp(w.start));
                        out.push('>');
                        out.push_str(&w.text);
                    }
                    if let Some(end) = l.end {
                        out.push('<');
                        out.push_str(&stamp(end));
                        out.push('>');
                    }
                }
            }
            None => out.push_str(&l.text),
        }
        out.push('\n');
    }
    out
}

/// A time as LRC writes it: `mm:ss`, `mm:ss.xx`, `mm:ss.xxx`, `mm:ss:xx`
/// or `hh:mm:ss.xx`. None for anything else, such as an `[ar:…]` tag.
fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || !s.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    let number = |p: &str| -> Option<f64> {
        (!p.is_empty() && p.chars().all(|c| c.is_ascii_digit() || c == '.'))
            .then(|| p.parse().ok())
            .flatten()
    };
    let t = match parts.as_slice() {
        [m, s] => number(m)? * 60.0 + number(s)?,
        // mm:ss:xx (hundredths after a colon), or hh:mm:ss(.xx).
        // mm:ss:xx, hundredths after a colon, as some writers do.
        [m, s, cs] if !cs.contains('.') && cs.len() == 2 && !s.contains('.') => {
            number(m)? * 60.0 + number(s)? + number(cs)? / 100.0
        }
        [h, m, s] => number(h)? * 3600.0 + number(m)? * 60.0 + number(s)?,
        _ => return None,
    };
    t.is_finite().then_some(t)
}

/// The `[…]` stamps at the start of an LRC line: their times, any
/// `[tag:value]` instead, and the rest of the line.
fn leading_stamps(line: &str) -> (Vec<f64>, Option<(String, String)>, &str) {
    let mut times = Vec::new();
    let mut rest = line.trim_start();
    while let Some(after) = rest.strip_prefix('[') {
        let Some(close) = after.find(']') else {
            break;
        };
        let inside = &after[..close];
        match parse_time(inside) {
            Some(t) => times.push(t),
            None if times.is_empty() => {
                if let Some((k, v)) = inside.split_once(':') {
                    let k = k.trim();
                    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphabetic() || c == '#') {
                        return (
                            times,
                            Some((k.to_ascii_lowercase(), v.trim().to_string())),
                            "",
                        );
                    }
                }
                break;
            }
            None => break,
        }
        rest = &after[close + 1..];
    }
    (times, None, rest)
}

/// The words of an enhanced LRC line (`<mm:ss.xx>word <mm:ss.xx>word`),
/// with the text before the first stamp and a closing time with no word.
fn word_stamps(text: &str) -> Option<(String, Vec<Word>, Option<f64>)> {
    if !text.contains('<') {
        return None;
    }
    let mut lead = String::new();
    let mut words: Vec<Word> = Vec::new();
    let mut rest = text;
    let mut any = false;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>').map(|c| open + c) else {
            break;
        };
        let Some(t) = parse_time(&rest[open + 1..close]) else {
            // Not a stamp: keep it as text.
            let piece = &rest[..=close];
            match words.last_mut() {
                Some(w) => w.text.push_str(piece),
                None => lead.push_str(piece),
            }
            rest = &rest[close + 1..];
            continue;
        };
        any = true;
        let before = &rest[..open];
        match words.last_mut() {
            Some(w) => w.text.push_str(before),
            None => lead.push_str(before),
        }
        words.push(Word {
            start: t,
            text: String::new(),
        });
        rest = &rest[close + 1..];
    }
    if !any {
        return None;
    }
    match words.last_mut() {
        Some(w) => w.text.push_str(rest),
        None => lead.push_str(rest),
    }
    // A last stamp with nothing after it closes the line.
    let mut end = None;
    if words.last().is_some_and(|w| w.text.trim().is_empty()) && words.len() > 1 {
        end = words.pop().map(|w| w.start);
    }
    // Spaces belong after a word: one written before the next stamp is
    // already there; a space at the start of a word moves to the one before.
    for i in 1..words.len() {
        let trimmed = words[i].text.trim_start().len();
        let pad = words[i].text.len() - trimmed;
        if pad > 0 {
            let space: String = words[i].text.drain(..pad).collect();
            words[i - 1].text.push_str(&space);
        }
    }
    Some((lead, words, end))
}

/// Text decoded from a file's bytes: UTF-8 (with or without a BOM),
/// UTF-16 with a BOM, else Latin-1.
pub fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest);
    }
    let utf16 = |rest: &[u8], le: bool| -> String {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Cow::Owned(utf16(rest, true));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Cow::Owned(utf16(rest, false));
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(bytes.iter().map(|&b| b as char).collect()),
    }
}

/// Lyrics from text: LRC when any line has a time, else plain lines.
/// None when there are no words at all.
pub fn parse(text: &str) -> Option<Vec<Line>> {
    let text = text.trim_start_matches('\u{feff}');
    let mut offset = 0.0;
    let mut timed: Vec<Line> = Vec::new();
    let mut plain: Vec<String> = Vec::new();
    for raw in text.lines() {
        let raw = raw.trim_end_matches('\r');
        let (times, tag, rest) = leading_stamps(raw);
        if let Some((key, value)) = tag {
            if key == "offset" {
                if let Ok(ms) = value.trim_start_matches('+').parse::<f64>() {
                    offset = ms / 1000.0;
                }
            }
            continue;
        }
        if times.is_empty() {
            plain.push(raw.trim().to_string());
            continue;
        }
        let (text, words, end) = match word_stamps(rest) {
            Some((lead, words, end)) => {
                let joined: String = std::iter::once(lead.as_str())
                    .chain(words.iter().map(|w| w.text.as_str()))
                    .collect();
                (joined.trim().to_string(), words, end)
            }
            None => (rest.trim().to_string(), Vec::new(), None),
        };
        // A line with several stamps is sung at each of them; word times
        // move with the copy.
        let first = times[0];
        for &t in &times {
            let shift = t - first;
            timed.push(Line {
                start: Some(t),
                text: text.clone(),
                words: words
                    .iter()
                    .map(|w| Word {
                        start: w.start + shift,
                        text: w.text.clone(),
                    })
                    .collect(),
                end: end.map(|e| e + shift),
            });
        }
    }
    if !timed.is_empty() {
        // A positive offset shows the lyrics sooner.
        for l in &mut timed {
            l.start = l.start.map(|s| (s - offset).max(0.0));
            l.end = l.end.map(|e| (e - offset).max(0.0));
            for w in &mut l.words {
                w.start = (w.start - offset).max(0.0);
            }
        }
        timed.sort_by(|a, b| a.start.unwrap_or(0.0).total_cmp(&b.start.unwrap_or(0.0)));
        if timed.iter().all(|l| l.text.is_empty()) {
            return None;
        }
        // Blank lines at the very start say nothing.
        while timed.first().is_some_and(|l| l.text.is_empty()) {
            timed.remove(0);
        }
        return Some(timed);
    }
    // Plain: blank lines kept as one gap between verses, none at the ends.
    let mut lines: Vec<Line> = Vec::new();
    for t in plain {
        if t.is_empty() && lines.last().is_none_or(|l| l.text.is_empty()) {
            continue;
        }
        lines.push(Line {
            text: t,
            ..Line::default()
        });
    }
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    (!lines.is_empty()).then_some(lines)
}

/// Lines from ID3 `SYLT` entries in milliseconds. Entries that start with
/// a line break begin a new line, and the others are its words; with no
/// line breaks at all each entry is a line.
pub fn from_synced(entries: &[(u32, String)]) -> Option<Vec<Line>> {
    let entries: Vec<(f64, &str)> = entries
        .iter()
        .map(|(ms, t)| (*ms as f64 / 1000.0, t.as_str()))
        .collect();
    let by_word = entries
        .iter()
        .skip(1)
        .any(|(_, t)| t.starts_with('\n') || t.starts_with('\r'));
    let mut lines: Vec<Line> = Vec::new();
    for (start, text) in entries {
        if !by_word {
            let text = text.trim();
            lines.push(Line {
                start: Some(start),
                text: text.to_string(),
                ..Line::default()
            });
            continue;
        }
        let new_line = text.starts_with('\n') || text.starts_with('\r') || lines.is_empty();
        let word = text.trim_start_matches(['\r', '\n']);
        if new_line {
            lines.push(Line {
                start: Some(start),
                ..Line::default()
            });
        }
        let line = lines.last_mut().expect("a line");
        line.words.push(Word {
            start,
            text: word.to_string(),
        });
        line.text.push_str(word);
    }
    for l in &mut lines {
        l.text = l.text.trim().to_string();
        // One word for the whole line times nothing finer than the line.
        if l.words.len() == 1 {
            l.words.clear();
        }
    }
    lines.sort_by(|a, b| a.start.unwrap_or(0.0).total_cmp(&b.start.unwrap_or(0.0)));
    lines.retain(|l| !l.text.is_empty() || l.start.is_some());
    (lines.iter().any(|l| !l.text.is_empty())).then_some(lines)
}

/// The `.lrc` files that would go with `track`: `name.lrc`, then
/// `name.mp3.lrc`.
pub fn sidecars(track: &Path) -> Vec<PathBuf> {
    let mut out = vec![track.with_extension("lrc")];
    if let Some(name) = track.file_name() {
        let mut n = name.to_os_string();
        n.push(".lrc");
        out.push(track.with_file_name(n));
    }
    out
}

/// Lyrics from an `.lrc` file beside `track`, either case of extension.
fn from_sidecar(track: &Path) -> Option<Lyrics> {
    for path in sidecars(track) {
        let upper = path.with_extension("LRC");
        for p in [path, upper] {
            if let Ok(bytes) = fs::read(&p) {
                if let Some(lines) = parse(&decode_text(&bytes)) {
                    return Some(Lyrics {
                        lines,
                        origin: Origin::Lrc(p),
                    });
                }
            }
        }
    }
    None
}

/// Lyrics from the track's tags: the best timed of what they hold.
fn from_tags(track: &Path) -> Option<Lyrics> {
    use lofty::prelude::*;
    let mut found: Vec<Vec<Line>> = Vec::new();
    // Timed lyrics are only in ID3v2, which the generic tag leaves out.
    if is_mpeg(track) {
        if let Some(lines) = synced_from_id3(track) {
            found.push(lines);
        }
    }
    if let Ok(file) = lofty::read_from_path(track) {
        for tag in file.tags() {
            for key in [ItemKey::Lyrics, ItemKey::UnsyncLyrics] {
                for text in tag.get_strings(key) {
                    if let Some(lines) = parse(text) {
                        found.push(lines);
                    }
                }
            }
        }
    }
    // The finest timing wins; among equals, the first found.
    let best = found.iter().map(|l| timing_of(l)).max()?;
    let lines = found.into_iter().find(|l| timing_of(l) == best)?;
    Some(Lyrics {
        lines,
        origin: Origin::Tag,
    })
}

fn is_mpeg(track: &Path) -> bool {
    track
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
}

/// The `SYLT` lyrics of an MP3, timed in milliseconds.
fn synced_from_id3(track: &Path) -> Option<Vec<Line>> {
    use lofty::config::ParseOptions;
    use lofty::file::AudioFile;
    use lofty::id3::v2::{
        Frame, FrameId, SyncTextContentType, SynchronizedTextFrame, TimestampFormat,
    };
    let mut f = fs::File::open(track).ok()?;
    let file = lofty::mpeg::MpegFile::read_from(&mut f, ParseOptions::new()).ok()?;
    let tag = file.id3v2()?;
    let sylt = FrameId::Valid(Cow::Borrowed("SYLT"));
    for frame in tag {
        let Frame::Binary(b) = frame else {
            continue;
        };
        if frame.id() != &sylt {
            continue;
        }
        let Ok(s) = SynchronizedTextFrame::parse(&b.data, frame.flags()) else {
            continue;
        };
        if s.timestamp_format != TimestampFormat::MS
            || s.content_type != SyncTextContentType::Lyrics
        {
            continue;
        }
        if let Some(lines) = from_synced(&s.content) {
            return Some(lines);
        }
    }
    None
}

/// The lyrics of the track at `path`: from an `.lrc` file beside it first,
/// then its tags. None when it has none (or can't be read).
pub fn find(path: &Path) -> Option<Lyrics> {
    from_sidecar(path).or_else(|| from_tags(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(lines: &[Line]) -> Vec<Option<f64>> {
        lines.iter().map(|l| l.start).collect()
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("01:02.50"), Some(62.5));
        assert_eq!(parse_time("00:05"), Some(5.0));
        assert_eq!(parse_time("1:02.345"), Some(62.345));
        assert_eq!(parse_time("01:02:50"), Some(62.5));
        assert_eq!(parse_time("ar"), None);
        assert_eq!(parse_time(""), None);
    }

    #[test]
    fn plain_lrc() {
        let lrc = "[ar:Someone]\n[ti:Song]\n\n[00:12.00]First line\n[00:15.30]Second line\n[00:20.00]\n[00:21.00]Third";
        let lines = parse(lrc).unwrap();
        assert_eq!(
            starts(&lines),
            vec![Some(12.0), Some(15.3), Some(20.0), Some(21.0)]
        );
        assert_eq!(lines[0].text, "First line");
        assert_eq!(lines[2].text, "");
        assert_eq!(timing_of(&lines), Timing::Lines);
        assert_eq!(line_at(&lines, 5.0), None);
        assert_eq!(line_at(&lines, 12.0), Some(0));
        assert_eq!(line_at(&lines, 16.0), Some(1));
        assert_eq!(line_at(&lines, 99.0), Some(3));
    }

    #[test]
    fn repeated_stamps_and_offset() {
        let lrc = "[offset:+500]\n[00:10.00][00:30.00]Chorus\n[00:20.00]Verse";
        let lines = parse(lrc).unwrap();
        assert_eq!(starts(&lines), vec![Some(9.5), Some(19.5), Some(29.5)]);
        assert_eq!(lines[2].text, "Chorus");
    }

    #[test]
    fn enhanced_lrc() {
        let lrc = "[00:10.00]<00:10.00>Hold <00:10.40>the <00:10.70>line<00:11.50>\n[00:12.00] <00:12.00>al<00:12.30>most home";
        let lines = parse(lrc).unwrap();
        assert_eq!(timing_of(&lines), Timing::Words);
        let l = &lines[0];
        assert_eq!(l.text, "Hold the line");
        assert_eq!(l.words.len(), 3);
        assert_eq!(l.words[1].text, "the ");
        assert_eq!(l.words[2].start, 10.7);
        assert_eq!(l.end, Some(11.5));
        let l = &lines[1];
        assert_eq!(l.text, "almost home");
        assert_eq!(l.words[0].text, "al");
        assert_eq!(l.words[1].text, "most home");
        assert_eq!(
            to_lrc(&lines[..1]),
            "[00:10.00]<00:10.00>Hold <00:10.40>the <00:10.70>line<00:11.50>\n"
        );
    }

    #[test]
    fn lrc_round_trip() {
        let lrc = "[00:01.00]One\n[01:02.25]Two\n";
        let lines = parse(lrc).unwrap();
        assert_eq!(to_lrc(&lines), lrc);
    }

    #[test]
    fn plain_text() {
        let text = "\n\nVerse one\nline two\n\n\n\nChorus\n\n";
        let lines = parse(text).unwrap();
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["Verse one", "line two", "", "Chorus"]);
        assert_eq!(timing_of(&lines), Timing::Plain);
        assert_eq!(line_at(&lines, 10.0), None);
        assert!(parse("  \n\n").is_none());
        assert!(parse("[ar:Only tags]\n").is_none());
    }

    #[test]
    fn bracketed_words_stay_text() {
        // A line that starts with [Chorus] is not a stamp.
        let lines = parse("[Chorus]\nla la").unwrap();
        assert_eq!(lines[0].text, "[Chorus]");
    }

    #[test]
    fn synced_by_line_and_word() {
        let by_line = vec![(1000, "One".to_string()), (3000, "Two".to_string())];
        let lines = from_synced(&by_line).unwrap();
        assert_eq!(starts(&lines), vec![Some(1.0), Some(3.0)]);
        assert_eq!(timing_of(&lines), Timing::Lines);
        let by_word = vec![
            (1000, "Hold ".to_string()),
            (1400, "the ".to_string()),
            (1700, "line".to_string()),
            (3000, "\nAl".to_string()),
            (3300, "most".to_string()),
        ];
        let lines = from_synced(&by_word).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hold the line");
        assert_eq!(lines[1].text, "Almost");
        assert_eq!(lines[1].words[1].start, 3.3);
        assert_eq!(timing_of(&lines), Timing::Words);
    }

    #[test]
    fn decodes_files() {
        assert_eq!(decode_text(b"\xEF\xBB\xBFhi"), "hi");
        assert_eq!(decode_text(b"caf\xE9"), "café");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
    }

    #[test]
    fn finds_sidecar_first() {
        let dir = std::env::temp_dir().join(format!("opendj-lyrics-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let track = dir.join("Song.flac");
        fs::write(&track, b"not audio").unwrap();
        assert!(find(&track).is_none());
        fs::write(dir.join("Song.lrc"), "[00:01.00]Hello").unwrap();
        let l = find(&track).unwrap();
        assert_eq!(l.origin, Origin::Lrc(dir.join("Song.lrc")));
        assert_eq!(l.lines[0].text, "Hello");
        fs::remove_dir_all(&dir).ok();
    }
}
