//! Analysis files (`.DAT`, `.EXT`, `.2EX`), read as EngineX's
//! `analysis.go` reads them (see the module notes for the licence).

use std::collections::HashMap;
use std::path::Path;

fn u16be(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn u32be(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beat {
    /// 1 to 4, the beat in its bar.
    pub number: u16,
    pub bpm: f64,
    pub time_ms: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cue {
    /// 0 for a memory cue, 1 to 8 for A to H.
    pub hot_cue: u32,
    pub time_ms: u32,
    pub end_ms: u32,
    pub is_loop: bool,
    pub active: bool,
    pub name: String,
    /// A, R, G, B.
    pub color: [u8; 4],
    pub color_id: u8,
    pub color_code: u8,
    pub loop_numerator: u16,
    pub loop_denominator: u16,
    /// From a `PCO2` (the `.EXT`'s list with names and colours).
    pub extended: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    pub tag: String,
    pub entry_bytes: u32,
    pub data: Vec<u8>,
}

impl Waveform {
    pub fn entries(&self) -> usize {
        self.data.len() / self.entry_bytes as usize
    }
}

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub beats: Vec<Beat>,
    pub cues: Vec<Cue>,
    pub waveforms: Vec<Waveform>,
    /// Phrases: (mood, number of phrases).
    pub structure: Option<(u16, usize)>,
    /// Sections it doesn't look into, by tag, e.g. `PPTH` (the audio's path).
    pub unknown: HashMap<String, Vec<Vec<u8>>>,
}

impl Analysis {
    pub fn waveform(&self, tag: &str) -> Option<&Waveform> {
        self.waveforms.iter().find(|w| w.tag == tag)
    }

    pub fn hot_cue(&self, number: u32) -> Option<&Cue> {
        self.cues.iter().find(|c| c.hot_cue == number)
    }
}

pub fn read(path: &Path) -> Result<Analysis, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&b)
}

/// Checks every section's bounds, the ones it skips too.
pub fn parse(b: &[u8]) -> Result<Analysis, String> {
    if b.len() < 12 || &b[..4] != b"PMAI" {
        return Err("invalid PMAI header".into());
    }
    let (h, n) = (u32be(b, 4) as usize, u32be(b, 8) as usize);
    // FAT32 and exFAT copies may pad a file past its stated length.
    if h < 12 || h > n || n > b.len() {
        return Err("invalid file length".into());
    }
    let b = &b[..n];
    let mut a = Analysis::default();
    let mut off = h;
    while off < n {
        if n - off < 12 {
            return Err("short section".into());
        }
        let s = &b[off..];
        let tag = String::from_utf8_lossy(&s[..4]).into_owned();
        let (sh, sn) = (u32be(s, 4) as usize, u32be(s, 8) as usize);
        if sh < 12 || sn < sh || sn > n - off {
            return Err(format!("invalid {tag} length"));
        }
        a.section(&tag, &s[..sn], sh)
            .map_err(|e| format!("{tag}: {e}"))?;
        off += sn;
    }
    Ok(a)
}

impl Analysis {
    fn section(&mut self, tag: &str, s: &[u8], h: usize) -> Result<(), String> {
        let bad = || "truncated or invalid data".to_string();
        match tag {
            "PQTZ" => {
                if s.len() < 24 {
                    return Err(bad());
                }
                let n = u32be(s, 20) as usize;
                if n > (s.len() - 24) / 8 {
                    return Err(bad());
                }
                for i in 0..n {
                    let p = &s[24 + 8 * i..];
                    self.beats.push(Beat {
                        number: u16be(p, 0),
                        bpm: u16be(p, 2) as f64 / 100.0,
                        time_ms: u32be(p, 4),
                    });
                }
            }
            "PCOB" | "PCO2" => {
                let pcob = tag == "PCOB";
                let (min, mut off, count_at) = if pcob { (24, 24, 18) } else { (20, 20, 16) };
                if s.len() < min {
                    return Err(bad());
                }
                let n = u16be(s, count_at) as usize;
                if n > (s.len() - off) / 12 {
                    return Err(bad());
                }
                for _ in 0..n {
                    if s.len() - off < 12 {
                        return Err(bad());
                    }
                    let p = &s[off..];
                    let size = u32be(p, 8) as usize;
                    if size < 12 || size > p.len() {
                        return Err(bad());
                    }
                    let p = &p[..size];
                    let mut c = Cue::default();
                    if pcob {
                        if size < 56 || &p[..4] != b"PCPT" {
                            return Err(bad());
                        }
                        c.hot_cue = u32be(p, 12);
                        c.active = u32be(p, 16) == 4;
                        c.is_loop = p[28] == 2;
                        c.time_ms = u32be(p, 32);
                        c.end_ms = u32be(p, 36);
                    } else {
                        if size < 40 || &p[..4] != b"PCP2" {
                            return Err(bad());
                        }
                        c.extended = true;
                        c.hot_cue = u32be(p, 12);
                        c.is_loop = p[16] == 2;
                        c.time_ms = u32be(p, 20);
                        c.end_ms = u32be(p, 24);
                        c.color_id = p[28];
                        c.loop_numerator = u16be(p, 36);
                        c.loop_denominator = u16be(p, 38);
                        if size > 43 {
                            let len = u32be(p, 40) as usize;
                            if !len.is_multiple_of(2) || len > size - 44 {
                                return Err(bad());
                            }
                            let units: Vec<u16> =
                                p[44..44 + len].chunks(2).map(|c| u16be(c, 0)).collect();
                            c.name = String::from_utf16_lossy(&units)
                                .trim_end_matches('\0')
                                .to_string();
                            let q = 44 + len;
                            if size > q {
                                c.color_code = p[q];
                            }
                            if size >= q + 4 {
                                c.color = [255, p[q + 1], p[q + 2], p[q + 3]];
                            }
                        }
                    }
                    if c.is_loop && c.end_ms < c.time_ms {
                        return Err("loop end precedes start".into());
                    }
                    self.merge_cue(c);
                    off += size;
                }
            }
            "PWV3" | "PWV5" | "PWV7" | "PWV4" | "PWAV" | "PWV2" => {
                if s.len() < 20 {
                    return Err(bad());
                }
                let (mut width, mut count) = (1, u32be(s, 12));
                if matches!(tag, "PWV3" | "PWV4" | "PWV5" | "PWV7") {
                    if s.len() < 24 {
                        return Err(bad());
                    }
                    (width, count) = (u32be(s, 12), u32be(s, 16));
                }
                if tag == "PWV4" {
                    width = 6;
                }
                let bytes = width as u64 * count as u64;
                if width == 0 || bytes > (s.len() - h) as u64 {
                    return Err(bad());
                }
                self.waveforms.push(Waveform {
                    tag: tag.to_string(),
                    entry_bytes: width,
                    data: s[h..h + bytes as usize].to_vec(),
                });
            }
            "PSSI" => {
                if s.len() < 32 {
                    return Err(bad());
                }
                let (width, n) = (u32be(s, 12) as usize, u16be(s, 16) as usize);
                if width < 24 {
                    return Err(bad());
                }
                let mut body = s[18..].to_vec();
                if u16be(&body, 0) > 20 {
                    // rekordbox 6 and later mask the phrases.
                    const MASK: [u8; 19] = [
                        0xcb, 0xe1, 0xee, 0xfa, 0xe5, 0xee, 0xad, 0xee, 0xe9, 0xd2, 0xe9, 0xeb,
                        0xe1, 0xe9, 0xf3, 0xe8, 0xe9, 0xf4, 0xe1,
                    ];
                    for (i, x) in body.iter_mut().enumerate() {
                        *x ^= MASK[i % MASK.len()].wrapping_add(n as u8);
                    }
                }
                if body.len() < 14 || n > (body.len() - 14) / width {
                    return Err(bad());
                }
                self.structure = Some((u16be(&body, 0), n));
            }
            _ => self
                .unknown
                .entry(tag.to_string())
                .or_default()
                .push(s.to_vec()),
        }
        Ok(())
    }

    /// A cue seen twice (`.DAT` and `.EXT`) is kept once, the extended one
    /// winning.
    fn merge_cue(&mut self, c: Cue) {
        for old in &mut self.cues {
            let same = (c.hot_cue != 0 && c.hot_cue == old.hot_cue)
                || (c.hot_cue == 0
                    && old.hot_cue == 0
                    && c.time_ms == old.time_ms
                    && c.is_loop == old.is_loop
                    && (!c.is_loop || c.end_ms == old.end_ms));
            if same {
                let active = c.active || old.active;
                if c.extended || !old.extended {
                    *old = c;
                }
                old.active = active;
                return;
            }
        }
        self.cues.push(c);
    }

    /// Reads another file of the same track into this one.
    pub fn merge(&mut self, b: Analysis) {
        if !b.beats.is_empty() {
            self.beats = b.beats;
        }
        for c in b.cues {
            self.merge_cue(c);
        }
        self.waveforms.extend(b.waveforms);
        if b.structure.is_some() {
            self.structure = b.structure;
        }
        for (k, v) in b.unknown {
            self.unknown.entry(k).or_default().extend(v);
        }
    }
}
