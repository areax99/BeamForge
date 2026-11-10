//! Gives saved projects (.beamforge files) their own icon in Windows Explorer.
//! macOS gets it from the app bundle (scripts/macos-app.sh) and Linux from its file type
//! (scripts/install-linux.sh); on Windows the program registers it for the current user
//! when it starts, pointing at the icon built into beamforge.exe (resource 2, see build.rs).

#[cfg(windows)]
pub fn register() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.display().to_string();
    let class = r"HKCU\Software\Classes\BeamForge.Project";
    let icon = format!("\"{exe}\",-2");
    let open = format!("\"{exe}\" \"%1\"");
    let entries: [(String, &str); 4] = [
        (r"HKCU\Software\Classes\.beamforge".to_string(), "BeamForge.Project"),
        (class.to_string(), "BeamForge project"),
        (format!(r"{class}\DefaultIcon"), icon.as_str()),
        (format!(r"{class}\shell\open\command"), open.as_str()),
    ];
    for (key, value) in entries {
        let _ = std::process::Command::new("reg")
            .args(["add", &key, "/ve", "/d", value, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

#[cfg(not(windows))]
pub fn register() {}

/// Whether a file is a BeamForge project (opened) rather than a drawing (imported).
/// Like the original's OpenProject (0x100c4c9e8), the extension is compared ignoring case.
pub fn is_project(p: &std::path::Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("beamforge"))
}

/// Save without risking the old file (MainWindow::SaveProject 0x10098548c): the project is
/// written to a temporary file "<name>_XXXXXX.<ext>" in the same folder, and only when that
/// worked does it replace the old one (the original copies it over; BeamForge renames, which
/// swaps it in one step). A failed write leaves the old project untouched.
pub fn safe_write(p: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("project");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("beamforge");
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let tmp = dir.join(format!("{stem}_{:06}.{ext}", (nanos ^ std::process::id()) % 1_000_000));
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// The document icon (a sheet with the orange diamond, drawn by examples/make_doc_icon.rs).
#[cfg(target_os = "macos")]
const DOCUMENT_PNG: &[u8] = include_bytes!("../assets/document.png");

/// Give a saved project its document icon in Finder. The app bundle declares the icon for
/// .beamforge files, but when BeamForge runs without being installed as BeamForge.app Finder
/// doesn't know that type and shows a plain text page; so the icon is also stored on the file
/// itself (NSWorkspace setIcon:forFile:options:). Windows and Linux use their file-type
/// registration instead.
#[cfg(target_os = "macos")]
pub fn mark_saved(p: &std::path::Path) {
    use objc2::AnyThread;
    use objc2_app_kit::{NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::{NSData, NSString};
    let Some(path) = p.to_str() else { return };
    let data = NSData::with_bytes(DOCUMENT_PNG);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else { return };
    let ws = NSWorkspace::sharedWorkspace();
    let _ = ws.setIcon_forFile_options(Some(&image), &NSString::from_str(path), NSWorkspaceIconCreationOptions::empty());
}

#[cfg(not(target_os = "macos"))]
pub fn mark_saved(_p: &std::path::Path) {}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn saved_project_gets_icon() {
        let p = std::env::temp_dir().join(format!("bf_icon_test_{}.beamforge", std::process::id()));
        super::safe_write(&p, b"{}").unwrap();
        // saving again keeps the content and leaves no temporary file behind
        super::safe_write(&p, b"{\"a\":1}").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{\"a\":1}");
        let stem = p.file_stem().unwrap().to_str().unwrap().to_string();
        let left = std::fs::read_dir(std::env::temp_dir()).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{stem}_"))).count();
        assert_eq!(left, 0);
        super::mark_saved(&p);
        // Finder keeps a custom icon in the file's resource fork
        let fork = std::fs::metadata(p.join("..namedfork/rsrc")).map(|m| m.len()).unwrap_or(0);
        let _ = std::fs::remove_file(&p);
        assert!(fork > 0);
    }
}
