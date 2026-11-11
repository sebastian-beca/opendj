//! `export.pdb`: DeviceSQL pages, read as EngineX's `pdb.go` and
//! `devicesql.go` read them (see the module notes for the licence).

use std::collections::HashMap;
use std::path::Path;

use super::{Library, Playlist, Track};

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// The DeviceSQL string at `buf[off]`: short ASCII, long ASCII or UTF-16LE.
pub fn string(buf: &[u8], off: usize) -> Result<String, String> {
    let short = || "truncated DeviceSQL string".to_string();
    let kind = *buf.get(off).ok_or_else(short)?;
    if kind & 1 == 1 {
        // Short ASCII: the length, header byte included, in the top 7 bits.
        let n = (kind >> 1) as usize;
        if n == 0 {
            return Ok(String::new());
        }
        let body = buf.get(off + 1..off + n).ok_or_else(short)?;
        return Ok(String::from_utf8_lossy(body).into_owned());
    }
    if off + 4 > buf.len() {
        return Err(short());
    }
    let n = u16le(buf, off + 1) as usize;
    if n < 4 || off + n > buf.len() {
        return Err(short());
    }
    let body = &buf[off + 4..off + n];
    match kind {
        0x40 => Ok(String::from_utf8_lossy(body).into_owned()),
        0x90 => {
            if !body.len().is_multiple_of(2) {
                return Err(short());
            }
            let units: Vec<u16> = body
                .chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(String::from_utf16_lossy(&units)
                .trim_end_matches('\0')
                .to_string())
        }
        _ => Err(format!("unknown DeviceSQL string kind {kind:#04x}")),
    }
}

pub fn read(path: &Path) -> Result<Library, String> {
    let file = std::fs::read(path).map_err(|e| e.to_string())?;
    parse(&file)
}

/// Every live row of every table, then the tracks and lists they make.
pub fn parse(file: &[u8]) -> Result<Library, String> {
    if file.len() < 28 {
        return Err("short header".into());
    }
    let ps = u32le(file, 4) as usize;
    let nt = u32le(file, 8) as usize;
    if !(512..=65536).contains(&ps) || !file.len().is_multiple_of(ps) || nt > (ps - 28) / 16 {
        return Err("invalid page size/table count".into());
    }
    let pages = file.len() / ps;
    let mut rows: Vec<(u32, &[u8])> = Vec::new();
    for ti in 0..nt {
        let t = &file[28 + ti * 16..];
        let (typ, first, last) = (u32le(t, 0), u32le(t, 8), u32le(t, 12));
        let mut seen = std::collections::HashSet::new();
        let mut n = first;
        while n != 0 {
            if !seen.insert(n) || n as usize >= pages {
                return Err(format!("invalid/cyclic page {n}"));
            }
            let p = &file[n as usize * ps..(n as usize + 1) * ps];
            if u32le(p, 4) != n || u32le(p, 8) != typ {
                return Err(format!("page {n} type/index mismatch"));
            }
            if p[27] & 0x40 == 0 {
                let count = (u32le(p, 24) & 0x1fff) as usize;
                // A full group of 16 rows takes 36 bytes; the last, if
                // partial, only its flags and the offsets it uses.
                let mut index_bytes = (count / 16) * 36;
                if !count.is_multiple_of(16) {
                    index_bytes += 4 + 2 * (count % 16);
                }
                let heap_end = ps
                    .checked_sub(index_bytes)
                    .filter(|&e| e >= 40)
                    .ok_or("row index overlaps header")?;
                let mut offs = Vec::new();
                for i in 0..count {
                    let base = ps - (i / 16) * 36;
                    let live = u16le(p, base - 4) & (1 << (i % 16)) != 0;
                    if !live {
                        continue;
                    }
                    let off = 40 + u16le(p, base - 6 - 2 * (i % 16)) as usize;
                    if off >= heap_end {
                        return Err("row outside heap".into());
                    }
                    offs.push(off);
                }
                offs.sort_unstable();
                for (i, &off) in offs.iter().enumerate() {
                    let end = offs.get(i + 1).copied().unwrap_or(heap_end);
                    if end <= off {
                        return Err("overlapping row".into());
                    }
                    rows.push((typ, &p[off..end]));
                }
            }
            if n == last {
                break;
            }
            n = u32le(p, 12);
            if n == 0 {
                return Err("page chain ends before last page".into());
            }
        }
    }

    let mut lib = Library::default();
    let mut names: HashMap<u32, HashMap<u32, String>> = HashMap::new();
    for &(typ, b) in &rows {
        let (id, name) = match typ {
            // Genres, labels, keys, artwork.
            1 | 4 | 5 | 13 => {
                if b.len() < 5 {
                    return Err("short lookup row".into());
                }
                (u32le(b, 0), string(b, if typ == 5 { 8 } else { 4 })?)
            }
            2 => {
                if b.len() < 10 {
                    return Err("short artist row".into());
                }
                let far = u16le(b, 0) & 4 != 0;
                if far && b.len() < 12 {
                    return Err("short far artist".into());
                }
                let off = if far {
                    u16le(b, 10) as usize
                } else {
                    b[9] as usize
                };
                (u32le(b, 4), string(b, off)?)
            }
            3 => {
                if b.len() < 22 {
                    return Err("short album row".into());
                }
                let far = u16le(b, 0) & 4 != 0;
                if far && b.len() < 24 {
                    return Err("short far album".into());
                }
                let off = if far {
                    u16le(b, 22) as usize
                } else {
                    b[21] as usize
                };
                (u32le(b, 12), string(b, off)?)
            }
            _ => continue,
        };
        names.entry(typ).or_default().insert(id, name);
    }
    let name = |typ: u32, id: u32| {
        names
            .get(&typ)
            .and_then(|m| m.get(&id))
            .cloned()
            .unwrap_or_default()
    };
    lib.keys = names.get(&5).cloned().unwrap_or_default();
    lib.artwork = names.get(&13).cloned().unwrap_or_default();

    let mut entries: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for &(typ, b) in &rows {
        match typ {
            0 => {
                if b.len() < 136 {
                    return Err("short track row".into());
                }
                let mut s = Vec::with_capacity(21);
                for i in 0..21 {
                    let off = u16le(b, 94 + 2 * i) as usize;
                    s.push(string(b, off).map_err(|e| format!("track string {i}: {e}"))?);
                }
                lib.tracks.push(Track {
                    id: u32le(b, 72),
                    path: s[20].clone(),
                    analysis_path: s[14].clone(),
                    title: s[17].clone(),
                    comment: s[16].clone(),
                    artist: name(2, u32le(b, 68)),
                    album: name(3, u32le(b, 64)),
                    genre: name(1, u32le(b, 60)),
                    label: name(4, u32le(b, 40)),
                    composer: name(2, u32le(b, 12)),
                    remixer: name(2, u32le(b, 44)),
                    key: name(5, u32le(b, 32)),
                    artwork_path: name(13, u32le(b, 28)),
                    sample_rate: u32le(b, 8),
                    file_size: u32le(b, 16) as u64,
                    bitrate: u32le(b, 48),
                    bpm: u32le(b, 56) as f64 / 100.0,
                    year: u16le(b, 80) as u32,
                    bit_depth: u16le(b, 82) as u32,
                    duration: u16le(b, 84) as f64,
                    rating: b[89] as u32,
                });
            }
            7 => {
                if b.len() < 21 {
                    return Err("short playlist".into());
                }
                lib.playlists.push(Playlist {
                    id: u32le(b, 12),
                    parent_id: u32le(b, 0),
                    sort_order: u32le(b, 8),
                    folder: u32le(b, 16) != 0,
                    name: string(b, 20)?,
                    tracks: Vec::new(),
                });
            }
            8 => {
                if b.len() < 12 {
                    return Err("short playlist entry".into());
                }
                entries
                    .entry(u32le(b, 8))
                    .or_default()
                    .push((u32le(b, 0), u32le(b, 4)));
            }
            _ => {}
        }
    }
    for p in &mut lib.playlists {
        let mut es = entries.remove(&p.id).unwrap_or_default();
        es.sort_by_key(|e| e.0);
        p.tracks = es.into_iter().map(|e| e.1).collect();
    }
    lib.validate()?;
    Ok(lib)
}

/// Layout rules players are known to hold a stick to, beyond what a reader
/// needs, from rbxport's stick reference (CDJ-3000 firmware 3.20) and the
/// Deep Symmetry notes: each table opens with an index page, its last page
/// links to the table's empty page, and no track string is left at offset 0.
pub fn strict(file: &[u8]) -> Result<(), String> {
    let ps = u32le(file, 4) as usize;
    let nt = u32le(file, 8) as usize;
    let next_unused = u32le(file, 12);
    for ti in 0..nt {
        let t = &file[28 + ti * 16..];
        let (typ, empty, first, last) = (u32le(t, 0), u32le(t, 4), u32le(t, 8), u32le(t, 12));
        let page = |n: u32| &file[n as usize * ps..(n as usize + 1) * ps];
        let index = page(first);
        if index[27] != 0x64 || u16le(index, 0x20) != 0x1fff || u16le(index, 0x22) != 0x1fff {
            return Err(format!("table {typ} doesn't open with an index page"));
        }
        if u32le(page(last), 12) != empty {
            return Err(format!(
                "table {typ}'s last page doesn't link to its empty page"
            ));
        }
        if empty >= next_unused {
            return Err(format!(
                "table {typ}'s empty page is past the next unused page"
            ));
        }
        if (empty as usize + 1) * ps <= file.len() && page(empty).iter().any(|&b| b != 0) {
            return Err(format!("table {typ}'s empty page isn't blank"));
        }
    }
    Ok(())
}

/// Whether every track row's 21 string offsets point somewhere (an unused
/// one at an empty string, not at 0).
pub fn track_strings_set(file: &[u8]) -> Result<(), String> {
    let lib_rows = rows_of(file, 0)?;
    for b in lib_rows {
        if (0..21).any(|i| u16le(b, 94 + 2 * i) == 0) {
            return Err(format!("track {} has a string at offset 0", u32le(b, 72)));
        }
    }
    Ok(())
}

/// The live rows of one table.
fn rows_of(file: &[u8], want: u32) -> Result<Vec<&[u8]>, String> {
    let ps = u32le(file, 4) as usize;
    let nt = u32le(file, 8) as usize;
    let mut out = Vec::new();
    for ti in 0..nt {
        let t = &file[28 + ti * 16..];
        if u32le(t, 0) != want {
            continue;
        }
        let (mut n, last) = (u32le(t, 8), u32le(t, 12));
        loop {
            let p = &file[n as usize * ps..(n as usize + 1) * ps];
            if p[27] & 0x40 == 0 {
                let count = (u32le(p, 24) & 0x1fff) as usize;
                for i in 0..count {
                    let base = ps - (i / 16) * 36;
                    if u16le(p, base - 4) & (1 << (i % 16)) != 0 {
                        let off = 40 + u16le(p, base - 6 - 2 * (i % 16)) as usize;
                        out.push(&p[off..]);
                    }
                }
            }
            if n == last {
                break;
            }
            n = u32le(p, 12);
        }
    }
    Ok(out)
}
