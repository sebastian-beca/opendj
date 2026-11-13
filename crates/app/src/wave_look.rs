//! How the decks' waveforms look: the colour scheme, the stacked bands'
//! palette, and whether the overview is drawn whole or from the bottom up.
//!
//! Picked in Settings › Decks and kept here, so the renderer can read it
//! every frame without reading the settings file; [`version`] changes with
//! every pick so the renderer applies it once.

use std::sync::atomic::{AtomicU32, Ordering};

use opendj_waveform::{Palette, Style, BAND_PALETTES};

use crate::settings;

pub const COLOR_KEY: &str = "waveform_color";
pub const PALETTE_KEY: &str = "waveform_palette";
pub const HALF_KEY: &str = "waveform_overview_half";
/// Settings › Decks › Size follows gain, off at first.
pub const GAIN_KEY: &str = "waveform_follows_gain";
/// Settings › Decks › Show vocals, off at first. A new key, so the
/// earlier "waveform_show_vocals" (on by default) no longer turns it on.
pub const VOCALS_KEY: &str = "waveform_vocal_tint";

/// The color schemes, in Settings order.
pub const COLOR_LABELS: [&str; 4] = ["RGB", "3-Band", "Blue", "Stems"];
/// The stacked bands' palettes, in [`BAND_PALETTES`] order.
pub const PALETTE_LABELS: [&str; BAND_PALETTES.len()] = ["Blue-Orange-White", "Lilac-Cyan-White"];
pub const SHAPE_LABELS: [&str; 2] = ["Full", "Half"];

static COLOR: AtomicU32 = AtomicU32::new(0);
static PALETTE: AtomicU32 = AtomicU32::new(0);
static HALF: AtomicU32 = AtomicU32::new(0);
static VERSION: AtomicU32 = AtomicU32::new(1);
static GAIN: AtomicU32 = AtomicU32::new(0);
static VOCALS: AtomicU32 = AtomicU32::new(0);

/// Reads the saved choices; called once at start.
pub fn load() {
    let read = |key, len: usize| (settings::number(key, 0.0).max(0.0) as usize).min(len - 1) as u32;
    COLOR.store(read(COLOR_KEY, COLOR_LABELS.len()), Ordering::Relaxed);
    PALETTE.store(read(PALETTE_KEY, PALETTE_LABELS.len()), Ordering::Relaxed);
    HALF.store(read(HALF_KEY, SHAPE_LABELS.len()), Ordering::Relaxed);
    GAIN.store(settings::flag(GAIN_KEY, false) as u32, Ordering::Relaxed);
    VOCALS.store(settings::flag(VOCALS_KEY, false) as u32, Ordering::Relaxed);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

fn pick(cell: &AtomicU32, key: &str, index: usize, len: usize) {
    let i = index.min(len - 1) as u32;
    cell.store(i, Ordering::Relaxed);
    settings::set(key, serde_json::json!(i));
    VERSION.fetch_add(1, Ordering::Relaxed);
}

pub fn choose_color(index: usize) {
    pick(&COLOR, COLOR_KEY, index, COLOR_LABELS.len());
}

pub fn choose_palette(index: usize) {
    pick(&PALETTE, PALETTE_KEY, index, PALETTE_LABELS.len());
}

pub fn choose_half(index: usize) {
    pick(&HALF, HALF_KEY, index, SHAPE_LABELS.len());
}

/// Size follows gain was switched; saved by the caller.
pub fn set_follows_gain(on: bool) {
    GAIN.store(on as u32, Ordering::Relaxed);
}

pub fn follows_gain() -> bool {
    GAIN.load(Ordering::Relaxed) == 1
}

/// Show vocals was switched; saved by the caller.
pub fn set_show_vocals(on: bool) {
    VOCALS.store(on as u32, Ordering::Relaxed);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Whether the waveforms tint the sung parts green.
pub fn show_vocals() -> bool {
    VOCALS.load(Ordering::Relaxed) == 1
}

/// How tall a deck's waveforms are drawn for its gain knob (1 at the
/// middle, 0 to 4): unchanged unless Size follows gain is on, and then kept
/// between a quarter and twice the height so a waveform never vanishes.
pub fn gain_scale(pregain: f64) -> f32 {
    if follows_gain() {
        pregain.clamp(0.25, 2.0) as f32
    } else {
        1.0
    }
}

/// Which of [`COLOR_LABELS`] is in use.
pub fn color() -> usize {
    COLOR.load(Ordering::Relaxed) as usize
}

/// Which of [`PALETTE_LABELS`] is in use.
pub fn palette_index() -> usize {
    PALETTE.load(Ordering::Relaxed) as usize
}

/// Whether the overview is drawn from the bottom up.
pub fn half() -> bool {
    HALF.load(Ordering::Relaxed) == 1
}

/// Changes whenever a choice does.
pub fn version() -> u32 {
    VERSION.load(Ordering::Relaxed)
}

/// The waveform style for the color scheme in use.
pub fn style() -> Style {
    match color() {
        1 => Style::Bands,
        2 => Style::Blue,
        3 => Style::Stems,
        _ => Style::Rgb,
    }
}

/// The band colours: RGB keeps its red, amber and blue; 3-Band uses the
/// palette picked.
pub fn palette() -> Palette {
    match style() {
        Style::Bands => Palette::bands(palette_index()),
        _ => Palette::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_sets_style_and_palette() {
        COLOR.store(1, Ordering::Relaxed);
        PALETTE.store(1, Ordering::Relaxed);
        assert_eq!(style(), Style::Bands);
        assert_eq!(palette(), Palette::bands(1));
        COLOR.store(0, Ordering::Relaxed);
        assert_eq!(style(), Style::Rgb);
        assert_eq!(palette(), Palette::default());
    }

    #[test]
    fn gain_sizes_only_when_on() {
        set_follows_gain(false);
        assert_eq!(gain_scale(2.0), 1.0);
        set_follows_gain(true);
        assert_eq!(gain_scale(1.5), 1.5);
        assert_eq!(gain_scale(0.0), 0.25);
        assert_eq!(gain_scale(4.0), 2.0);
        set_follows_gain(false);
    }
}
