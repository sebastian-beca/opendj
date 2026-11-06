//! Settings › Collection › General: the storage bar and its Manage menu.

use super::*;

impl Prefs {
    /// Settings › Collection › General › Storage: fills StorageUi with the
    /// disk's parts and returns the row that shows them.
    pub(super) fn storage_item(&self) -> PrefItem {
        let mut folders = settings::music_folders();
        folders.extend(settings::albums_folder());
        folders.extend(settings::mixed_sets_folder());
        let music = crate::storage::music_bytes(folders.clone());
        let (_, analysis) = cache::analysis_usage();
        let (_, stems) = cache::stems_usage();
        let (played, _) = crate::sources::cache_usage();
        let disk = folders
            .first()
            .cloned()
            .or_else(cache::cache_dir)
            .and_then(|p| crate::storage::disk(&p));
        let ours = music.unwrap_or(0) + analysis + stems + played;
        let whole = disk.map_or(ours.max(1), |(total, _)| total.max(1)) as f32;
        let part = |name: &str, bytes: u64, tint: slint::Color| StoragePart {
            name: name.into(),
            size: size(bytes).into(),
            share: bytes as f32 / whole,
            tint,
        };
        let rgb = slint::Color::from_rgb_u8;
        let mut parts = vec![
            match music {
                Some(bytes) => part("Music", bytes, rgb(0xb6, 0x9c, 0xff)),
                None => StoragePart {
                    size: "measuring…".into(),
                    ..part("Music", 0, rgb(0xb6, 0x9c, 0xff))
                },
            },
            part("Analysis", analysis, rgb(0x5a, 0xc8, 0xfa)),
            part("Stems", stems, rgb(0xff, 0x9f, 0x0a)),
            part("Played tracks", played, rgb(0x30, 0xd1, 0x58)),
        ];
        let mut summary = String::new();
        if let Some((total, free)) = disk {
            let other = total.saturating_sub(free).saturating_sub(ours);
            parts.push(part("Other", other, rgb(0x63, 0x63, 0x66)));
            summary = format!("{} free of {}", size(free), size(total));
        }
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<StorageUi>();
            g.set_free_share((1.0 - parts.iter().map(|p| p.share).sum::<f32>()).max(0.0));
            g.set_parts(ModelRc::new(VecModel::from(parts)));
            g.set_summary(summary.into());
            g.set_actions(strings(&STORAGE_ACTIONS));
            let (confirm, warning, label) = match self.storage_pending {
                Some(0) => (
                    format!("Clear the analysis cache ({})?", size(analysis)),
                    "Every track is analyzed again when it next loads: its waveform, tempo \
                     and key. Cues and tags stay in your collection.",
                    "Clear",
                ),
                Some(1) => (
                    format!("Clear high quality stems ({})?", size(stems)),
                    "Preparing them again takes a while for each track. Tracks still get \
                     quick stems when they load.",
                    "Clear",
                ),
                Some(2) => (
                    format!("Clear played tracks ({})?", size(played)),
                    "Tracks played from your servers download again when next played. \
                     Tracks you added to your collection stay.",
                    "Clear",
                ),
                _ => (String::new(), "", ""),
            };
            g.set_confirm(confirm.into());
            g.set_warning(warning.into());
            g.set_confirm_label(label.into());
            g.set_note(self.storage_note.clone().into());
        }
        item(
            "storage_manage",
            STORAGE,
            "Storage",
            "Clear the analysis cache, stems and played tracks",
        )
    }

    /// Manage › a clean-up: asks first, but Show cache folder just opens it.
    pub(super) fn storage_picked(&mut self, index: usize) {
        self.storage_note.clear();
        if index == 3 {
            self.storage_pending = None;
            if let Some(dir) = cache::cache_dir() {
                crate::files::reveal(&dir);
            }
        } else {
            self.storage_pending = Some(index);
        }
        self.refresh();
    }

    /// The yes to the clean-up picked from Manage.
    pub(super) fn storage_confirmed(&mut self) {
        let Some(index) = self.storage_pending.take() else {
            return;
        };
        let (_, before) = match index {
            0 => cache::analysis_usage(),
            1 => cache::stems_usage(),
            _ => (0, 0),
        };
        self.storage_note = match index {
            0 => {
                let n = cache::clear_analysis();
                format!(
                    "Cleared {} from {n} {}.",
                    size(before),
                    if n == 1 { "analysis" } else { "analyses" }
                )
            }
            1 => {
                let n = cache::clear_stems();
                format!(
                    "Cleared {} of stems from {n} {}.",
                    size(before),
                    if n == 1 { "track" } else { "tracks" }
                )
            }
            _ => match crate::sources::clear_cache() {
                0 => "There were no played tracks to clear.".into(),
                freed => format!("Cleared {} of played tracks.", size(freed)),
            },
        };
        self.refresh();
    }
}
