//! An import running in the background, so a slow file never freezes the window.
//!
//! The person's case: .ai files kept in iCloud Drive with "Optimise Mac Storage" are only
//! placeholders on the Mac (flag SF_DATALESS). Reading one makes macOS download it first, and the
//! read waits until then: minutes on a slow line, for ever when offline. Done on the window's own
//! thread, that looked like BeamForge had stopped responding.

use crate::formats::Imported;
use crate::i18n::{tr, trf};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct ImportJob {
    pub path: PathBuf,
    /// The file was only in iCloud when the import began.
    pub icloud: bool,
    started: std::time::Instant,
    result: Arc<Mutex<Option<Result<Imported, String>>>>,
}

impl ImportJob {
    pub fn start(path: PathBuf, icloud: bool) -> Self {
        let result = Arc::new(Mutex::new(None));
        let slot = result.clone();
        let p = path.clone();
        std::thread::spawn(move || {
            let r = crate::formats::import_file(&p);
            *slot.lock().unwrap() = Some(r);
        });
        Self { path, icloud, started: std::time::Instant::now(), result }
    }

    pub fn take_result(&self) -> Option<Result<Imported, String>> {
        self.result.lock().ok()?.take()
    }
}

/// Whether macOS keeps this file only in iCloud for now (its data isn't on the disk).
pub fn in_cloud_only(p: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::darwin::fs::MetadataExt;
        // SF_DATALESS from <sys/stat.h>: the file's contents live with a file provider (iCloud)
        const SF_DATALESS: u32 = 0x4000_0000;
        // symlink_metadata reads the flags without touching the data, so it never waits
        return std::fs::symlink_metadata(p).is_ok_and(|m| m.st_flags() & SF_DATALESS != 0);
    }
    #[allow(unreachable_code)]
    {
        let _ = p;
        false
    }
}

/// Ask iCloud to bring the file down now (the read would ask too; this starts it at once).
pub fn request_download(p: &Path) {
    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("brctl").arg("download").arg(p).spawn();
    }
}

/// The small "Importing…" window. Returns true when the person cancels.
pub fn window(ctx: &egui::Context, job: &ImportJob) -> bool {
    let mut cancel = false;
    let name = job.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    crate::theme::centered(ctx, "import_job", egui::Window::new(tr("Importazione")).resizable(false).collapsible(false)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(trf("Importazione di {}…", &[&name]));
        });
        let secs = job.started.elapsed().as_secs();
        if job.icloud {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tr("Il file è su iCloud e non ancora su questo Mac: si attende che venga scaricato. Se non arriva, controlla la connessione o aprilo una volta nel Finder.")).small().color(crate::theme::p().text_dim));
        }
        if secs >= 2 {
            ui.label(egui::RichText::new(trf("{} s", &[&secs])).small().color(crate::theme::p().text_dim));
        }
        ui.add_space(6.0);
        if ui.button(crate::i18n::cancel()).clicked() {
            cancel = true;
        }
    });
    cancel
}
