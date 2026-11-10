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

/// The color schemes, in Settings order.
pub const COLOR_LABELS: [&str; 3] = ["RGB", "3-Band", "Blue"];
/// The stacked bands' palettes, in [`BAND_PALETTES`] order.
pub const PALETTE_LABELS: [&str; BAND_PALETTES.len()] = ["Blue-Orange-White", "Lilac-Cyan-White"];
pub const SHAPE_LABELS: [&str; 2] = ["Full", "Half"];

static COLOR: AtomicU32 = AtomicU32::new(0);
static PALETTE: AtomicU32 = AtomicU32::new(0);
static HALF: AtomicU32 = AtomicU32::new(0);
static VERSION: AtomicU32 = AtomicU32::new(1);

/// Reads the saved choices; called once at start.
pub fn load() {
    let read = |key, len: usize| (settings::number(key, 0.0).max(0.0) as usize).min(len - 1) as u32;
    COLOR.store(read(COLOR_KEY, COLOR_LABELS.len()), Ordering::Relaxed);
    PALETTE.store(read(PALETTE_KEY, PALETTE_LABELS.len()), Ordering::Relaxed);
    HALF.store(read(HALF_KEY, SHAPE_LABELS.len()), Ordering::Relaxed);
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
}
