//! Instruments' and inputs' own images, shown on the cover of a deck
//! playing them (`Cover` in `ui/deck.slint`). An image file dropped on such
//! a deck, or chosen from its cover's right-click menu, is copied to
//! `Instruments/Art/` in the settings folder, named after the plugin's key
//! or the input; without one the deck shows its icon on the deck color,
//! the same square a track without a cover gets.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use slint::{ComponentHandle, DataTransfer, Image, ModelRc, VecModel};

use crate::{settings, AppWindow, InstrumentsUi, TrackDrag};

/// File types taken as images.
const IMAGE_TYPES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// The inputs' names for their files, in `InstrumentsUi.input-art` order:
/// the microphone and aux inputs 1 to 3.
const INPUTS: [&str; 4] = ["microphone", "aux-1", "aux-2", "aux-3"];

thread_local! {
    /// The ticked plugins' keys, in the picker's order.
    static PLUGINS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn root() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("Instruments").join("Art"))
}

/// A file name from a key: its last part (the plugin's id, or the
/// input's name) with letters, digits, `-` and `.` kept, then a hash of all
/// of it, since plugin keys hold long paths.
fn stem(key: &str) -> String {
    let tail = key.rsplit(['#', ':', '/']).next().unwrap_or(key);
    let readable: String = tail
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    format!("{readable}-{hash:016x}")
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_TYPES.contains(&e.to_ascii_lowercase().as_str()))
}

/// The image file kept for `key` in `dir`, if any.
fn find(dir: &Path, key: &str) -> Option<PathBuf> {
    let stem = stem(key);
    IMAGE_TYPES
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

/// Keeps a copy of `image` in `dir` as `key`'s, in place of any before.
fn keep(dir: &Path, key: &str, image: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    forget(dir, key);
    let ext = image
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let to = dir.join(format!("{}.{ext}", stem(key)));
    std::fs::copy(image, &to)?;
    Ok(to)
}

fn forget(dir: &Path, key: &str) {
    while let Some(old) = find(dir, key) {
        if std::fs::remove_file(&old).is_err() {
            break;
        }
    }
}

/// The key `live` (see `InstrumentsUi.live`) keeps its image under, with
/// `instrument` the plugin picked (from 1).
fn key_of(live: i32, instrument: i32) -> Option<String> {
    match live {
        3 => PLUGINS.with_borrow(|p| {
            usize::try_from(instrument - 1)
                .ok()
                .and_then(|i| p.get(i).cloned())
        }),
        1 => Some(INPUTS[0].into()),
        2 => Some(INPUTS[1].into()),
        4 => Some(INPUTS[2].into()),
        5 => Some(INPUTS[3].into()),
        _ => None,
    }
}

fn load(dir: Option<&Path>, key: &str) -> Option<Image> {
    let path = find(dir?, key)?;
    match Image::load_from_path(&path) {
        Ok(image) => Some(image),
        Err(e) => {
            eprintln!("instrument art {}: {e:?}", path.display());
            None
        }
    }
}

fn set_list(images: Vec<Option<Image>>, set: impl Fn(ModelRc<Image>, ModelRc<bool>)) {
    let has: Vec<bool> = images.iter().map(Option::is_some).collect();
    let images: Vec<Image> = images.into_iter().map(Option::unwrap_or_default).collect();
    set(
        ModelRc::new(VecModel::from(images)),
        ModelRc::new(VecModel::from(has)),
    );
}

/// Shows each plugin's and input's image again.
fn refresh(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    let dir = root();
    let plugins = PLUGINS.with_borrow(|p| p.iter().map(|k| load(dir.as_deref(), k)).collect());
    set_list(plugins, |art, has| {
        ui.set_plugin_art(art);
        ui.set_plugin_has_art(has);
    });
    let inputs = INPUTS.iter().map(|k| load(dir.as_deref(), k)).collect();
    set_list(inputs, |art, has| {
        ui.set_input_art(art);
        ui.set_input_has_art(has);
    });
}

/// The ticked plugins changed: `keys` in the picker's order.
pub fn show_plugins(app: &AppWindow, keys: Vec<String>) {
    PLUGINS.set(keys);
    refresh(app);
}

/// Keeps `image` as `live`'s and shows it.
fn set(app: &AppWindow, live: i32, instrument: i32, image: &Path) {
    let (Some(dir), Some(key)) = (root(), key_of(live, instrument)) else {
        return;
    };
    match keep(&dir, &key, image) {
        Ok(to) => eprintln!("instrument art for {key}: {}", to.display()),
        Err(e) => eprintln!("instrument art for {key}: {e}"),
    }
    refresh(app);
}

/// Hooks up the drop, choose and remove callbacks. `take_files` gives the
/// files of the last drop from outside (see `crate::file_drop`).
pub fn install(app: &AppWindow, take_files: impl Fn() -> Vec<PathBuf> + 'static) {
    let ui = app.global::<InstrumentsUi>();
    let weak = app.as_weak();
    ui.on_drop_art(move |deck, live, instrument| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let files = take_files();
        match files.iter().find(|p| is_image(p)) {
            Some(image) => set(&app, live, instrument, image),
            // Not an image: a track to load, as on any deck.
            None if !files.is_empty() => {
                let mut data = DataTransfer::default();
                data.set_file_paths(files);
                app.global::<TrackDrag>().invoke_drop(deck, data);
            }
            None => {}
        }
    });
    let weak = app.as_weak();
    ui.on_choose_art(move |live, instrument| {
        let weak = weak.clone();
        crate::prefs::choose_file("Choose an image", move |path| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    if is_image(&path) {
                        set(&app, live, instrument, &path);
                    }
                }
            });
        });
    });
    let weak = app.as_weak();
    ui.on_remove_art(move |live, instrument| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if let (Some(dir), Some(key)) = (root(), key_of(live, instrument)) {
            forget(&dir, &key);
        }
        refresh(&app);
    });
    refresh(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_is_kept_under_its_key_in_place_of_the_last() {
        let dir = std::env::temp_dir().join(format!("opendj-art-{}", std::process::id()));
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let png = src.join("Juno.PNG");
        let jpg = src.join("juno.jpg");
        std::fs::write(&png, b"png").unwrap();
        std::fs::write(&jpg, b"jpg").unwrap();
        let art = dir.join("art");
        let key = "clap:/Library/Audio/Plug-Ins/CLAP/Juno-6 V.clap#com.arturia.Juno-6 V";
        assert!(find(&art, key).is_none());
        let kept = keep(&art, key, &png).unwrap();
        let name = kept.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            name.starts_with("com.arturia.Juno-6_V-") && name.ends_with(".png"),
            "{name}"
        );
        assert_ne!(
            stem(key),
            stem("clap:/elsewhere/Juno-6 V.clap#com.arturia.Juno-6 V")
        );
        assert_eq!(find(&art, key), Some(kept.clone()));
        // A new one replaces it, whatever its type.
        let kept = keep(&art, key, &jpg).unwrap();
        assert_eq!(find(&art, key), Some(kept));
        assert_eq!(std::fs::read_dir(&art).unwrap().count(), 1);
        forget(&art, key);
        assert!(find(&art, key).is_none());
        assert!(is_image(&png) && !is_image(&src.join("track.mp3")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
