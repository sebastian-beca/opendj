//! Track metadata, decoded audio and waveform summaries.

use std::fmt;
use std::path::PathBuf;

use crate::beats::BeatGrid;
use crate::cover::Cover;
use crate::loudness::Loudness;
use crate::stems::Stems;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

/// Slices per second of audio in the finest level of a waveform summary.
pub const WAVEFORM_RATE: f64 = 1000.0;

/// One waveform slice: the peak of the low, mid and high bands and of the
/// whole signal, then their RMS in the same order. Each is the square root of
/// the level relative to the track's loudest peak, scaled to 0..255, which
/// keeps detail in quiet passages.
pub type WaveformBin = [u8; 8];

/// A track's waveform summary at several resolutions.
#[derive(Clone, Debug, Default)]
pub struct Waveform {
    /// Slices per second in `levels[0]`.
    pub bins_per_second: f64,
    /// `levels[0]` is the full resolution; each further level merges pairs of
    /// slices of the one before (peaks by maximum, RMS by power), down to a
    /// single slice (see [`Waveform::from_finest`]). Zoomed-out views read a coarser level instead of
    /// scanning thousands of slices per pixel.
    pub levels: Vec<Vec<WaveformBin>>,
}

impl Waveform {
    /// A waveform from its finest level; each coarser level merges pairs of
    /// slices of the one before, keeping the larger peak and the mean power.
    pub fn from_finest(bins_per_second: f64, finest: Vec<WaveformBin>) -> Self {
        // Bins hold the square root of the level; power is its square.
        let power = |b: u8| (b as f32 / 255.0).powi(4);
        let unpower = |p: f32| (p.sqrt().sqrt() * 255.0).round() as u8;
        let mut levels = vec![finest];
        while levels.last().unwrap().len() > 1 {
            let next = levels
                .last()
                .unwrap()
                .chunks(2)
                .map(|c| {
                    let (a, b) = (c[0], *c.last().unwrap());
                    std::array::from_fn(|k| {
                        if k < 4 {
                            a[k].max(b[k])
                        } else {
                            unpower((power(a[k]) + power(b[k])) * 0.5)
                        }
                    })
                })
                .collect();
            levels.push(next);
        }
        Waveform {
            bins_per_second,
            levels,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.levels.first().is_none_or(|l| l.is_empty())
    }

    /// The whole track in `slices` slices, each the loudest peak and the
    /// mean RMS of its part of the finest level, on the same square-root
    /// scale: what the library draws as a track's mini waveform. Empty
    /// when the track isn't analysed.
    pub fn overview(&self, slices: usize) -> Vec<OverviewSlice> {
        let Some(finest) = self.levels.first().filter(|l| !l.is_empty()) else {
            return Vec::new();
        };
        let power = |b: u8| (b as f32 / 255.0).powi(4);
        (0..slices)
            .map(|i| {
                let from = i * finest.len() / slices;
                let to = ((i + 1) * finest.len() / slices).max(from + 1);
                let part = &finest[from.min(finest.len() - 1)..to.min(finest.len())];
                let peak = part.iter().map(|b| b[3]).max().unwrap_or(0);
                let mean = part.iter().map(|b| power(b[7])).sum::<f32>() / part.len() as f32;
                [peak, (mean.sqrt().sqrt() * 255.0).round() as u8]
            })
            .collect()
    }
}

/// One slice of a track's overview: its peak, then its RMS (see
/// [`Waveform::overview`]).
pub type OverviewSlice = [u8; 2];

/// Where a track's audio comes from.
#[derive(Clone, Debug)]
pub enum Source {
    /// Generated on load by [`crate::synth`], for the built-in sample library.
    Synth {
        seed: u64,
    },
    File(PathBuf),
}

/// Frames per chunk of [`Audio`] (about 1.5 s at 44.1 kHz).
const CHUNK: usize = 1 << 16;

/// One chunk of decoded frames, set once when it has been decoded.
type Chunk = OnceLock<Box<[[f32; 2]]>>;

/// Decoded stereo audio at the file's own sample rate.
///
/// A track can play while it is still being decoded, as in Mixxx: an
/// [`AudioWriter`] fills it in order, one chunk at a time, and frames that
/// aren't decoded yet read as silence. Decoding runs hundreds of times faster
/// than playback, so in practice only a seek far ahead in the first second
/// after loading can land on them.
pub struct Audio {
    pub sample_rate: u32,
    chunks: Box<[Chunk]>,
    /// Frames in the track: the decoder's estimate until decoding finishes.
    len: AtomicUsize,
    /// Frames decoded so far.
    decoded: AtomicUsize,
    complete: AtomicBool,
    /// The track split into stems, when it has them; set once they start
    /// decoding, possibly while the track is already playing.
    stems: OnceLock<Arc<Stems>>,
    /// How loud the track is, once analysed; auto gain levels it.
    loudness: OnceLock<Loudness>,
}

impl Audio {
    /// Audio that is already fully decoded.
    pub fn from_frames(sample_rate: u32, frames: Vec<[f32; 2]>) -> Self {
        let (audio, mut writer) = Audio::writer(sample_rate, frames.len());
        writer.extend(&frames);
        drop(writer.finish());
        Arc::try_unwrap(audio).unwrap_or_else(|_| unreachable!("the writer is gone"))
    }

    /// Empty audio of about `expected_frames` frames, and the writer that fills
    /// it. Frames beyond one chunk past the estimate are dropped.
    pub fn writer(sample_rate: u32, expected_frames: usize) -> (Arc<Audio>, AudioWriter) {
        let chunks = (0..expected_frames / CHUNK + 2)
            .map(|_| OnceLock::new())
            .collect();
        let audio = Arc::new(Audio {
            sample_rate,
            chunks,
            len: AtomicUsize::new(expected_frames),
            decoded: AtomicUsize::new(0),
            complete: AtomicBool::new(false),
            stems: OnceLock::new(),
            loudness: OnceLock::new(),
        });
        let writer = AudioWriter {
            audio: audio.clone(),
            buf: Vec::with_capacity(CHUNK),
            chunk: 0,
        };
        (audio, writer)
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Frames decoded so far; all of them once [`Audio::is_complete`].
    pub fn decoded(&self) -> usize {
        self.decoded.load(Ordering::Acquire)
    }

    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// Frame `i`, or silence if it isn't decoded yet or is past the end.
    #[inline]
    pub fn frame(&self, i: usize) -> [f32; 2] {
        self.chunks
            .get(i / CHUNK)
            .and_then(OnceLock::get)
            .and_then(|c| c.get(i % CHUNK))
            .copied()
            .unwrap_or([0.0; 2])
    }

    /// The decoded frames, in order, as chunks.
    pub fn decoded_chunks(&self) -> impl Iterator<Item = &[[f32; 2]]> {
        self.chunks.iter().map_while(|c| c.get().map(|c| &c[..]))
    }

    pub fn duration(&self) -> f64 {
        self.len() as f64 / self.sample_rate as f64
    }

    /// The track's stems, if it has them.
    pub fn stems(&self) -> Option<&Arc<Stems>> {
        self.stems.get()
    }

    /// How loud the track is, once it has been analysed.
    pub fn loudness(&self) -> Option<Loudness> {
        self.loudness.get().copied()
    }

    /// Sets how loud the track is; the first measurement stays.
    pub fn set_loudness(&self, loudness: Loudness) {
        let _ = self.loudness.set(loudness);
    }

    /// Gives the audio its stems, which must be at its sample rate. False if
    /// it already has stems or the rate differs.
    pub fn set_stems(&self, stems: Arc<Stems>) -> bool {
        stems.sample_rate == self.sample_rate && self.stems.set(stems).is_ok()
    }
}

impl fmt::Debug for Audio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Audio({} frames @ {} Hz)", self.len(), self.sample_rate)
    }
}

/// Fills an [`Audio`] while it may already be playing.
pub struct AudioWriter {
    audio: Arc<Audio>,
    buf: Vec<[f32; 2]>,
    chunk: usize,
}

impl AudioWriter {
    pub fn push(&mut self, frame: [f32; 2]) {
        self.buf.push(frame);
        if self.buf.len() == CHUNK {
            self.flush();
        }
    }

    pub fn extend(&mut self, frames: &[[f32; 2]]) {
        for &f in frames {
            self.push(f);
        }
    }

    /// The audio being written.
    pub fn audio(&self) -> &Arc<Audio> {
        &self.audio
    }

    /// Publishes the frames buffered so far as a chunk.
    fn flush(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let frames = std::mem::replace(&mut self.buf, Vec::with_capacity(CHUNK));
        let n = frames.len();
        if let Some(slot) = self.audio.chunks.get(self.chunk) {
            let _ = slot.set(frames.into_boxed_slice());
            self.chunk += 1;
            let decoded = self.audio.decoded.fetch_add(n, Ordering::AcqRel) + n;
            // Play on past the estimate while frames keep coming.
            self.audio.len.fetch_max(decoded, Ordering::AcqRel);
        }
    }

    /// Publishes the last frames and sets the audio's true length.
    pub fn finish(mut self) -> Arc<Audio> {
        self.flush();
        let a = &self.audio;
        a.len
            .store(a.decoded.load(Ordering::Acquire), Ordering::Release);
        a.complete.store(true, Ordering::Release);
        self.audio
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub id: usize,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub genre: String,
    /// 0..5 stars.
    pub rating: u8,
    /// Tempo from tags or analysis; 0 when unknown until the track is analysed.
    pub bpm: f64,
    pub key: String,
    pub duration: f64,
    /// Offset of the first beat in seconds.
    pub first_beat: f64,
    /// Where every beat falls, when the tempo changes within the track or
    /// the grid was edited; empty means a steady `bpm` from `first_beat`.
    /// Set it with [`Track::set_grid`], which keeps `bpm` and `first_beat`
    /// in step.
    pub grid: BeatGrid,
    pub source: Source,
    /// Band peaks and RMS for drawing, as Mixxx's analyzer produces for its
    /// RGB waveforms. Empty until the track is loaded and analysed.
    pub waveform: Arc<Waveform>,
    /// Set on the copy of the track that is loaded on a deck.
    pub audio: Option<Arc<Audio>>,
    /// Album art, looked up when the track is loaded on a deck.
    pub cover: Option<Arc<Cover>>,
    /// What the library knows beyond the file's tags.
    pub info: LibraryInfo,
}

/// A track's details from the library: what the DJ set, or brought in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibraryInfo {
    pub comment: String,
    /// The track's grouping, e.g. "11 YOGA".
    pub grouping: String,
    /// Words the DJ tagged the track with, e.g. hashtags ("#uplifting", "#synth").
    pub labels: Vec<String>,
    /// Colour as 0xRRGGBB.
    pub color: Option<u32>,
    pub play_count: u32,
    pub hotcues: Vec<Hotcue>,
    /// The tempo and first beat came from the library and are kept, rather
    /// than replaced by this app's analysis.
    pub fixed_grid: bool,
    /// The library's grid was found by this app's analysis, so analysing
    /// the track again on request may replace it (unlike one the DJ edited
    /// or an import brought).
    pub grid_analysed: bool,
    /// Where the library's grid came from: "analysis", "user", or the app
    /// it was brought in from; empty without one.
    pub grid_from: String,
    /// The DJ locked the grid: edits and analysis leave it alone.
    pub grid_locked: bool,
    /// The analysed grid fits the beats poorly: worth checking by ear.
    pub grid_unsure: bool,
    /// When the track joined the library, as a Unix time (0 if unknown).
    pub added: i64,
    /// The library has the track but its file is not on disk.
    pub missing: bool,
    /// Whether `key` was found by this app's analysis, so a different key
    /// detector may replace it; false for keys from tags or another app.
    /// None when not known (libraries from before this was kept).
    pub key_detected: Option<bool>,
    /// The remixer or mix name, e.g. "Club Mix".
    pub remix: String,
    /// The year, or a full date, as the tags give it.
    pub year: String,
    /// Its number on the album; 0 for none.
    pub track_number: u32,
    /// The DJ's own tag fields, from Tag 1 up: words such as moods
    /// ("#uplifting") in one and instruments ("#synth #vox") in another.
    /// VirtualDJ's User 1 and User 2 are the first two. What each field is
    /// called is the DJ's choice (see the app's settings).
    pub user_tags: Vec<String>,
    /// A gain the DJ set for this track, in dB; the deck's gain knob is set
    /// to it when the track loads.
    pub gain_db: Option<f32>,
    /// When it was last played, as a Unix time.
    pub last_played: Option<i64>,
    /// Its disc on the release; 0 for none.
    pub disc_number: u32,
    /// Record label.
    pub label: String,
    pub composer: String,
    /// Who remixed it, where the tags keep that apart from the mix name.
    pub remixer: String,
    pub isrc: String,
    /// Named sections, phrases and mix points, in time order.
    pub markers: Vec<Marker>,
    /// Where the track is on its way into the collection: "inbox",
    /// "triage", "collection", "dismissed" (passed over in the inbox), or
    /// empty when the DJ hasn't set up the stage folders.
    pub stage: String,
}

impl LibraryInfo {
    /// Tag field `i` (from 0), empty when the track has none there.
    pub fn user_tag(&self, i: usize) -> &str {
        self.user_tags.get(i).map_or("", String::as_str)
    }

    /// Sets tag field `i` (from 0), making room for it.
    pub fn set_user_tag(&mut self, i: usize, words: String) {
        if self.user_tags.len() <= i {
            self.user_tags.resize(i + 1, String::new());
        }
        self.user_tags[i] = words;
    }

    /// The words of every tag field, as `labels` keeps them for searching.
    pub fn user_words(&self) -> Vec<String> {
        self.user_tags
            .iter()
            .flat_map(|f| f.split_whitespace())
            .map(str::to_string)
            .collect()
    }
}

/// Hot cues numbered from here up are memory cues: rekordbox's unnumbered
/// cues, which mark a place on the waveform but have no pad.
pub const MEMORY_CUE: usize = 1000;

/// A saved hot cue, or a saved loop when it has an end.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hotcue {
    /// From 0: hot cue 1 is number 0. Numbers from [`MEMORY_CUE`] up are
    /// memory cues.
    pub number: usize,
    pub seconds: f64,
    pub name: String,
    /// Where a saved loop ends; `seconds` is where it starts.
    pub end: Option<f64>,
    /// Colour as 0xRRGGBB, when one was set.
    pub color: Option<u32>,
}

impl Hotcue {
    pub fn is_memory(&self) -> bool {
        self.number >= MEMORY_CUE
    }
}

/// A point or stretch of a track that isn't a cue: a named section, a
/// phrase, or a point another app's Automix mixes at.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub kind: MarkerKind,
    pub seconds: f64,
    /// Where a section ends, when it has a length.
    pub end: Option<f64>,
    /// "Break 1", a phrase such as "Chorus", or a mix point such as
    /// "fadeStart".
    pub name: String,
    pub color: Option<u32>,
    /// Found by this app's analysis (a detected phrase), so analysing
    /// again replaces it; false for markers from an import.
    pub detected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerKind {
    /// A part of the track the DJ named (VirtualDJ's remix points).
    Section,
    /// A phrase from analysis (rekordbox's Intro, Verse, Chorus…).
    Phrase,
    /// Where Automix starts, fades or cuts (VirtualDJ's automix points).
    Automix,
}

impl MarkerKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Phrase => "phrase",
            Self::Automix => "automix",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Section, Self::Phrase, Self::Automix]
            .into_iter()
            .find(|k| k.name() == name)
    }
}

impl Track {
    /// The length of a beat at the main tempo.
    pub fn beat_length(&self) -> f64 {
        if self.bpm > 0.0 {
            60.0 / self.bpm
        } else {
            0.5
        }
    }

    /// Sets the beat grid, and the main tempo and first beat from it.
    pub fn set_grid(&mut self, grid: BeatGrid) {
        if !grid.is_empty() {
            self.bpm = grid.main_bpm(if self.duration > 0.0 {
                self.duration
            } else {
                f64::MAX
            });
            self.first_beat = grid.first_beat();
        }
        self.grid = grid;
    }

    /// The beat grid as a list of markers: the stored one, or the steady
    /// tempo from the first beat. Empty when the tempo is unknown.
    pub fn beat_grid(&self) -> BeatGrid {
        if !self.grid.is_empty() || self.bpm <= 0.0 {
            self.grid.clone()
        } else {
            BeatGrid::constant(self.bpm, self.first_beat)
        }
    }

    /// The beat number at `seconds` (0 at the first beat). The lookups below
    /// don't allocate, so the audio thread uses them.
    pub fn beat_at(&self, seconds: f64) -> f64 {
        if !self.grid.is_empty() {
            self.grid.beat_at(seconds)
        } else {
            (seconds - self.first_beat) / self.beat_length()
        }
    }

    /// When beat number `beat` falls.
    pub fn seconds_at(&self, beat: f64) -> f64 {
        if !self.grid.is_empty() {
            self.grid.seconds_at(beat)
        } else {
            self.first_beat + beat * self.beat_length()
        }
    }

    /// The tempo at `seconds`; 0 when unknown.
    pub fn bpm_at(&self, seconds: f64) -> f64 {
        if !self.grid.is_empty() {
            self.grid.bpm_at(seconds)
        } else {
            self.bpm
        }
    }

    /// The length of the beat at `seconds`.
    pub fn beat_length_at(&self, seconds: f64) -> f64 {
        match self.bpm_at(seconds) {
            b if b > 0.0 => 60.0 / b,
            _ => 0.5,
        }
    }

    /// The beat nearest `seconds`, or the one at or before it with `floor`.
    pub fn snap(&self, seconds: f64, floor: bool) -> f64 {
        let b = self.beat_at(seconds);
        self.seconds_at(if floor { (b + 1e-6).floor() } else { b.round() })
    }

    /// `seconds` on the nearest step of `beats` beats (a fraction of a beat
    /// or several), counted from the grid's first beat; with `floor` the
    /// step at or before it.
    pub fn snap_to(&self, seconds: f64, beats: f64, floor: bool) -> f64 {
        let q = self.beat_at(seconds) / beats;
        self.seconds_at(beats * if floor { (q + 1e-6).floor() } else { q.round() })
    }

    /// `seconds` moved by `beats` beats along the grid.
    pub fn beats_from(&self, seconds: f64, beats: f64) -> f64 {
        self.seconds_at(self.beat_at(seconds) + beats)
    }
}
