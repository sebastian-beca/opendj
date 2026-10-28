//! openDJ's own mappings: copies of a Mixxx mapping that the DJ names and
//! will be able to change. Each is a small JSON file next to the settings
//! (`controller-mappings/<id>.json`) holding its name, the Mixxx mapping it
//! is based on and the changes made to it. The Mixxx mapping itself is
//! never written to, so the original can always be gone back to.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// One of the DJ's own mappings.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnMapping {
    /// Its file name without `.json`.
    pub id: String,
    pub name: String,
    /// The Mixxx mapping it is a copy of, by file name.
    pub based_on: String,
    /// The changes made to it; empty until editing comes.
    pub changes: Vec<Value>,
}

impl OwnMapping {
    fn read(path: &Path) -> Option<Self> {
        let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        Some(OwnMapping {
            id: path.file_stem()?.to_string_lossy().into_owned(),
            name: v["name"].as_str()?.to_string(),
            based_on: v["based_on"].as_str()?.to_string(),
            changes: v["changes"].as_array().cloned().unwrap_or_default(),
        })
    }

    fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let v = json!({
            "name": self.name,
            "based_on": self.based_on,
            "changes": self.changes,
        });
        std::fs::write(
            dir.join(format!("{}.json", self.id)),
            serde_json::to_vec_pretty(&v)?,
        )
    }
}

/// The own mappings in `dir` based on the Mixxx mapping `based_on`, by name.
pub fn list(dir: &Path, based_on: &str) -> Vec<OwnMapping> {
    let mut out: Vec<OwnMapping> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| OwnMapping::read(&p))
        .filter(|m| m.based_on == based_on)
        .collect();
    out.sort_by_key(|m| m.name.to_lowercase());
    out
}

/// A name for a copy of `name` that no mapping in `taken` has yet:
/// "Pioneer DDJ-FLX4 copy", then "… copy 2".
pub fn copy_name(name: &str, taken: &[String]) -> String {
    let base = format!("{name} copy");
    (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} {n}")
            }
        })
        .find(|n| !taken.contains(n))
        .unwrap_or(base)
}

/// Makes a new own mapping called `name`, a copy of `from` (its changes
/// too), based on the Mixxx mapping `based_on`.
pub fn create(
    dir: &Path,
    name: &str,
    based_on: &str,
    from: Option<&OwnMapping>,
) -> std::io::Result<OwnMapping> {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let stem = if stem.is_empty() {
        "mapping".to_string()
    } else {
        stem
    };
    let id = (1..)
        .map(|n| {
            if n == 1 {
                stem.clone()
            } else {
                format!("{stem}-{n}")
            }
        })
        .find(|id| !path(dir, id).exists())
        .unwrap_or(stem);
    let m = OwnMapping {
        id,
        name: name.to_string(),
        based_on: based_on.to_string(),
        changes: from.map(|f| f.changes.clone()).unwrap_or_default(),
    };
    m.write(dir)?;
    Ok(m)
}

pub fn rename(dir: &Path, id: &str, name: &str) -> std::io::Result<()> {
    let mut m = OwnMapping::read(&path(dir, id))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such mapping"))?;
    m.name = name.to_string();
    m.write(dir)
}

pub fn delete(dir: &Path, id: &str) -> std::io::Result<()> {
    std::fs::remove_file(path(dir, id))
}

fn path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_rename_and_delete() {
        let dir = std::env::temp_dir().join(format!("opendj-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let base = "Pioneer-DDJ-FLX4.midi.xml";
        assert!(list(&dir, base).is_empty());
        let a = create(&dir, "FLX4 house", base, None).unwrap();
        assert_eq!(a.id, "flx4-house");
        let b = create(&dir, "FLX4 house", base, Some(&a)).unwrap();
        assert_eq!(b.id, "flx4-house-2");
        create(&dir, "K2", "Allen and Heath Xone K2.midi.xml", None).unwrap();
        assert_eq!(list(&dir, base).len(), 2);
        rename(&dir, &b.id, "FLX4 techno").unwrap();
        let names: Vec<String> = list(&dir, base).into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["FLX4 house", "FLX4 techno"]);
        assert_eq!(copy_name("FLX4 house", &names), "FLX4 house copy");
        assert_eq!(copy_name("FLX4", &["FLX4 copy".to_string()]), "FLX4 copy 2");
        delete(&dir, &a.id).unwrap();
        assert_eq!(list(&dir, base).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
