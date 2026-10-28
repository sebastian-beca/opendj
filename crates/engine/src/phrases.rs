//! Phrases: a track cut into sections of whole bars (intro, build, drop,
//! breakdown, outro) from its beat grid and waveform.
//!
//! Each bar gets the mean level of the low, mid and high bands. A dynamic
//! program then picks the cuts that fit those levels best with as few
//! sections as it can, allowing only lengths of 8 to 32 bars in steps of 4
//! (8 and 16 cost less), so short flickers never become phrases of their
//! own. The first and last section may be shorter, down to 4 bars, for a
//! pickup or a tail. Each cut then moves up to 4 bars to where the bass
//! comes in or drops out, so a kick entering early ends the intro there.
//! A section is a drop when its bass and level come close
//! to the track's strongest section's, and a chorus (main groove) when they
//! are a little further off; the others are named by where they fall. Neighbours of the same kind merge.
//!
//! The method follows the research in the project notes
//! (phrase-analysis-review.md); it was written for openDJ.

use crate::track::{Marker, MarkerKind, Track};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseKind {
    Intro,
    /// Rising into a drop (an "up").
    Build,
    /// The peak.
    Drop,
    /// The bass gone (a break or bridge).
    Breakdown,
    Outro,
    /// Strong but short of the peak: the main groove or a chorus.
    Chorus,
}

impl PhraseKind {
    const ALL: [Self; 6] = [
        Self::Intro,
        Self::Build,
        Self::Drop,
        Self::Breakdown,
        Self::Outro,
        Self::Chorus,
    ];

    /// What the library calls it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Intro => "Intro",
            Self::Build => "Build",
            Self::Drop => "Drop",
            Self::Breakdown => "Breakdown",
            Self::Outro => "Outro",
            Self::Chorus => "Chorus",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phrase {
    /// Where it starts, in seconds; the first phrase starts at 0.
    pub start: f64,
    pub kind: PhraseKind,
}

/// Shortest and longest section in bars, apart from the first and last.
const MIN_BARS: usize = 8;
const MAX_BARS: usize = 32;
/// The first and last section can be this short.
const MIN_EDGE_BARS: usize = 4;
/// Fewer bars than this: too short for phrases.
const MIN_TRACK_BARS: usize = 16;
/// How much each band counts when fitting: bass most, as that is what
/// drops and breakdowns change.
const BAND_WEIGHT: [f32; 3] = [1.5, 1.0, 0.7];
/// The price of one more section, in squared level units. Higher means
/// fewer, longer phrases.
const PER_SECTION: f32 = 1.2;
/// The price of a length that is not a multiple of 8 bars, and of a first
/// or last section that is not a multiple of 4.
const NOT_EIGHT: f32 = 0.4;
const NOT_FOUR: f32 = 0.8;
/// How far a cut may move to where the bass comes in or drops out, and
/// how big that change must be (in normalised bass level).
const SNAP_BARS: usize = 4;
const SNAP_JUMP: f32 = 0.2;
/// A drop: bass and overall level at least this share of the track's
/// strongest section's.
const DROP_SHARE: f32 = 0.85;
/// A chorus (main groove): short of a drop, but bass at least the first
/// share and level at least the second of the strongest section's.
const CHORUS_LOW_SHARE: f32 = 0.6;
const CHORUS_LEVEL_SHARE: f32 = 0.7;

/// Whether the track's waveform covers all of it, so its phrases can be
/// found (while it is still being analysed they can't).
pub fn waveform_ready(track: &Track) -> bool {
    let wave = &track.waveform;
    wave.bins_per_second > 0.0
        && track.duration > 0.0
        && wave
            .levels
            .first()
            .is_some_and(|l| l.len() as f64 / wave.bins_per_second >= track.duration - 0.5)
}

/// Phrases as the library keeps them: markers of kind phrase, each
/// ending where the next starts (the last at `duration`).
pub fn to_markers(phrases: &[Phrase], duration: f64) -> Vec<Marker> {
    phrases
        .iter()
        .enumerate()
        .map(|(i, p)| Marker {
            kind: MarkerKind::Phrase,
            seconds: p.start,
            end: Some(phrases.get(i + 1).map_or(duration, |n| n.start)),
            name: p.kind.name().to_string(),
            color: None,
            detected: true,
        })
        .collect()
}

/// The detected phrases stored with a track, earliest first.
pub fn stored(track: &Track) -> Vec<Phrase> {
    let mut out: Vec<Phrase> = track
        .info
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::Phrase && m.detected)
        .filter_map(|m| {
            Some(Phrase {
                start: m.seconds,
                kind: PhraseKind::from_name(&m.name)?,
            })
        })
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// Whether the track has phrases from an import, which detected ones
/// don't replace.
pub fn imported(track: &Track) -> bool {
    track
        .info
        .markers
        .iter()
        .any(|m| m.kind == MarkerKind::Phrase && !m.detected)
}

/// `markers` with its detected phrases replaced by `phrases`.
pub fn with_phrases(markers: &[Marker], phrases: Vec<Marker>) -> Vec<Marker> {
    let mut out: Vec<Marker> = markers
        .iter()
        .filter(|m| !(m.kind == MarkerKind::Phrase && m.detected))
        .cloned()
        .chain(phrases)
        .collect();
    out.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    out
}

/// The phrases of a track, earliest first; empty without a tempo or
/// waveform, for a track shorter than 16 bars, or when it all sounds alike.
pub fn detect(track: &Track) -> Vec<Phrase> {
    let wave = &track.waveform;
    let Some(slices) = wave.levels.first() else {
        return Vec::new();
    };
    if track.bpm <= 0.0 || slices.is_empty() || wave.bins_per_second <= 0.0 {
        return Vec::new();
    }
    // Bars start at the first bar line at or after the start (bar lines
    // every fourth beat from beat 0, as the waveform draws them).
    let first = (track.beat_at(0.0) / 4.0 - 1e-6).ceil() * 4.0;
    let mut starts = Vec::new();
    let mut levels = Vec::new();
    let slice = |s: f64| ((s * wave.bins_per_second) as usize).min(slices.len());
    loop {
        let beat = first + 4.0 * starts.len() as f64;
        let (a, b) = (track.seconds_at(beat), track.seconds_at(beat + 4.0));
        if b > track.duration {
            break;
        }
        let (i, j) = (slice(a), slice(b));
        if i >= j {
            break;
        }
        // RMS bytes are square roots of the level; squared, they are levels.
        let mut sum = [0f32; 3];
        for bin in &slices[i..j] {
            for (k, s) in sum.iter_mut().enumerate() {
                *s += (bin[4 + k] as f32 / 255.0).powi(2);
            }
        }
        starts.push(a);
        levels.push(sum.map(|s| s / (j - i) as f32));
    }
    if levels.len() < MIN_TRACK_BARS {
        return Vec::new();
    }
    normalise(&mut levels);
    let cuts = refine(&levels, &segment(&levels));
    let kinds = classify(&levels, &cuts);
    let mut phrases: Vec<Phrase> = Vec::new();
    for (n, (&cut, kind)) in cuts.iter().zip(kinds).enumerate() {
        if phrases.last().is_some_and(|p| p.kind == kind) {
            continue;
        }
        phrases.push(Phrase {
            start: if n == 0 { 0.0 } else { starts[cut] },
            kind,
        });
    }
    // One phrase for the whole track says nothing.
    if phrases.len() < 2 {
        phrases.clear();
    }
    phrases
}

/// Scales each band so the track's loud bars (90th percentile) are 1.
fn normalise(levels: &mut [[f32; 3]]) {
    for k in 0..3 {
        let mut v: Vec<f32> = levels.iter().map(|l| l[k]).collect();
        v.sort_by(f32::total_cmp);
        let top = v[(v.len() - 1) * 9 / 10].max(1e-6);
        levels.iter_mut().for_each(|l| l[k] /= top);
    }
}

/// The price of a section `len` bars long, or None where it isn't allowed;
/// `edge` for the first or last section.
fn length_price(len: usize, edge: bool) -> Option<f32> {
    if len > MAX_BARS || len < if edge { MIN_EDGE_BARS } else { MIN_BARS } {
        return None;
    }
    if !len.is_multiple_of(4) {
        return edge.then_some(NOT_FOUR + NOT_EIGHT);
    }
    Some(if len.is_multiple_of(8) {
        0.0
    } else {
        NOT_EIGHT
    })
}

/// The first bar of each section, ascending, starting with 0.
fn segment(levels: &[[f32; 3]]) -> Vec<usize> {
    let n = levels.len();
    // Prefix sums of the weighted levels and their squares, so a section's
    // squared deviation from its mean is a few subtractions.
    let mut s1 = vec![[0f32; 3]; n + 1];
    let mut s2 = vec![0f32; n + 1];
    for (i, l) in levels.iter().enumerate() {
        let w = [0, 1, 2].map(|k| l[k] * BAND_WEIGHT[k]);
        s1[i + 1] = [0, 1, 2].map(|k| s1[i][k] + w[k]);
        s2[i + 1] = s2[i] + w.iter().map(|v| v * v).sum::<f32>();
    }
    let sse = |a: usize, b: usize| {
        let len = (b - a) as f32;
        s2[b] - s2[a] - (0..3).map(|k| (s1[b][k] - s1[a][k]).powi(2)).sum::<f32>() / len
    };
    // best[j]: the cheapest way to cover bars 0..j; from[j]: where its last
    // section starts.
    let mut best = vec![f32::INFINITY; n + 1];
    let mut from = vec![0usize; n + 1];
    best[0] = 0.0;
    for j in 1..=n {
        for i in j.saturating_sub(MAX_BARS)..j {
            if !best[i].is_finite() {
                continue;
            }
            let Some(price) = length_price(j - i, i == 0 || j == n) else {
                continue;
            };
            let cost = best[i] + sse(i, j) + PER_SECTION + price;
            if cost < best[j] {
                best[j] = cost;
                from[j] = i;
            }
        }
    }
    if !best[n].is_finite() {
        // No allowed split (can't happen from 16 bars up, but stay safe).
        return vec![0];
    }
    let mut cuts = Vec::new();
    let mut j = n;
    while j > 0 {
        j = from[j];
        cuts.push(j);
    }
    cuts.reverse();
    cuts
}

/// Moves each cut to where the bass actually comes in or drops out, up to
/// `SNAP_BARS` either way: the fit above prefers whole 8- and 16-bar
/// sections, so a kick entering on its own a few bars early would otherwise
/// end up inside the intro. The bass crosses halfway between the levels on
/// either side; the earliest crossing in reach wins.
fn refine(levels: &[[f32; 3]], cuts: &[usize]) -> Vec<usize> {
    let n = levels.len();
    let low = |a: usize, b: usize| levels[a..b].iter().map(|l| l[0]).sum::<f32>() / (b - a) as f32;
    let mut out = cuts.to_vec();
    for i in 1..cuts.len() {
        let (a, c) = (out[i - 1], cuts[i]);
        let b = cuts.get(i + 1).copied().unwrap_or(n);
        let (before, after) = (low(a, c), low(c, b));
        if (after - before).abs() < SNAP_JUMP {
            continue;
        }
        let half = (before + after) / 2.0;
        let rises = after > before;
        // Both sides keep their least length.
        let min_left = if i == 1 { MIN_EDGE_BARS } else { MIN_BARS };
        let min_right = if i + 1 == cuts.len() {
            MIN_EDGE_BARS
        } else {
            MIN_BARS
        };
        let lo = c.saturating_sub(SNAP_BARS).max(a + min_left);
        let hi = (c + SNAP_BARS).min(b.saturating_sub(min_right));
        // A crossing: a bar on the old side of halfway, then two on the new.
        let crosses = |x: usize| {
            let next = [levels[x][0], levels[(x + 1).min(n - 1)][0]];
            if rises {
                levels[x - 1][0] < half && next.iter().all(|&l| l >= half)
            } else {
                levels[x - 1][0] >= half && next.iter().all(|&l| l < half)
            }
        };
        if let Some(x) = (lo..=hi).find(|&x| crosses(x)) {
            out[i] = x;
        }
    }
    out
}

/// The kind of each section (`cuts` are their first bars).
fn classify(levels: &[[f32; 3]], cuts: &[usize]) -> Vec<PhraseKind> {
    let n = levels.len();
    let mean = |a: usize, b: usize| {
        let mut m = [0f32; 3];
        for l in &levels[a..b] {
            (0..3).for_each(|k| m[k] += l[k] / (b - a) as f32);
        }
        m
    };
    let level = |m: [f32; 3]| (m[0] + m[1] + m[2]) / 3.0;
    let ends: Vec<usize> = cuts.iter().skip(1).copied().chain([n]).collect();
    let means: Vec<[f32; 3]> = cuts.iter().zip(&ends).map(|(&a, &b)| mean(a, b)).collect();
    let top_low = means.iter().map(|m| m[0]).fold(0.0, f32::max);
    let top_level = means.iter().map(|m| level(*m)).fold(0.0, f32::max);
    let drop: Vec<bool> = means
        .iter()
        .map(|m| m[0] >= DROP_SHARE * top_low && level(*m) >= DROP_SHARE * top_level)
        .collect();
    let chorus: Vec<bool> = means
        .iter()
        .map(|m| m[0] >= CHORUS_LOW_SHARE * top_low && level(*m) >= CHORUS_LEVEL_SHARE * top_level)
        .collect();
    let first_drop = drop.iter().position(|&d| d);
    // The outro is the last section and whatever quieter ones lead into it
    // after the last strong section, drop or chorus: a full groove after
    // the last drop is not yet the outro.
    let last_strong = (0..cuts.len()).rposition(|s| drop[s] || chorus[s]);
    (0..cuts.len())
        .map(|s| {
            let strong_next = drop.get(s + 1) == Some(&true) || chorus.get(s + 1) == Some(&true);
            if drop[s] {
                PhraseKind::Drop
            } else if s == 0 {
                PhraseKind::Intro
            } else if s + 1 == cuts.len() {
                // The last section, short of a drop, is the outro even with
                // the groove still running.
                PhraseKind::Outro
            } else if chorus[s] {
                PhraseKind::Chorus
            } else if last_strong.is_some_and(|l| s > l) {
                PhraseKind::Outro
            } else if first_drop.is_none_or(|f| s < f)
                || strong_next && rising(levels, cuts[s], ends[s])
            {
                // A lift into the first drop, or one that climbs into the next.
                PhraseKind::Build
            } else {
                PhraseKind::Breakdown
            }
        })
        .collect()
}

/// Whether the level climbs through bars `a..b`: its last quarter is
/// clearly louder than its first.
fn rising(levels: &[[f32; 3]], a: usize, b: usize) -> bool {
    let q = ((b - a) / 4).max(1);
    let avg = |r: &[[f32; 3]]| r.iter().map(|l| l[0] + l[1] + l[2]).sum::<f32>() / r.len() as f32;
    avg(&levels[b - q..b]) > avg(&levels[a..a + q]) * 1.25
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis;
    use crate::synth;
    use crate::track::{Source, Track};
    use std::sync::Arc;

    /// A synthetic track, analysed: its sections (intro, build, drop,
    /// breakdown, drop, outro) are each an eighth of it.
    fn analysed(bpm: f64, bars: usize, seed: u64) -> (Track, f64) {
        let bar = 240.0 / bpm;
        let duration = bars as f64 * bar;
        let audio = synth::render_with(bpm, 0.0, duration, seed);
        let a = analysis::analyse(&audio, Some(bpm));
        let mut t = Track {
            id: 0,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration,
            first_beat: 0.0,
            grid: Default::default(),
            source: Source::Synth { seed },
            waveform: Arc::new(a.waveform),
            audio: None,
            cover: None,
            info: Default::default(),
        };
        t.bpm = a.bpm;
        t.first_beat = a.first_beat;
        t.set_grid(a.grid);
        (t, bar)
    }

    #[test]
    fn phrases_are_kept_as_markers_beside_imported_ones() {
        let (mut t, _) = analysed(124.0, 128, 3);
        assert!(waveform_ready(&t));
        let p = detect(&t);
        let markers = to_markers(&p, t.duration);
        assert_eq!(markers.last().unwrap().end, Some(t.duration));
        let section = Marker {
            kind: MarkerKind::Section,
            seconds: 10.0,
            end: None,
            name: "Break 1".into(),
            color: None,
            detected: false,
        };
        t.info.markers = with_phrases(std::slice::from_ref(&section), markers.clone());
        assert_eq!(stored(&t), p);
        assert!(!imported(&t));
        // Found again: the old ones go, the section stays.
        t.info.markers = with_phrases(&t.info.markers, markers[..1].to_vec());
        assert_eq!(stored(&t), p[..1]);
        assert!(t.info.markers.contains(&section));
        // Phrases from an import.
        t.info.markers.push(Marker {
            kind: MarkerKind::Phrase,
            detected: false,
            ..section
        });
        assert!(imported(&t));
        // Not ready while the waveform is still being made.
        t.duration *= 2.0;
        assert!(!waveform_ready(&t));
    }

    #[test]
    fn finds_the_sections_of_a_track() {
        let (t, bar) = analysed(124.0, 128, 3);
        let p = detect(&t);
        let kinds: Vec<_> = p.iter().map(|p| p.kind).collect();
        assert_eq!(kinds.first(), Some(&PhraseKind::Intro), "{p:?}");
        assert_eq!(kinds.last(), Some(&PhraseKind::Outro), "{p:?}");
        assert!(kinds.contains(&PhraseKind::Breakdown), "{p:?}");
        assert_eq!(
            kinds.iter().filter(|k| **k == PhraseKind::Drop).count(),
            2,
            "{p:?}"
        );
        // The breakdown starts where the synth's does (bar 64), give or take a bar.
        let breakdown = p.iter().find(|p| p.kind == PhraseKind::Breakdown).unwrap();
        assert!((breakdown.start / bar - 64.0).abs() <= 1.0, "{p:?}");
    }

    #[test]
    fn no_short_phrases() {
        for seed in 0..4 {
            let (t, bar) = analysed(128.0, 96, seed);
            let p = detect(&t);
            assert!(p.len() <= 8, "{p:?}");
            let ends = p.iter().skip(1).map(|p| p.start).chain([t.duration]);
            for (ph, end) in p.iter().zip(ends) {
                assert!(end - ph.start >= 4.0 * bar - 0.05, "{p:?}");
            }
        }
    }

    #[test]
    fn nothing_without_a_tempo_or_for_short_tracks() {
        let (mut t, _) = analysed(124.0, 64, 1);
        assert!(!detect(&t).is_empty());
        t.bpm = 0.0;
        t.grid = Default::default();
        assert!(detect(&t).is_empty());
        let (short, _) = analysed(124.0, 12, 1);
        assert!(detect(&short).is_empty());
    }

    #[test]
    fn segments_prefer_long_whole_phrases() {
        // 16 quiet bars, 32 loud, 3 bars of a blip, 13 loud, 16 quiet.
        let mut levels = vec![[0.2, 0.3, 0.3]; 16];
        levels.extend(vec![[1.0, 1.0, 1.0]; 32]);
        levels.extend(vec![[0.4, 0.8, 1.0]; 3]);
        levels.extend(vec![[1.0, 1.0, 1.0]; 13]);
        levels.extend(vec![[0.2, 0.3, 0.3]; 16]);
        let cuts = segment(&levels);
        assert!(cuts.contains(&16) && cuts.contains(&64), "{cuts:?}");
        let ends = cuts.iter().skip(1).copied().chain([levels.len()]);
        assert!(
            cuts.iter().zip(ends).all(|(a, b)| b - a >= MIN_BARS),
            "the blip is not its own phrase: {cuts:?}"
        );
    }

    #[test]
    fn the_intro_ends_where_the_kick_comes_in() {
        // 12 bars without bass, the kick alone for 4, then the full groove,
        // a break and the groove again: the fit alone likes a 16-bar intro.
        let mut levels = vec![[0.05, 0.4, 0.4]; 12];
        levels.extend(vec![[0.6, 0.45, 0.45]; 4]);
        levels.extend(vec![[1.0, 1.0, 1.0]; 32]);
        levels.extend(vec![[0.1, 0.6, 0.6]; 16]);
        levels.extend(vec![[1.0, 1.0, 1.0]; 16]);
        let cuts = refine(&levels, &segment(&levels));
        assert_eq!(cuts.get(1), Some(&12), "{cuts:?}");
        // The others are where the bass changes already.
        assert!(cuts.contains(&48) && cuts.contains(&64), "{cuts:?}");
    }

    #[test]
    fn the_outro_starts_after_the_last_strong_section() {
        // Intro, drop, then the groove carries on until a quiet tail.
        let mut levels = vec![[0.3, 0.4, 0.4]; 16];
        levels.extend(vec![[1.0, 1.0, 1.0]; 16]);
        levels.extend(vec![[0.75, 0.75, 0.8]; 24]);
        levels.extend(vec![[0.1, 0.2, 0.2]; 16]);
        let cuts = [0, 16, 32, 56];
        use PhraseKind::*;
        assert_eq!(classify(&levels, &cuts), [Intro, Drop, Chorus, Outro]);
    }

    #[test]
    fn a_strong_section_short_of_the_peak_is_a_chorus() {
        // Intro, drop, main groove, break, drop, outro: 16 bars each, 8 for the break.
        let mut levels = vec![[0.3, 0.4, 0.4]; 16];
        levels.extend(vec![[1.0, 1.0, 1.0]; 16]);
        levels.extend(vec![[0.75, 0.75, 0.8]; 16]);
        levels.extend(vec![[0.1, 0.6, 0.6]; 8]);
        levels.extend(vec![[1.0, 1.0, 1.0]; 16]);
        levels.extend(vec![[0.3, 0.4, 0.4]; 16]);
        let cuts = [0, 16, 32, 48, 56, 72];
        use PhraseKind::*;
        assert_eq!(
            classify(&levels, &cuts),
            [Intro, Drop, Chorus, Breakdown, Drop, Outro]
        );
    }
}
