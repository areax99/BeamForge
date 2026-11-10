// On Windows, start as a window app (no extra black console window).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod canvas;
mod combine;
mod devices;
mod fileicon;
mod formats;
mod gcode;
mod i18n;
mod icons;
mod import;
mod measure;
mod laser;
mod lasertest;
mod lasertest_ui;
mod model;
mod preview;
mod offset;
mod panels;
mod sysmenu;
mod text;
mod theme;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot = None;
    let mut open_file = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(std::path::PathBuf::from(&args[i + 1]));
            i += 2;
            continue;
        }
        open_file = Some(std::path::PathBuf::from(&args[i]));
        i += 1;
    }
    if screenshot.is_none() {
        std::thread::spawn(fileicon::register);
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("BeamForge")
            .with_inner_size([1440.0, 860.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "BeamForge",
        options,
        Box::new(move |cc| {
            let mut a = app::BeamForge::new(cc, screenshot);
            if let Some(p) = open_file {
                if fileicon::is_project(&p) {
                    a.open_path_now(p);
                } else {
                    a.import_path(&p);
                }
            }
            // BEAMFORGE_OPEN=deselect (for testing): show the imported drawing unselected
            if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("deselect")) {
                a.selection.clear();
            }
            // BEAMFORGE_OPEN=offset (for testing): open the Offset dialog on the imported drawing
            if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("offset")) {
                a.open_offset();
            }
            // BEAMFORGE_OPEN=interval / material / testpreview (for testing): the laser test windows
            if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("interval")) {
                a.open_interval_test();
            }
            if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("material")) {
                a.open_material_test();
            }
            if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("testpreview")) {
                let d = lasertest::MaterialTest::new(&a.doc.device).build(&a.doc);
                a.preview_test(d);
            }
            // BEAMFORGE_OPEN=measure (for testing): ruler on the first shape's second piece
            if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("measure")) {
                a.tool = app::Tool::Measure;
                a.measure.open = true;
                a.measure.shape = a.doc.shapes.first().map(|s| s.id);
                a.measure.prim = Some(1);
            }
            Ok(Box::new(a))
        }),
    )
}

/// The window and Dock/taskbar icon, built into the program (assets/icon.png, made by
/// `cargo run --example make_icon`).
fn app_icon() -> eframe::egui::IconData {
    let png = include_bytes!("../assets/icon.png");
    match image::load_from_memory(png) {
        Ok(img) => {
            let img = img.resize_exact(256, 256, image::imageops::FilterType::Triangle).to_rgba8();
            let (width, height) = img.dimensions();
            eframe::egui::IconData { rgba: img.into_raw(), width, height }
        }
        Err(_) => eframe::egui::IconData::default(),
    }
}
