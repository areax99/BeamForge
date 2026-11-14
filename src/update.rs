//! Help > Updates: asks GitHub which RayEtch releases exist and offers the newest one.
//!
//! Releases live at github.com/areax99/RayEtch/releases. Each one is tagged `v<version>`
//! (for example `v1.0.0-beta.2`), marked "pre-release" while it is a beta, and carries the
//! archives made by build-release.local.py. RayEtch compares the tag with its own version
//! (Cargo.toml) and, when a newer one exists, shows its notes and offers "Download and install":
//! the archive for this computer is downloaded, unpacked, swapped with the running program and
//! started when RayEtch restarts. The check runs `curl` (present on macOS, Linux and
//! Windows 10+) in a background thread, so the window never waits for the network.
//!
//! The original checks once a day, 2 s after start (MainWindow::runUpdateCheck 0x10068122c,
//! day of year compared with the stored one, then "start_auto_update_check" after 2 000 000 000 ns);
//! RayEtch does the same, quietly unless something newer is found.

use eframe::egui;
use std::sync::{Arc, Mutex};

pub const RELEASES_PAGE: &str = "https://github.com/areax99/RayEtch/releases";
const API: &str = "https://api.github.com/repos/areax99/RayEtch/releases?per_page=20";
/// The automatic check waits this long after start (runUpdateCheck: 2 s).
pub const AUTO_CHECK_DELAY_S: f64 = 2.0;
/// Include betas (pre-releases) when looking for something newer: for now every release is a beta.
pub const INCLUDE_BETAS: bool = true;

/// This program's version, as written in Cargo.toml (e.g. "1.0.0-beta.1").
pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub name: String,
    pub notes: String,
    pub page: String,
    pub beta: bool,
    /// The archive meant for this computer, if the release has one: (file name, download address).
    pub asset: Option<(String, String)>,
    /// Its size in bytes, for the progress bar (0 when GitHub didn't say).
    pub asset_size: u64,
}

#[derive(Clone, Debug)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Newer(Release),
    NoReleases,
    Failed(String),
    /// Downloading the archive: bytes so far (read from the file as it grows) and the total.
    Downloading { release: Release, file: std::path::PathBuf, total: u64 },
    /// Unpacking and putting the new program in place.
    Installing(Release),
    /// Done: the new program is in place and starts when RayEtch restarts.
    Installed { version: String, relaunch: std::path::PathBuf },
    /// Downloading or installing went wrong; the old program is untouched (or put back).
    InstallFailed { release: Release, error: String },
}

#[derive(Clone)]
pub struct Checker {
    pub state: Arc<Mutex<State>>,
    /// Started by the daily automatic check: only a newer version opens the window.
    pub quiet: bool,
}

impl Default for Checker {
    fn default() -> Self {
        Self { state: Arc::new(Mutex::new(State::Idle)), quiet: false }
    }
}

impl Checker {
    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or(State::Idle)
    }

    pub fn start(&mut self, ctx: &egui::Context, quiet: bool) {
        if matches!(self.state(), State::Checking) {
            return;
        }
        self.quiet = quiet;
        *self.state.lock().unwrap() = State::Checking;
        let slot = self.state.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = fetch().map(|list| pick_newer(&list, current()));
            *slot.lock().unwrap() = match result {
                Ok(Some(r)) => State::Newer(r),
                Ok(None) => State::UpToDate,
                Err(e) if e == "empty" => State::NoReleases,
                Err(e) => State::Failed(e),
            };
            ctx.request_repaint();
        });
    }
}

impl Checker {
    /// Download the release's archive and put it in place of the running program (install_archive).
    pub fn install(&mut self, ctx: &egui::Context, release: Release) {
        let Some((name, url)) = release.asset.clone() else { return };
        let dir = std::env::temp_dir().join(format!("rayetch-update-{}", release.version));
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            *self.state.lock().unwrap() = State::InstallFailed { release, error: e.to_string() };
            return;
        }
        let file = dir.join(&name);
        *self.state.lock().unwrap() = State::Downloading { release: release.clone(), file: file.clone(), total: release.asset_size };
        let slot = self.state.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = download(&url, &file).and_then(|_| {
                *slot.lock().unwrap() = State::Installing(release.clone());
                ctx.request_repaint();
                install_archive(&file, &dir)
            });
            *slot.lock().unwrap() = match result {
                Ok(relaunch) => State::Installed { version: release.version.clone(), relaunch },
                Err(error) => State::InstallFailed { release, error },
            };
            ctx.request_repaint();
        });
    }
}

/// Every copy can install an update now: a protected folder asks for the administrator password,
/// and a development build installs the package where RayEtch normally lives (standard_install).
pub fn can_self_install() -> bool {
    install_target().is_ok()
}

/// For the Updates window: where the update will go, whether the password will be asked, and
/// whether that is a different place from the running copy (a development build).
pub fn install_info() -> Option<(std::path::PathBuf, bool, bool)> {
    let t = install_target().ok()?;
    let exe = std::env::current_exe().ok()?;
    let elsewhere = !exe.starts_with(&t.path);
    Some((t.path, t.admin, elsewhere))
}

/// Where the update goes, and whether writing there needs the administrator.
pub struct Target {
    pub path: std::path::PathBuf,
    pub admin: bool,
}

/// What gets replaced: the .app bundle on macOS, the program file on Windows and Linux.
/// A development build (run from cargo's target folder) isn't replaced: the update is installed
/// in the standard place instead, and that copy starts on restart.
fn install_target() -> Result<Target, String> {
    // RAYETCH_UPDATE_TARGET (for testing): replace this copy instead of the running one
    let path = if let Some(t) = std::env::var_os("RAYETCH_UPDATE_TARGET") {
        std::path::PathBuf::from(t)
    } else {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.canonicalize().unwrap_or(exe);
        let bundle = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(|p| p.to_path_buf());
        if exe.components().any(|c| c.as_os_str() == "target") || (cfg!(target_os = "macos") && bundle.is_none()) {
            standard_install()?
        } else if cfg!(target_os = "macos") {
            bundle.unwrap()
        } else {
            exe
        }
    };
    Ok(Target { admin: !writable(path.parent().ok_or("no parent folder")?), path })
}

/// The usual home of an installed RayEtch: /Applications on a Mac (~/Applications when the
/// person can't write there and has no admin rights is not tried: the password prompt covers it),
/// %LOCALAPPDATA%\Programs\RayEtch on Windows, ~/.local/bin on Linux.
fn standard_install() -> Result<std::path::PathBuf, String> {
    if cfg!(target_os = "macos") {
        Ok("/Applications/RayEtch.app".into())
    } else if cfg!(windows) {
        let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA not set")?;
        Ok(std::path::PathBuf::from(base).join("Programs").join("RayEtch").join("rayetch.exe"))
    } else {
        let home = std::env::var_os("HOME").ok_or("HOME not set")?;
        Ok(std::path::PathBuf::from(home).join(".local").join("bin").join("rayetch"))
    }
}

/// Whether this folder (or, when missing, the nearest folder above it) takes new files.
fn writable(dir: &std::path::Path) -> bool {
    let Some(existing) = dir.ancestors().find(|p| p.exists()) else { return false };
    let probe = existing.join(format!(".rayetch-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

fn download(url: &str, file: &std::path::Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sSfL", "--max-time", DOWNLOAD_TIMEOUT_S, "-o"]).arg(file).arg(url);
    no_console(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Unpack the archive next to it, find the new program, swap it with the running one (the old one
/// is kept until the swap succeeds and put back if it fails), and return what to start on restart.
/// A running program can be renamed on every system, so no helper process is needed.
fn install_archive(file: &std::path::Path, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    install_archive_to(file, dir, install_target()?)
}

#[cfg(test)]
fn install_target_at(path: &std::path::Path) -> Target {
    Target { path: path.to_path_buf(), admin: !writable(path.parent().unwrap()) }
}

fn install_archive_to(file: &std::path::Path, dir: &std::path::Path, target: Target) -> Result<std::path::PathBuf, String> {
    let unpacked = dir.join("unpacked");
    std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("ditto");
        c.arg("-x").arg("-k").arg(file).arg(&unpacked);
        c
    } else {
        // bsdtar (Windows 10+) and GNU tar both read .zip and .tar.gz
        let mut c = std::process::Command::new("tar");
        c.arg("-xf").arg(file).arg("-C").arg(&unpacked);
        c
    };
    no_console(&mut cmd);
    let out = cmd.output().map_err(|e| format!("unpack: {e}"))?;
    if !out.status.success() {
        return Err(format!("unpack: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let want = if cfg!(target_os = "macos") { "RayEtch.app" } else if cfg!(windows) { "rayetch.exe" } else { "rayetch" };
    // an archive made before the rename still holds the program under its old name
    let old_name = if cfg!(target_os = "macos") { "BeamForge.app" } else if cfg!(windows) { "beamforge.exe" } else { "beamforge" };
    let new = find(&unpacked, want, 3).or_else(|| find(&unpacked, old_name, 3)).ok_or_else(|| format!("{want} not found in the archive"))?;
    let Target { path: target, admin } = target;
    if admin {
        swap_as_admin(&new, &target)?;
        let _ = std::fs::remove_dir_all(dir);
        return Ok(target);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let old = target.with_file_name(format!(".{}.old", target.file_name().unwrap().to_string_lossy()));
    if old.is_dir() {
        let _ = std::fs::remove_dir_all(&old);
    } else {
        let _ = std::fs::remove_file(&old);
    }
    // a first install (development build) has nothing to move aside
    if target.exists() {
        std::fs::rename(&target, &old).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    let placed = std::fs::rename(&new, &target).or_else(|_| copy_all(&new, &target));
    if let Err(e) = placed {
        let _ = std::fs::rename(&old, &target);
        return Err(format!("{}: {e}", target.display()));
    }
    #[cfg(unix)]
    if !cfg!(target_os = "macos") {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
    }
    if cfg!(target_os = "macos") {
        // downloaded with curl, so no quarantine flag is expected; clear it in case
        let _ = std::process::Command::new("xattr").arg("-dr").arg("com.apple.quarantine").arg(&target).output();
    }
    // the old copy goes; on Windows the running .exe can't be deleted, it is cleared next start
    if old.is_dir() {
        let _ = std::fs::remove_dir_all(&old);
    } else {
        let _ = std::fs::remove_file(&old);
    }
    let _ = std::fs::remove_dir_all(dir);
    Ok(target)
}

/// Put `new` in place of `target` in a folder only the administrator may change: the system asks
/// for the password once (macOS: the usual password window through osascript; Linux: pkexec;
/// Windows: the UAC prompt). The old copy is moved aside first and put back if the move fails.
fn swap_as_admin(new: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    let old = target.with_file_name(format!(".{}.old", target.file_name().unwrap().to_string_lossy()));
    let (n, t, o) = (new.display().to_string(), target.display().to_string(), old.display().to_string());
    let parent = target.parent().map(|p| p.display().to_string()).unwrap_or_default();
    let mut cmd;
    if cfg!(windows) {
        // cmd.exe inside an elevated PowerShell Start-Process; a running .exe may be renamed
        let script = format!(
            "if not exist \"{parent}\" mkdir \"{parent}\" & del /f /q \"{o}\" 2>nul & (if exist \"{t}\" move /y \"{t}\" \"{o}\") & copy /y \"{n}\" \"{t}\" || move /y \"{o}\" \"{t}\""
        );
        let ps = format!("Start-Process cmd -ArgumentList '/c {}' -Verb RunAs -Wait -WindowStyle Hidden", script.replace('\'', "''"));
        cmd = std::process::Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", &ps]);
    } else {
        let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let copy = if cfg!(target_os = "macos") { "ditto" } else { "cp -R" };
        let script = format!(
            "mkdir -p {p} && rm -rf {o} && {{ [ ! -e {t} ] || mv {t} {o}; }} && {{ {copy} {n} {t} && rm -rf {o} || {{ rm -rf {t}; mv {o} {t}; exit 1; }}; }}{chmod}",
            p = q(&parent),
            o = q(&o),
            t = q(&t),
            n = q(&n),
            chmod = if cfg!(target_os = "macos") { String::new() } else { format!(" && chmod 755 {}", q(&t)) },
        );
        if cfg!(target_os = "macos") {
            let apple = format!("do shell script \"{}\" with administrator privileges", script.replace('\\', "\\\\").replace('"', "\\\""));
            cmd = std::process::Command::new("osascript");
            cmd.arg("-e").arg(apple);
        } else {
            cmd = std::process::Command::new("pkexec");
            cmd.args(["sh", "-c", &script]);
        }
    }
    no_console(&mut cmd);
    let out = cmd.output().map_err(|e| format!("administrator: {e}"))?;
    // UAC's own exit code isn't the copy's: check the result on disk
    if out.status.success() && target.exists() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if msg.is_empty() { "the administrator password was not given".into() } else { msg })
    }
}

/// Remove a program left behind by an update on Windows (it couldn't be deleted while running).
pub fn clean_leftovers() {
    if let Ok(Target { path, admin: false }) = install_target() {
        let old = path.with_file_name(format!(".{}.old", path.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_file(old);
    }
}

fn find(dir: &std::path::Path, name: &str, depth: u32) -> Option<std::path::PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
        if depth > 0 && p.is_dir() && p.extension().is_none_or(|x| x != "app") {
            if let Some(f) = find(&p, name, depth - 1) {
                return Some(f);
            }
        }
    }
    None
}

fn copy_all(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_all(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Start the newly installed program (called as the old one exits).
pub fn relaunch(target: &std::path::Path) {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-n").arg(target);
        c
    } else {
        std::process::Command::new(target)
    };
    let _ = cmd.spawn();
}

fn no_console(_cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        _cmd.creation_flags(0x0800_0000);
    }
}

/// Longest wait for the archive download, in seconds (estimated).
pub(crate) const DOWNLOAD_TIMEOUT_S: &str = "600";

fn fetch() -> Result<Vec<Release>, String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sSL", "--max-time", "15", "-H", "Accept: application/vnd.github+json", "-H"])
        .arg(format!("User-Agent: RayEtch/{}", current()))
        .arg(API);
    no_console(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let Some(items) = v.as_array() else {
        // GitHub answers {"message": "Not Found"} for a missing or private repository
        let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unexpected answer");
        return Err(msg.to_string());
    };
    let list: Vec<Release> = items
        .iter()
        .filter(|r| !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false))
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?;
            let sizes: Vec<(String, u64)> = r
                .get("assets")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|x| Some((x.get("name")?.as_str()?.to_string(), x.get("size")?.as_u64()?))).collect())
                .unwrap_or_default();
            let assets: Vec<(String, String)> = r
                .get("assets")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| Some((x.get("name")?.as_str()?.to_string(), x.get("browser_download_url")?.as_str()?.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            Some(Release {
                version: tag.trim_start_matches(['v', 'V']).to_string(),
                name: r.get("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty()).unwrap_or(tag).to_string(),
                notes: r.get("body").and_then(|b| b.as_str()).unwrap_or("").to_string(),
                page: r.get("html_url").and_then(|u| u.as_str()).unwrap_or(RELEASES_PAGE).to_string(),
                beta: r.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false),
                asset: asset_for_this_computer(&assets),
                asset_size: 0,
            })
            .map(|mut rel| {
                if let Some((n, _)) = &rel.asset {
                    rel.asset_size = sizes.iter().find(|(m, _)| m == n).map(|(_, z)| *z).unwrap_or(0);
                }
                rel
            })
        })
        .collect();
    if list.is_empty() { Err("empty".into()) } else { Ok(list) }
}

/// The archive named for this system and processor (names from build-release.local.py:
/// RayEtch-<version>-<os>-<arch>…); on a Mac the Universal one first.
fn asset_for_this_computer(assets: &[(String, String)]) -> Option<(String, String)> {
    let os = if cfg!(target_os = "macos") { "-macos-" } else if cfg!(windows) { "-windows-" } else { "-linux-" };
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
    let mine: Vec<&(String, String)> = assets.iter().filter(|(n, _)| n.contains(os)).collect();
    let pick = |want: &str| mine.iter().find(|(n, _)| n.contains(&format!("{os}{want}"))).map(|a| (*a).clone());
    if cfg!(target_os = "macos") {
        pick("universal").or_else(|| pick(arch))
    } else {
        pick(arch).or_else(|| (arch == "arm64").then(|| pick("aarch64")).flatten())
    }
}

/// The newest release above `mine`, betas included while INCLUDE_BETAS is on.
fn pick_newer(list: &[Release], mine: &str) -> Option<Release> {
    let mine = Version::parse(mine)?.key();
    list.iter()
        .filter(|r| INCLUDE_BETAS || !r.beta)
        .filter_map(|r| Some((Version::parse(&r.version)?.key(), r)))
        .filter(|(v, _)| *v > mine)
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, r)| r.clone())
}

/// major.minor.patch with an optional "-beta.N" (a pre-release sorts below the plain version).
struct Version {
    core: (u64, u64, u64),
    pre: Option<(String, u64)>,
}

impl Version {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches(['v', 'V']);
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut n = core.split('.').map(|x| x.parse::<u64>().ok());
        let core = (n.next()??, n.next().flatten().unwrap_or(0), n.next().flatten().unwrap_or(0));
        let pre = pre.map(|p| match p.rsplit_once('.') {
            Some((l, n)) if n.parse::<u64>().is_ok() => (l.to_lowercase(), n.parse().unwrap()),
            _ => (p.to_lowercase(), 0),
        });
        Some(Self { core, pre })
    }

    /// Sort key: the number first, then final above any pre-release, then the pre-release label and number.
    fn key(&self) -> ((u64, u64, u64), bool, Option<(String, u64)>) {
        (self.core, self.pre.is_none(), self.pre.clone())
    }
}

/// Today's day number, to check at most once a day like the original.
pub fn today() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(v: &str, beta: bool) -> Release {
        Release { version: v.into(), name: v.into(), notes: String::new(), page: String::new(), beta, asset: None, asset_size: 0 }
    }

    #[test]
    fn versions_compare() {
        let v = |s| Version::parse(s).unwrap().key();
        assert!(v("1.0.0-beta.2") > v("1.0.0-beta.1"));
        assert!(v("1.0.0-beta.10") > v("1.0.0-beta.9"));
        assert!(v("1.0.0") > v("1.0.0-beta.9"));
        assert!(v("1.0.1-beta.1") > v("1.0.0"));
        assert!(v("v1.1.0") > v("1.0.9"));
    }

    #[test]
    fn picks_newest() {
        let list = vec![rel("1.0.0-beta.1", true), rel("1.0.0-beta.3", true), rel("1.0.0-beta.2", true)];
        assert_eq!(pick_newer(&list, "1.0.0-beta.1").unwrap().version, "1.0.0-beta.3");
        assert!(pick_newer(&list, "1.0.0-beta.3").is_none());
    }

    #[test]
    fn picks_archive() {
        let a = vec![
            ("RayEtch-1.0.0-beta.2-macos-universal.zip".to_string(), "u".to_string()),
            ("RayEtch-1.0.0-beta.2-windows-x86_64.zip".to_string(), "w".to_string()),
            ("RayEtch-1.0.0-beta.2-linux-x86_64-gnu.tar.gz".to_string(), "l".to_string()),
        ];
        assert!(asset_for_this_computer(&a).is_some() || cfg!(target_arch = "aarch64"));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod install_tests {
    /// A fake installed bundle is swapped with the one inside a release-style zip.
    #[test]
    fn swaps_bundle() {
        let base = std::env::temp_dir().join(format!("bf-install-test-{}", std::process::id()));
        let installed = base.join("Applications/RayEtch.app");
        std::fs::create_dir_all(installed.join("Contents")).unwrap();
        std::fs::write(installed.join("Contents/version"), "old").unwrap();
        let new_src = base.join("build/RayEtch.app");
        std::fs::create_dir_all(new_src.join("Contents")).unwrap();
        std::fs::write(new_src.join("Contents/version"), "new").unwrap();
        let work = base.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let zip = work.join("RayEtch-9.9.9-beta.1-macos-universal.zip");
        assert!(std::process::Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&new_src).arg(&zip).status().unwrap().success());
        // SAFETY: tests in this module run alone; the variable only points the installer at the fake copy
        unsafe { std::env::set_var("RAYETCH_UPDATE_TARGET", &installed) };
        let r = super::install_archive(&zip, &work);
        unsafe { std::env::remove_var("RAYETCH_UPDATE_TARGET") };
        assert_eq!(r.unwrap(), installed);
        assert_eq!(std::fs::read_to_string(installed.join("Contents/version")).unwrap(), "new");
        assert!(!base.join("Applications/.RayEtch.app.old").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod first_install_tests {
    /// With nothing installed yet (a development build's first update) the bundle is simply placed.
    #[test]
    fn places_bundle_where_none_was() {
        let base = std::env::temp_dir().join(format!("bf-first-install-{}", std::process::id()));
        let new_src = base.join("build/RayEtch.app");
        std::fs::create_dir_all(new_src.join("Contents")).unwrap();
        std::fs::write(new_src.join("Contents/version"), "new").unwrap();
        let work = base.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let zip = work.join("RayEtch-9.9.9-beta.1-macos-universal.zip");
        assert!(std::process::Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&new_src).arg(&zip).status().unwrap().success());
        let target = base.join("Apps/RayEtch.app");
        let t = super::install_target_at(&target);
        assert!(!t.admin);
        let r = super::install_archive_to(&zip, &work, t);
        assert_eq!(r.unwrap(), target);
        assert_eq!(std::fs::read_to_string(target.join("Contents/version")).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&base);
    }
}
