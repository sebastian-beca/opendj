//! VirtualDJ mapper files.

use super::Import;

pub(super) fn import(_text: &str) -> Result<Import, String> {
    Err("VirtualDJ mappings aren't read yet".into())
}
