//! Checks GitHub once per start for a newer release, so the top of the
//! window can say so. Pre-releases are skipped (GitHub's "latest" never
//! returns them), and a version dismissed once isn't shown again.
//!
//! The macOS app and the AppImage can install the update themselves: the
//! download must match the checksum GitHub publishes for it, and on macOS
//! it must also be notarized and signed by the same developer as the
//! running app. The old app is swapped out and openDJ starts again. Other
//! installs (the Linux tarball, a build from source) get the release page.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use sha2::{Digest, Sha256};

use serde_json::Value;

use crate::diagnostics::VERSION;
use crate::settings;

const LATEST: &str = "https://api.github.com/repos/sebastian-beca/opendj/releases/latest";

/// A release newer than this build.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    /// The release page, with the notes and downloads.
    pub url: String,
    /// The download this install can update itself from.
    pub asset: Option<Asset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub url: String,
    /// "sha256:<hex>", as GitHub lists it.
    pub digest: Option<String>,
}

/// What this copy of openDJ is, as far as updating goes.
#[derive(Debug, Clone, PartialEq)]
enum Install {
    /// The openDJ.app bundle at this path.
    MacApp(PathBuf),
    /// The AppImage file at this path.
    AppImage(PathBuf),
}

fn this_install() -> Option<Install> {
    if cfg!(target_os = "macos") {
        // .../openDJ.app/Contents/MacOS/opendj
        let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
        let app = exe.parent()?.parent()?.parent()?;
        (app.extension()? == "app").then(|| Install::MacApp(app.to_path_buf()))
    } else {
        std::env::var_os("APPIMAGE").map(|p| Install::AppImage(p.into()))
    }
}

/// The asset name this install updates from.
fn wants(install: &Install, name: &str) -> bool {
    match install {
        Install::MacApp(_) => name.ends_with("-macos.zip"),
        Install::AppImage(_) => name.ends_with(&format!("-{}.AppImage", std::env::consts::ARCH)),
    }
}

/// Starts the check in the background, unless it is turned off in
/// Settings. The answer comes once, or never if there's nothing new.
pub fn check() -> Receiver<Release> {
    let (tx, rx) = channel();
    if settings::flag("update_check", true) {
        std::thread::spawn(move || {
            if let Some(r) = latest().filter(|r| !dismissed(&r.version)) {
                let _ = tx.send(r);
            }
        });
    }
    rx
}

/// Hides the banner for this version; the next one shows again.
pub fn dismiss(version: &str) {
    settings::set("update_dismissed", serde_json::json!(version));
}

fn dismissed(version: &str) -> bool {
    settings::string("update_dismissed") == version
}

fn latest() -> Option<Release> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent(format!("openDJ/{VERSION}"))
        .build()
        .into();
    let body: Value = agent
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| eprintln!("update check: {e}"))
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    newer(&body, VERSION, this_install().as_ref())
}

/// The release in GitHub's answer, if it is newer than `current`.
fn newer(release: &Value, current: &str, install: Option<&Install>) -> Option<Release> {
    let tag = release["tag_name"].as_str()?;
    let version = tag.trim_start_matches('v');
    if release["draft"].as_bool() == Some(true)
        || release["prerelease"].as_bool() == Some(true)
        || !is_newer(version, current)
    {
        return None;
    }
    let asset = install.and_then(|install| {
        release["assets"].as_array()?.iter().find_map(|a| {
            if !wants(install, a["name"].as_str()?) {
                return None;
            }
            Some(Asset {
                url: a["browser_download_url"].as_str()?.to_string(),
                digest: a["digest"].as_str().map(str::to_string),
            })
        })
    });
    Some(Release {
        version: version.to_string(),
        url: release["html_url"].as_str()?.to_string(),
        asset,
    })
}

/// News from an install running in the background.
pub enum Progress {
    /// What the banner says meanwhile.
    Status(String),
    /// The new version is in place: quit, and it starts.
    Done,
    /// Nothing changed; the reason for the alert.
    Failed(String),
}

/// Downloads, checks and puts the new version in place of this one.
pub fn install(release: Release) -> Receiver<Progress> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = (|| {
            let install = this_install().ok_or("this copy of openDJ can't update itself")?;
            let asset = release
                .asset
                .as_ref()
                .ok_or("no download for this system")?;
            run(&install, asset, &tx)
        })();
        let _ = tx.send(match result {
            Ok(()) => Progress::Done,
            Err(e) => {
                eprintln!("update to {}: {e}", release.version);
                Progress::Failed(format!(
                    "Couldn't install openDJ {}: {e}. The release page has the download.",
                    release.version
                ))
            }
        });
    });
    rx
}

fn run(install: &Install, asset: &Asset, tx: &Sender<Progress>) -> Result<(), String> {
    let dir = crate::settings::config_dir()
        .ok_or("no settings folder")?
        .join("updates");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("download");
    let _ = tx.send(Progress::Status("Downloading…".into()));
    download(&asset.url, &file)?;
    let _ = tx.send(Progress::Status("Checking…".into()));
    check_digest(&file, asset.digest.as_deref())?;
    match install {
        Install::MacApp(app) => {
            let unpacked = dir.join("new");
            command(
                Command::new("ditto")
                    .args(["-x", "-k"])
                    .arg(&file)
                    .arg(&unpacked),
            )?;
            let new_app = unpacked.join("openDJ.app");
            check_signature(&new_app, app)?;
            let _ = tx.send(Progress::Status("Installing…".into()));
            swap_app(app, &new_app)
        }
        Install::AppImage(path) => {
            let _ = tx.send(Progress::Status("Installing…".into()));
            // Next to the old one, so the rename can't cross disks.
            let staged = path.with_extension("AppImage.new");
            std::fs::copy(&file, &staged).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            std::fs::rename(&staged, path).map_err(|e| {
                let _ = std::fs::remove_file(&staged);
                e.to_string()
            })
        }
    }
    .map(|()| {
        let _ = std::fs::remove_dir_all(&dir);
    })
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15 * 60)))
        .user_agent(format!("openDJ/{VERSION}"))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    let mut reader = response.body_mut().with_config().limit(1 << 30).reader();
    let mut out = std::fs::File::create(to).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut out).map_err(|e| e.to_string())?;
    Ok(())
}

/// The file must match the checksum GitHub published; a release without
/// one isn't installed.
fn check_digest(file: &Path, digest: Option<&str>) -> Result<(), String> {
    let want = digest
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or("the release has no checksum")?;
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got.eq_ignore_ascii_case(want) {
        Ok(())
    } else {
        Err("the download is damaged (checksum mismatch)".into())
    }
}

/// The new app must be notarized, and signed by the same team as this
/// one when this one is signed (an ad-hoc build can't vouch for anyone).
fn check_signature(new_app: &Path, running: &Path) -> Result<(), String> {
    command(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(new_app),
    )
    .map_err(|_| "the new app's signature is broken".to_string())?;
    command(
        Command::new("spctl")
            .args(["--assess", "--type", "execute"])
            .arg(new_app),
    )
    .map_err(|_| "the new app isn't notarized".to_string())?;
    let new_team = team(new_app).ok_or("the new app has no developer signature")?;
    match team(running) {
        Some(ours) if ours != new_team => Err("the new app is signed by someone else".into()),
        _ => Ok(()),
    }
}

/// The TeamIdentifier from `codesign -dv`, if there is one.
fn team(app: &Path) -> Option<String> {
    let out = Command::new("codesign").arg("-dv").arg(app).output().ok()?;
    team_in(&String::from_utf8_lossy(&out.stderr))
}

fn team_in(codesign_output: &str) -> Option<String> {
    codesign_output
        .lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "not set")
        .map(str::to_string)
}

/// Moves the running app aside, puts the new one in its place and removes
/// the old one; puts the old one back if the move fails.
fn swap_app(app: &Path, new_app: &Path) -> Result<(), String> {
    let parent = app.parent().ok_or("no folder")?;
    let old = parent.join(".openDJ-previous.app");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(app, &old).map_err(|e| format!("can't replace {} ({e})", app.display()))?;
    // A rename where it can, a copy across disks.
    let placed = std::fs::rename(new_app, app).is_ok()
        || command(Command::new("ditto").arg(new_app).arg(app)).is_ok();
    if !placed {
        let _ = std::fs::remove_dir_all(app);
        let _ = std::fs::rename(&old, app);
        return Err(format!("can't write to {}", parent.display()));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

fn command(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Starts the new version once this process has quit.
pub fn relaunch() {
    let Some(install) = this_install() else {
        return;
    };
    let start = match &install {
        Install::MacApp(app) => format!("open {}", shell_quote(app)),
        Install::AppImage(path) => format!("{} >/dev/null 2>&1 &", shell_quote(path)),
    };
    let script = format!(
        "while kill -0 {} 2>/dev/null; do sleep 0.2; done; {start}",
        std::process::id()
    );
    let _ = Command::new("/bin/sh").arg("-c").arg(script).spawn();
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// Compares "major.minor.patch". A build of a pre-release such as
/// 0.3.0-beta.1 counts as older than 0.3.0 itself.
fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Option<([u64; 3], bool)> {
        let (numbers, pre) = match v.split_once('-') {
            Some((n, _)) => (n, true),
            None => (v, false),
        };
        let mut out = [0; 3];
        for (slot, part) in out.iter_mut().zip(numbers.split('.')) {
            *slot = part.parse().ok()?;
        }
        Some((out, pre))
    }
    match (parts(candidate), parts(current)) {
        (Some((a, a_pre)), Some((b, b_pre))) => a > b || (a == b && b_pre && !a_pre),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.5"));
        assert!(is_newer("1.0.0", "0.99.0"));
        assert!(is_newer("0.3.0", "0.3.0-beta.1"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.3.0"));
        assert!(!is_newer("0.3.0-beta.2", "0.3.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }

    #[test]
    fn reads_githubs_answer() {
        let release = json!({
            "tag_name": "v0.3.0",
            "html_url": "https://github.com/sebastian-beca/opendj/releases/tag/v0.3.0",
            "draft": false,
            "prerelease": false,
        });
        assert_eq!(
            newer(&release, "0.2.0", None),
            Some(Release {
                version: "0.3.0".into(),
                url: "https://github.com/sebastian-beca/opendj/releases/tag/v0.3.0".into(),
                asset: None,
            })
        );
        assert_eq!(newer(&release, "0.3.0", None), None);
        let mut pre = release.clone();
        pre["prerelease"] = json!(true);
        assert_eq!(newer(&pre, "0.2.0", None), None);
    }

    #[test]
    fn picks_the_download_for_this_install() {
        let release = json!({
            "tag_name": "v0.3.0",
            "html_url": "https://example.com/r",
            "assets": [
                {"name": "openDJ-0.3.0-macos.zip", "browser_download_url": "https://example.com/mac", "digest": "sha256:ab"},
                {"name": "openDJ-0.3.0-x86_64.AppImage", "browser_download_url": "https://example.com/appimage"},
                {"name": "opendj-0.3.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.com/tar"},
            ],
        });
        let mac = newer(
            &release,
            "0.2.0",
            Some(&Install::MacApp("/A/openDJ.app".into())),
        );
        assert_eq!(
            mac.unwrap().asset,
            Some(Asset {
                url: "https://example.com/mac".into(),
                digest: Some("sha256:ab".into()),
            })
        );
        let appimage = newer(
            &release,
            "0.2.0",
            Some(&Install::AppImage("/x.AppImage".into())),
        );
        let expect_appimage = std::env::consts::ARCH == "x86_64";
        assert_eq!(appimage.unwrap().asset.is_some(), expect_appimage);
    }

    #[test]
    fn checks_the_digest() {
        let dir = std::env::temp_dir().join(format!("opendj-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("f");
        std::fs::write(&file, b"abc").unwrap();
        let abc = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(check_digest(&file, Some(abc)).is_ok());
        assert!(check_digest(&file, Some("sha256:00")).is_err());
        assert!(check_digest(&file, None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_the_team() {
        let signed = "Executable=/A/openDJ.app/Contents/MacOS/opendj\nAuthority=Developer ID Application: Seba (AB12CD34EF)\nTeamIdentifier=AB12CD34EF\n";
        assert_eq!(team_in(signed).as_deref(), Some("AB12CD34EF"));
        assert_eq!(team_in("Signature=adhoc\nTeamIdentifier=not set\n"), None);
    }

    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(
            shell_quote(Path::new("/a b/it's.app")),
            "'/a b/it'\\''s.app'"
        );
    }
}
