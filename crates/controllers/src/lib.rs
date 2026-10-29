//! MIDI controller support driven by Mixxx's own controller mappings.
//!
//! Mixxx ships a mapping for each supported controller: a `.midi.xml` file
//! binding MIDI messages to controls or script functions, plus JavaScript
//! that implements anything beyond a direct binding (jog wheels, shift
//! layers, LEDs). This crate runs those files unchanged: the XML is read as
//! Mixxx reads it, and the scripts run in QuickJS against the same `engine`
//! and `midi` API, talking to the shared control registry.

pub mod catalog;
mod controller;
pub mod edit;
pub mod export;
pub mod format;
mod host;
pub mod import;
pub mod layout;
mod manager;
pub mod mapping;
pub mod own;
pub mod probe;
mod rows;
mod script;
pub mod view;

use std::path::PathBuf;

pub use controller::Controller;
pub use host::{LAYER_GROUP, LAYER_KEYS};
pub use manager::{
    device_key, list_mappings, match_mapping, ConnectedController, Controllers, MappingChoice, Port,
};
pub use mapping::Mapping;

/// Where Mixxx's mappings are: `OPENDJ_CONTROLLERS` if set, otherwise next
/// to the executable (`../Resources/controllers` in a macOS bundle,
/// `controllers/` beside it, or `../share/opendj/controllers` in a Linux
/// install or AppImage), otherwise the repository's `res/controllers`.
pub fn default_mappings_dir() -> Option<PathBuf> {
    let valid = |p: &PathBuf| p.join(mapping::REQUIRED_SCRIPT).is_file();
    if let Some(dir) = std::env::var_os("OPENDJ_CONTROLLERS") {
        return Some(PathBuf::from(dir)).filter(valid);
    }
    let mut candidates = Vec::new();
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
    {
        candidates.push(exe_dir.join("../Resources/controllers"));
        candidates.push(exe_dir.join("controllers"));
        candidates.push(exe_dir.join("../share/opendj/controllers"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../res/controllers"));
    candidates.into_iter().find(valid)
}
