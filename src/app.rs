//! BeamForge application state and document actions.

use crate::i18n::{tr, trf};
use crate::gcode::{self, Job, JobOptions, StartFrom};
use crate::laser::{Cmd, Ev, Laser};
use crate::model::*;
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Line,
    Rect,
    Ellipse,
    Polygon,
    Triangle,
    Pentagon,
    Octagon,
    Star,
    DualStar,
    Text,
    Measure,
    PositionLaser,
    Tabs,
}

/// The shapes in the tool bar's shapes menu, in the original's order
/// (MainWindow::on_actionTriangle/Pentagon/Octagon/Star/DualStar_triggered 0x100c532a0..0x100c533a4).
pub const SHAPE_TOOLS: [Tool; 8] = [Tool::Rect, Tool::Ellipse, Tool::Triangle, Tool::Pentagon, Tool::Polygon, Tool::Octagon, Tool::Star, Tool::DualStar];

impl Tool {
    /// Fixed number of sides for the triangle, pentagon and octagon tools.
    pub fn fixed_sides(self) -> Option<u32> {
        match self {
            Tool::Triangle => Some(3),
            Tool::Pentagon => Some(5),
            Tool::Octagon => Some(8),
            _ => None,
        }
    }
    /// A regular polygon drawn in a dragged box (PolygonTool 0x100757654).
    pub fn is_ngon(self) -> bool {
        self == Tool::Polygon || self.fixed_sides().is_some()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Laser,
    Move,
    Console,
    Shape,
}

pub enum Drag {
    None,
    Move { start: P2, orig: Vec<Shape> },
    Scale { handle: usize, orig: Vec<Shape>, bbox: BBox },
    /// Shift + a corner handle: turn the selection about its centre (SelectionTool::Update 0x10077784c).
    /// `start` is the pointer's angle from `pivot` when the drag began, `deg` the turn shown so far.
    Rotate { orig: Vec<Shape>, pivot: P2, start: f64, deg: f64 },
    Rubber { start: P2 },
    Create { start: P2 },
}

pub struct View {
    /// pixels per mm
    pub zoom: f32,
    /// screen offset of world (0,0) from the canvas top-left, Y grows down on screen
    pub offset: egui::Vec2,
    pub fit_pending: bool,
    pub auto_fit: bool,
    pub last_area: egui::Vec2,
}

pub struct ScreenshotState {
    pub path: PathBuf,
    pub frames: u32,
    pub requested: bool,
    pub image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

pub use crate::preview::Preview;

/// Wait before trying the next port after one gave no answer (estimated).
pub const AUTOCONNECT_NEXT: Duration = Duration::from_millis(300);
/// Wait after a full round of ports found no laser (estimated).
pub const AUTOCONNECT_RETRY: Duration = Duration::from_secs(3);
/// How long an opened port has to answer like a GRBL laser (estimated).
pub const AUTOCONNECT_VERIFY: Duration = Duration::from_secs(4);

pub struct AutoConnect {
    pub queue: Vec<String>,
    pub next_try: Instant,
    pub pending: bool,
    pub verified: bool,
    pub quiet: bool,
    pub verify_deadline: Option<Instant>,
}

impl Default for AutoConnect {
    fn default() -> Self {
        Self { queue: vec![], next_try: Instant::now(), pending: false, verified: false, quiet: false, verify_deadline: None }
    }
}

/// Ports to try in order: the chosen one first, else every likely serial port
/// (on macOS the /dev/cu.* call-out devices, never Bluetooth or debug consoles).
/// Ports never tried, as in Connection_Serial::AvailablePorts (called by ConnectionPossible 0x1005a9164): macOS system ports.
pub const EXCLUDED_PORTS: [&str; 3] = ["cu.wlan-debug", "cu.debug-console", "cu.Bluetooth-Incoming-Port"];

/// Extra ports listed one per line in PortExclude.txt in the settings folder.
fn excluded_by_file(name: &str) -> bool {
    let Some(dir) = crate::i18n::settings_folder() else { return false };
    std::fs::read_to_string(dir.join("PortExclude.txt"))
        .map(|t| t.lines().any(|l| l.trim() == name))
        .unwrap_or(false)
}

pub fn auto_port_order(chosen: &str, ports: &[String]) -> Vec<String> {
    if !chosen.is_empty() {
        return vec![chosen.to_string()];
    }
    let skip = |p: &str| {
        let l = p.to_ascii_lowercase();
        let name = p.rsplit('/').next().unwrap_or(p);
        EXCLUDED_PORTS.iter().any(|e| name == *e) || excluded_by_file(name)
            || l.contains("bluetooth") || l.contains("airpods")
            || (cfg!(target_os = "macos") && l.starts_with("/dev/tty."))
    };
    // Only USB serial adapters are tried (cf. IsCandidatePort 0x1005a85e4): a Bluetooth
    // speaker's port (cu.JBLCharge3…) can hang when opened. Windows COM names say nothing, so all are kept.
    let usb_like = |p: &str| {
        let l = p.to_ascii_lowercase();
        cfg!(windows) || l.contains("usb") || l.contains("acm") || l.contains("wch") || l.contains("serial")
    };
    ports.iter().filter(|p| !skip(p) && usb_like(p)).cloned().collect()
}

pub struct BeamForge {
    pub doc: Document,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub undo: Vec<Document>,
    pub redo: Vec<Document>,
    pub last_checkpoint_frame: u64,
    pub selection: Vec<u64>,
    pub tool: Tool,
    pub view: View,
    pub drag: Drag,
    pub line_pts: Vec<P2>,
    pub cursor_world: Option<P2>,
    pub clipboard: Vec<Shape>,
    pub current_layer: usize,
    pub selected_row: Option<usize>,
    /// The canvas selection the layer list last followed: when it changes, the row of the last
    /// selected object's layer is selected (the person's request).
    pub row_follows: Vec<u64>,
    pub right_tab: RightTab,
    /// Width of the right options panel (drag its left edge); remembered in panel_width.json.
    pub right_width: f32,
    /// Set by Finestra > Reimposta disposizione predefinita: next frame forces the default width.
    pub reset_layout: bool,
    pub job_opts: JobOptions,
    pub laser: Laser,
    pub connected: Option<String>,
    pub autocon: AutoConnect,
    pub ports: Vec<String>,
    pub port_sel: String,
    pub machine_state: String,
    pub head_pos: Option<P2>,
    pub job_progress: Option<(usize, usize)>,
    pub paused: bool,
    pub console: Vec<String>,
    pub console_input: String,
    pub jog_dist: f64,
    pub jog_speed: f64,
    pub preview: Option<Preview>,
    pub show_device: bool,
    pub show_about: bool,
    /// Help > Updates: the window, and the GitHub check behind it (update.rs).
    pub show_updates: bool,
    pub updates: crate::update::Checker,
    /// The daily automatic check still has to run this session.
    pub auto_update_pending: bool,
    /// The updated program to start when this one exits ("Riavvia ora").
    pub relaunch_on_exit: Option<PathBuf>,
    /// An import reading its file in the background (import_job.rs).
    pub import_job: Option<crate::import_job::ImportJob>,
    pub show_settings: bool,
    /// the general settings (Impostazioni), see prefs.rs
    pub prefs: crate::prefs::Prefs,
    /// the Impostazioni window's copy while it is open
    pub prefs_draft: Option<crate::prefs::Prefs>,
    /// the window's "check for updates" switch while it is open
    pub prefs_updates: Option<bool>,
    pub settings_tab: usize,
    /// when the last safety copy was written (seconds of app time)
    pub last_autosave: f64,
    pub measure: crate::measure::MeasureState,
    /// Device settings being edited, applied only on OK.
    pub device_draft: Option<Device>,
    pub editor_layer: Option<usize>,
    /// The layer's settings when the editor opened, restored by Annulla.
    pub editor_orig: Option<(usize, CutSetting)>,
    /// 0 = Comune, 1 = Avanzate
    pub editor_tab: u8,
    /// Saved defaults per layer (Rendi predefinito).
    pub cut_defaults: HashMap<usize, CutSetting>,
    pub textures: HashMap<u64, egui::TextureHandle>,
    /// what each picture's texture was made from (its layer's image settings), so a change of
    /// image mode redraws it
    pub texture_keys: HashMap<u64, u64>,
    pub anchor: usize,
    pub lock_ratio: bool,
    /// Amount the Ruota box turns the selection by (QNumEdit_ValueChanged 0x100c53f38).
    pub rot_input: f64,
    pub rot_for: Vec<u64>,
    /// Width/height scale boxes in % (QXY_ValueChanged 0x100c540e0: factor = value / 100).
    pub scale_pct: [f64; 2],
    pub status: String,
    pub screenshot: Option<ScreenshotState>,
    pub new_text: String,
    pub text_size: f64,
    pub text_font: String,
    pub polygon_sides: u32,
    /// The shape the shapes button in the tool bar shows (last one picked from its menu).
    pub shape_tool: Tool,
    /// manual tab being dragged: (shape index, tab index)
    pub tab_drag: Option<(usize, usize)>,
    pub shape_menu_open: bool,
    /// The tool bar's combine-objects menu, and the assistant window when open.
    pub combine_menu_open: bool,
    pub combine_assist: Option<crate::combine::CombineAssist>,
    /// The operation the combine button shows and runs on a plain click (None = the assistant).
    pub combine_current: Option<crate::combine::CombineOp>,
    /// A held press already opened a button's menu; its release is not a click.
    pub menu_hold_opened: bool,
    /// Test input played back frame by frame (BEAMFORGE_CLICKS).
    pub input_script: Vec<(u32, egui::Event)>,
    pub input_frame: u32,
    pub title_cache: String,
    pub system_menu: Option<crate::sysmenu::SystemMenu>,
    /// Offset dialog: its settings (kept between uses, as the original's preferences), whether it
    /// is open, the live preview and whether the selected objects overlap.
    pub offset_settings: crate::offset::OffsetSettings,
    pub offset_open: bool,
    pub offset_preview: Vec<Vec<Polyline>>,
    pub offset_overlap: bool,
    /// Laser tools → Test intervallo / Test materiale (open when Some). The interval settings are
    /// kept in the settings folder like the original's IntervalTest_* preferences.
    pub interval_test: Option<crate::lasertest::IntervalTest>,
    pub material_test: Option<crate::lasertest::MaterialTest>,
    pub material_presets: Vec<(String, crate::lasertest::MaterialTest)>,
    pub material_preset_sel: String,
    /// What waits on the "save before closing?" question (MainWindow::CloseProject).
    pub confirm: Option<PendingClose>,
    /// The person already answered: let the window close.
    pub allow_close: bool,
    /// The person's saved lasers (Periferiche) and which one is in use.
    pub device_list: crate::devices::DeviceList,
    pub device_sel: Option<usize>,
    /// The Periferiche window, open when Some.
    pub devices_ui: Option<crate::devices::DevicesUi>,
}

/// An action that closes the current project and waits for Salva / Non salvare / Annulla.
#[derive(Clone, Debug, PartialEq)]
pub enum PendingClose {
    Quit,
    New,
    Open(Option<PathBuf>),
}

impl BeamForge {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        let mut app = Self::create(cc, screenshot);
        // the general settings; checks start from the defaults so a run is always the same
        app.prefs = if app.screenshot.is_some() { crate::prefs::Prefs::default() } else { crate::prefs::Prefs::load() };
        app.prefs.dark_background = !crate::theme::p().light;
        app.apply_text_defaults();
        crate::update::clean_leftovers();
        app.auto_update_pending = app.screenshot.is_none()
            && crate::i18n::load_json::<bool>("update_auto.json").unwrap_or(true)
            && crate::i18n::load_json::<u64>("update_check_day.json") != Some(crate::update::today());
        // start with the default laser from Periferiche
        if let Some(i) = app.device_list.default {
            app.use_device(i);
        }
        // test hook: show the save question at start (for --screenshot checks)
        if std::env::var_os("BEAMFORGE_SHOW_SAVE_PROMPT").is_some() {
            app.confirm = Some(PendingClose::Quit);
        }
        // BEAMFORGE_OPEN=settings,device,about (for testing) opens those windows at start
        if let Ok(v) = std::env::var("BEAMFORGE_OPEN") {
            app.show_settings = v.contains("settings");
            // settings,tab3: open Impostazioni on its fourth tab (for testing)
            if let Some(t) = v.split(',').find_map(|w| w.strip_prefix("tab")).and_then(|n| n.parse::<usize>().ok()) {
                app.settings_tab = t;
            }
            app.show_device = v.contains("device") && !v.contains("devices");
            app.show_about = v.contains("about");
            app.show_updates = v.contains("updates");
            if v.contains("updates-new") {
                // a pretend newer release, to see the install button
                *app.updates.state.lock().unwrap() = crate::update::State::Newer(crate::update::Release {
                    version: "1.0.0-beta.2".into(),
                    name: "BeamForge 1.0.0 Beta 2".into(),
                    notes: "Sample notes.".into(),
                    page: crate::update::RELEASES_PAGE.into(),
                    beta: true,
                    asset: Some(("BeamForge-1.0.0-beta.2-macos-universal.zip".into(), String::new())),
                    asset_size: 12_000_000,
                });
            } else if app.show_updates {
                app.updates.start(&cc.egui_ctx, false);
            }
            // BEAMFORGE_OPEN=shapes: the shapes menu open, with one of each new shape drawn
            if v.contains("shapes") {
                app.shape_menu_open = true;
                let star = |middle| Geometry::Star { points: STAR_POINTS, rx: 40.0, ry: 40.0, inner: STAR_INNER, middle };
                for (g, x) in [(Geometry::Polygon { sides: 3, r: 40.0, ry: Some(40.0) }, 60.0), (Geometry::Polygon { sides: 8, r: 40.0, ry: Some(40.0) }, 150.0), (star(None), 240.0), (star(Some(DUAL_STAR_MIDDLE)), 330.0)] {
                    app.doc.add_shape(Shape::new(0, g, 0, x, 200.0));
                }
            }
            // BEAMFORGE_OPEN=photo,preview (for testing): a 150 × 150 mm photo-like picture on an
            // Image layer at 0.1 mm, then the preview of its engraving (a job of about a million moves)
            if v.contains("photo") {
                let px = 1500u32;
                let gray: Vec<u8> = (0..px * px)
                    .map(|i| {
                        let (x, y) = ((i % px) as f64 / px as f64, (i / px) as f64 / px as f64);
                        (127.0 + 120.0 * ((x * 37.0).sin() * (y * 23.0).cos())) as u8
                    })
                    .collect();
                app.doc.layers[0].mode = CutMode::Image;
                app.doc.layers[0].interval = 0.1;
                // photo,<mode>: the layer's image mode by its saved name (dot, halftone, newsprint...)
                for m in ImageMode::ALL {
                    let name = m.label().to_lowercase().replace(' ', "");
                    if v.split(',').any(|w| w == name) {
                        app.doc.layers[0].image_mode = m;
                    }
                }
                app.doc.add_shape(Shape::new(0, Geometry::Image { w: 150.0, h: 150.0, px_w: px, px_h: px, gray }, 0, 100.0, 100.0));
                if v.contains("preview") {
                    app.open_preview();
                    // photo,preview,zoom: the preview zoomed in 8× on the middle
                    if v.contains("zoom")
                        && let Some(pv) = app.preview.as_mut()
                    {
                        pv.zoom_to(8.0);
                    }
                }
            }
            // BEAMFORGE_OPEN=legend: a black cut rectangle and a red filled ellipse, then the preview
            // (its legend tells the two apart)
            if v.contains("legend") {
                app.doc.add_shape(Shape::new(0, Geometry::Rect { w: 70.0, h: 140.0, radius: 0.0 }, 0, 120.0, 200.0));
                app.doc.layers[2].mode = CutMode::Fill;
                app.doc.add_shape(Shape::new(0, Geometry::Ellipse { rx: 15.0, ry: 22.0 }, 2, 120.0, 210.0));
                // legend,text: filled text on a blue Fill layer too
                if v.contains("text") {
                    app.doc.layers[1].mode = CutMode::Fill;
                    app.new_text = "BeamForge".into();
                    app.text_size = 20.0;
                    app.current_layer = 1;
                    app.make_text_shape([120.0, 150.0]);
                }
                app.open_preview();
            }
            // BEAMFORGE_OPEN=rotate: one rectangle picked and turned 20°, to show the rotate arrows
            // (run with BEAMFORGE_SHIFT=1 so Shift counts as held)
            if v.contains("rotate") {
                let a = app.doc.add_shape(Shape::new(0, Geometry::Rect { w: 120.0, h: 70.0, radius: 0.0 }, 0, 200.0, 220.0));
                app.selection = vec![a];
                app.rotate_sel(20.0);
            }
            // BEAMFORGE_OPEN=combine: two overlapping squares picked, the combine menu open
            // (and with "assist" the assistant showing the union)
            if v.contains("combine") {
                let a = app.doc.add_shape(Shape::new(0, Geometry::Rect { w: 80.0, h: 80.0, radius: 0.0 }, 0, 160.0, 220.0));
                let b = app.doc.add_shape(Shape::new(0, Geometry::Ellipse { rx: 45.0, ry: 45.0 }, 0, 215.0, 175.0));
                app.selection = vec![a, b];
                app.combine_menu_open = !v.contains("assist");
                if v.contains("assist") {
                    app.open_combine_assistant();
                    if let Some(c) = app.combine_assist.as_mut() {
                        c.applied = Some(crate::combine::CombineOp::Subtract);
                    }
                }
            }
            if v.contains("devices") {
                app.open_devices();
                if v.contains("manual") {
                    if let Some(st) = app.devices_ui.as_mut() {
                        let mut d = Device::default();
                        d.name = tr("Il mio laser").into();
                        st.editor = Some((None, d));
                    }
                }
            }
            if v.contains("editor") {
                app.editor_layer = Some(0);
                app.editor_tab = if v.contains("advanced") { 1 } else { 0 };
                if v.contains("offsetfill") {
                    app.doc.layers[0].mode = CutMode::OffsetFill;
                } else if v.contains("image") {
                    let cs = &mut app.doc.layers[0];
                    cs.mode = CutMode::Image;
                    cs.image_mode = ImageMode::Stucki;
                    cs.speed = DEFAULT_SPEED_MM_MIN;
                    cs.max_power = 20.0;
                    cs.min_power = 20.0;
                    cs.interval = 0.1;
                    cs.angle = 0.0;
                    cs.bidir = true;
                    cs.negative = false;
                    cs.output = true;
                    cs.air = true;
                    cs.passes = 1;
                    cs.ramp_length = 0.0;
                    cs.dot_width_on = false;
                    cs.dot_width = 0.08;
                    cs.pass_through = false;
                    cs.cells_per_inch = 50.0;
                    cs.halftone_angle = 22.5;
                    cs.fill_grouping = FillGrouping::MergeAll;
                } else if v.contains("fill") {
                    app.doc.layers[0].mode = CutMode::Fill;
                }
            }
        }
        app
    }

    fn create(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        // theme: the saved choice (themes/theme.ini in the settings folder), dark by default;
        // BEAMFORGE_THEME=light|dark (for testing) overrides it for this run only
        let light = std::env::var("BEAMFORGE_THEME").ok().map(|t| t == "light").unwrap_or_else(crate::theme::saved_light);
        crate::theme::set_light(&cc.egui_ctx, light);
        crate::theme::use_system_fonts(&cc.egui_ctx);
        crate::theme::STEADY_NO_SAVE.store(screenshot.is_some(), std::sync::atomic::Ordering::Relaxed);
        // language: the saved choice, otherwise the system's
        crate::i18n::set(crate::i18n::load_choice().unwrap_or_else(crate::i18n::system_lang));
        // BEAMFORGE_LANG=de (for testing) overrides it for this run only
        if let Some(l) = std::env::var("BEAMFORGE_LANG").ok().and_then(|c| crate::i18n::Lang::from_code(&c)) {
            crate::i18n::set(l);
        }
        let ports = crate::laser::list_ports();
        BeamForge {
            doc: Document::default(),
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            last_checkpoint_frame: 0,
            selection: Vec::new(),
            tool: Tool::Select,
            view: View { zoom: 2.0, offset: egui::vec2(40.0, 840.0), fit_pending: true, auto_fit: true, last_area: egui::Vec2::ZERO },
            drag: Drag::None,
            line_pts: Vec::new(),
            cursor_world: None,
            clipboard: Vec::new(),
            current_layer: 0,
            selected_row: None,
            row_follows: Vec::new(),
            right_tab: RightTab::Laser,
            reset_layout: false,
            right_width: crate::i18n::load_json::<f32>("panel_width.json").filter(|w| w.is_finite()).unwrap_or(crate::panels::RIGHT_WIDTH_DEFAULT).clamp(crate::panels::RIGHT_WIDTH_MIN, crate::panels::RIGHT_WIDTH_MAX),
            job_opts: JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: true },
            laser: Laser::new(cc.egui_ctx.clone()),
            connected: None,
            port_sel: String::new(),
            autocon: AutoConnect::default(),
            ports,
            machine_state: tr("Scollegato").into(),
            head_pos: None,
            job_progress: None,
            paused: false,
            console: vec![tr("BeamForge 1.0 — software laser libero, licenza open source Apache 2.0. Nessuna attivazione richiesta.").into()],
            console_input: String::new(),
            jog_dist: 10.0,
            jog_speed: 3000.0,
            preview: None,
            show_device: false,
            show_about: false,
            show_updates: false,
            updates: Default::default(),
            auto_update_pending: false,
            relaunch_on_exit: None,
            import_job: None,
            show_settings: false,
            prefs: crate::prefs::Prefs::default(),
            prefs_draft: None,
            prefs_updates: None,
            settings_tab: 0,
            last_autosave: 0.0,
            measure: Default::default(),
            device_draft: None,
            editor_layer: None,
            editor_orig: None,
            editor_tab: 0,
            cut_defaults: crate::i18n::load_cut_defaults(),
            textures: HashMap::new(),
            texture_keys: HashMap::new(),
            anchor: 4,
            lock_ratio: false,
            rot_input: 0.0,
            rot_for: Vec::new(),
            scale_pct: [100.0, 100.0],
            status: tr("Pronto").into(),
            screenshot: screenshot.map(|path| ScreenshotState { path, frames: 0, requested: false, image: Arc::new(Mutex::new(None)) }),
            new_text: "BeamForge".into(),
            text_size: 20.0,
            text_font: crate::text::fonts()[0].name.clone(),
            polygon_sides: 6,
            shape_tool: Tool::Rect,
            tab_drag: None,
            shape_menu_open: false,
            combine_menu_open: false,
            combine_assist: None,
            combine_current: Some(crate::combine::CombineOp::Union),
            menu_hold_opened: false,
            input_script: scripted_input(),
            input_frame: 0,
            title_cache: String::new(),
            system_menu: crate::sysmenu::install(&cc.egui_ctx),
            confirm: None,
            allow_close: false,
            offset_settings: Default::default(),
            offset_open: false,
            offset_preview: Vec::new(),
            offset_overlap: false,
            interval_test: None,
            material_test: None,
            material_presets: crate::i18n::load_json("material_test_presets.json").unwrap_or_default(),
            material_preset_sel: String::new(),
            device_list: crate::devices::DeviceList::load(),
            device_sel: None,
            devices_ui: None,
        }
    }

    // ---------- undo ----------

    /// Save the document for undo. Changes on consecutive frames (a drag) share one step.
    pub fn checkpoint(&mut self, frame: u64) {
        if frame > self.last_checkpoint_frame + 1 || frame == 0 {
            self.undo.push(self.doc.clone());
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.last_checkpoint_frame = frame;
        self.dirty = true;
    }

    pub fn checkpoint_now(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        self.last_checkpoint_frame = 0;
    }

    pub fn do_undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.clean_selection();
            self.status = tr("Annullato").into();
        }
    }

    pub fn do_redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.clean_selection();
            self.status = tr("Ripetuto").into();
        }
    }

    fn clean_selection(&mut self) {
        let ids: Vec<u64> = self.doc.shapes.iter().map(|s| s.id).collect();
        self.selection.retain(|id| ids.contains(id));
    }

    // ---------- files ----------

    /// CloseProject 0x100c4b2cc asks only when the project is modified and holds a shape.
    pub fn needs_save_prompt(&self) -> bool {
        self.dirty && !self.doc.shapes.is_empty() && self.screenshot.is_none()
    }

    /// Run `action` now, or first ask whether to save the modified project.
    pub fn request_close(&mut self, action: PendingClose, ctx: &egui::Context) {
        if self.needs_save_prompt() {
            self.confirm = Some(action);
        } else {
            self.finish_close(action, ctx);
        }
    }

    fn finish_close(&mut self, action: PendingClose, ctx: &egui::Context) {
        match action {
            PendingClose::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            PendingClose::New => self.new_doc_now(),
            PendingClose::Open(Some(p)) => self.open_path_now(p),
            PendingClose::Open(None) => self.open_dialog_now(),
        }
    }

    /// The question itself: Salva, Non salvare (Discard) or Annulla, as in the original.
    pub fn save_prompt(&mut self, ctx: &egui::Context) {
        // closing the window (red button, ⌘Q, Esci) goes through the same question
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.needs_save_prompt() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.confirm.is_none() {
                    self.confirm = Some(PendingClose::Quit);
                }
            }
        }
        let Some(action) = self.confirm.clone() else { return };
        enum Choice {
            Save,
            Discard,
            Cancel,
        }
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("save_prompt")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(tr("Salvare il progetto?"));
            ui.add_space(6.0);
            ui.label(tr("Il progetto è stato modificato.\n\nVuoi salvarlo prima di chiuderlo?"));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new(tr("Salva")).strong()).min_size(egui::vec2(96.0, 26.0))).clicked() {
                    choice = Some(Choice::Save);
                }
                if ui.add(egui::Button::new(tr("Non salvare")).min_size(egui::vec2(96.0, 26.0))).clicked() {
                    choice = Some(Choice::Discard);
                }
                if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(egui::vec2(96.0, 26.0))).clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
        if choice.is_none() && (modal.should_close() || ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
            choice = Some(Choice::Cancel);
        }
        match choice {
            Some(Choice::Save) => {
                self.confirm = None;
                self.save(false);
                // a cancelled or failed save keeps the project open (SaveProject returns false)
                if !self.dirty {
                    self.finish_close(action, ctx);
                }
            }
            Some(Choice::Discard) => {
                self.confirm = None;
                self.finish_close(action, ctx);
            }
            Some(Choice::Cancel) => self.confirm = None,
            None => {}
        }
    }

    pub fn new_doc(&mut self, ctx: &egui::Context) {
        self.request_close(PendingClose::New, ctx);
    }

    fn new_doc_now(&mut self) {
        let device = self.doc.device.clone();
        self.checkpoint_now();
        self.doc = Document::default();
        self.doc.device = device;
        self.path = None;
        self.dirty = false;
        self.selection.clear();
        self.view.fit_pending = true;
        self.status = tr("Nuovo progetto").into();
    }

    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        self.request_close(PendingClose::Open(None), ctx);
    }

    pub fn open_path(&mut self, p: PathBuf, ctx: &egui::Context) {
        self.request_close(PendingClose::Open(Some(p)), ctx);
    }

    fn open_dialog_now(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter(tr("BeamForge project"), &["beamforge", "json"]).pick_file() {
            self.open_path_now(p);
        }
    }

    pub fn open_path_now(&mut self, p: PathBuf) {
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Document>(&s).map_err(|e| e.to_string())) {
            Ok(mut d) => {
                while d.layers.len() < LAYER_COUNT {
                    let i = d.layers.len();
                    let mut c = CutSetting::default();
                    c.name = layer_name(i);
                    d.layers.push(c);
                }
                self.checkpoint_now();
                self.doc = d;
                self.path = Some(p.clone());
                self.dirty = false;
                self.selection.clear();
                self.textures.clear();
                self.view.fit_pending = true;
                self.status = trf("Aperto {}", &[&p.display()]);
            }
            Err(e) => self.status = trf("Impossibile aprire il progetto: {}", &[&e]),
        }
    }

    pub fn save(&mut self, save_as: bool) {
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new().add_filter(tr("BeamForge project"), &["beamforge"]).set_file_name("design.beamforge").save_file()
        } else {
            self.path.clone()
        };
        let Some(p) = path else { return };
        match serde_json::to_string(&self.doc).map_err(|e| e.to_string()).and_then(|s| crate::fileicon::safe_write(&p, s.as_bytes())) {
            Ok(()) => {
                crate::fileicon::mark_saved(&p);
                self.path = Some(p.clone());
                self.dirty = false;
                // MainWindow::SaveProject 0x10098548c: "Project file saved as %1."
                self.status = trf("File di progetto salvato come {}.", &[&p.display()]);
            }
            Err(e) => self.status = trf("Salvataggio non riuscito: {}", &[&e]),
        }
    }

    pub fn import_dialog(&mut self) {
        use crate::formats::{IMAGE_EXTS, MODEL_EXTS, PCB_EXTS, VECTOR_EXTS, all_exts};
        // the same groups as the original's Import dialog
        let all = all_exts();
        if let Some(p) = rfd::FileDialog::new()
            .add_filter(tr("Tutti supportati"), &all)
            .add_filter(tr("File di immagine"), IMAGE_EXTS)
            .add_filter(tr("File vettoriali"), VECTOR_EXTS)
            .add_filter(tr("File di oggetti 3D"), MODEL_EXTS)
            .add_filter(tr("File PCB"), PCB_EXTS)
            .pick_file()
        {
            self.import_path(&p);
        }
    }

    /// Where an imported drawing's middle goes: the middle of what the canvas shows, or the middle
    /// of the work area before the canvas has been drawn once.
    fn import_point(&self) -> P2 {
        let a = self.view.last_area;
        if a.x > 1.0 && a.y > 1.0 && self.view.zoom > 0.0 {
            let z = self.view.zoom;
            [((a.x / 2.0 - self.view.offset.x) / z) as f64, ((self.view.offset.y - a.y / 2.0) / z) as f64]
        } else {
            [self.doc.device.width / 2.0, self.doc.device.height / 2.0]
        }
    }

    /// Import a file without freezing the window: the file is read and turned into shapes on a
    /// background thread (`poll_import` adds them when done). A file iCloud keeps only in the cloud
    /// ("dataless") makes the first read wait for its download, which can take minutes or never end
    /// offline; the window stays usable meanwhile and the import can be cancelled.
    /// --screenshot runs import at once, so their test setups see the shapes straight away.
    pub fn import_path(&mut self, p: &std::path::Path) {
        // BEAMFORGE_ASYNC_IMPORT=1 (for testing): a --screenshot run imports in the background too,
        // to see the "Importing…" window
        if self.screenshot.is_some() && std::env::var_os("BEAMFORGE_ASYNC_IMPORT").is_none() {
            let r = crate::formats::import_file(p);
            self.finish_import(p, r);
            return;
        }
        let icloud = crate::import_job::in_cloud_only(p);
        if icloud {
            crate::import_job::request_download(p);
        }
        self.import_job = Some(crate::import_job::ImportJob::start(p.to_path_buf(), icloud));
    }

    /// Show the import in progress and add its shapes once the background read is done.
    pub fn poll_import(&mut self, ctx: &egui::Context) {
        let Some(job) = self.import_job.as_ref() else { return };
        if let Some(result) = job.take_result() {
            let path = job.path.clone();
            self.import_job = None;
            self.finish_import(&path, result);
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        if crate::import_job::window(ctx, job) {
            // cancelled: the background read is left to end on its own and its result dropped
            self.status = trf("Importazione annullata: {}", &[&job.path.display()]);
            self.import_job = None;
        }
    }

    fn finish_import(&mut self, p: &std::path::Path, result: Result<crate::formats::Imported, String>) {
        use crate::formats::Imported;
        let imported = match result {
            Ok(i) => i,
            Err(e) => {
                // the original shows a message box (Core_Message::ShowError in ImportFileShapes
                // 0x100982974); a line in the status bar alone looked like nothing happened
                if self.screenshot.is_none() {
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title(tr("Importazione non riuscita"))
                        .set_description(format!("{}\n\n{}", p.display(), e))
                        .show();
                }
                self.status = e;
                return;
            }
        };
        match imported {
            Imported::Placed(shapes) => {
                // a project's shapes keep their place; its groups get new ids here
                self.checkpoint_now();
                let mut map: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
                let outer = (self.prefs.group_imported && shapes.len() > 1).then(|| self.doc.alloc_id());
                let mut ids = Vec::new();
                for mut s in shapes {
                    let mut gs: Vec<u64> = outer.into_iter().collect();
                    for g in &s.groups {
                        let id = *map.entry(*g).or_insert_with(|| self.doc.alloc_id());
                        gs.push(id);
                    }
                    s.groups = gs;
                    ids.push(self.doc.add_shape(s));
                }
                // "Seleziona le forme importate"
                self.selection = if self.prefs.select_imported { ids } else { Vec::new() };
                self.status = trf("Importato {}", &[&p.display()]);
            }
            Imported::Shapes(mut shapes) => {
                self.checkpoint_now();
                // MainWindow::ImportFile (0x100980068) hands ImportFileShapes the work area's point with
                // ALIGN_H/ALIGN_V = centre: the drawing's middle goes there instead of staying at the file's
                // own coordinates (a DXF drawn far from 0,0 used to land off the page and seem not
                // imported). PCB files keep theirs (TranslatorDescriptor::UsesAbsoluteCoordinates), so
                // the layers of one board stay lined up.
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if !crate::formats::PCB_EXTS.contains(&ext.as_str()) {
                    let mut b = BBox::empty();
                    for s in &shapes {
                        let sb = s.bbox();
                        if sb.is_valid() {
                            b.add(sb.min);
                            b.add(sb.max);
                        }
                    }
                    if b.is_valid() {
                        let (c, target) = (b.center(), self.import_point());
                        for s in &mut shapes {
                            s.tx += target[0] - c[0];
                            s.ty += target[1] - c[1];
                        }
                    }
                }
                // "Raggruppa le forme importate": the file comes in as one group of its pieces
                let gid = (self.prefs.group_imported && shapes.len() > 1).then(|| self.doc.alloc_id());
                let mut ids = Vec::new();
                for mut s in shapes {
                    if let Some(g) = gid {
                        s.groups = vec![g];
                    }
                    ids.push(self.doc.add_shape(s));
                }
                // "Seleziona le forme importate"
                self.selection = if self.prefs.select_imported { ids } else { Vec::new() };
                self.status = trf("Importato {}", &[&p.display()]);
            }
            Imported::Image(mut s) => {
                self.checkpoint_now();
                // put images (pictures and 3D depth maps) on a free layer set to Image mode
                let used = self.doc.used_layers();
                let layer = (0..CUT_LAYER_COUNT).find(|l| !used.contains(l)).unwrap_or(0);
                if self.doc.layers[layer].mode != CutMode::Image {
                    let cs = &mut self.doc.layers[layer];
                    cs.mode = CutMode::Image;
                    cs.speed = DEFAULT_SPEED_MM_MIN;
                    cs.max_power = 20.0;
                    cs.interval = 0.1;
                }
                s.layer = layer;
                let id = self.doc.add_shape(s);
                self.selection = vec![id];
                self.status = trf("Importato {}", &[&p.display()]);
            }
        }
    }

    pub fn export_svg(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name("design.svg").save_file() else { return };
        let w = self.doc.device.width;
        let h = self.doc.device.height;
        let mut s = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n"
        );
        for sh in &self.doc.shapes {
            if matches!(sh.geom, Geometry::Image { .. }) {
                continue;
            }
            let c = PALETTE[sh.layer];
            let mut d = String::new();
            for pl in sh.world_paths() {
                for (i, p) in pl.pts.iter().enumerate() {
                    d.push_str(&format!("{}{:.3},{:.3} ", if i == 0 { "M" } else { "L" }, p[0], h - p[1]));
                }
                if pl.closed {
                    d.push_str("Z ");
                }
            }
            s.push_str(&format!(
                "  <path d=\"{}\" fill=\"none\" stroke=\"#{:02x}{:02x}{:02x}\" stroke-width=\"0.1\"/>\n",
                d.trim(),
                c[0],
                c[1],
                c[2]
            ));
        }
        s.push_str("</svg>\n");
        match std::fs::write(&p, s) {
            Ok(()) => self.status = trf("Esportato {}", &[&p.display()]),
            Err(e) => self.status = trf("Impossibile esportare: {}", &[&e]),
        }
    }

    // ---------- job ----------

    pub fn plan_job(&self) -> Job {
        gcode::plan(&self.doc, &self.selection, &self.job_opts)
    }

    pub fn job_gcode(&self) -> (Job, String) {
        let job = self.plan_job();
        let g = gcode::write_gcode(&self.doc, &job, &self.job_opts);
        (job, g)
    }

    /// A job for a test document (Test intervallo / Test materiale): everything in it is cut,
    /// with the project's start-from and origin settings.
    pub fn test_job(&self, doc: &Document) -> (Job, String) {
        let opts = gcode::JobOptions { selected_only: false, ..self.job_opts.clone() };
        let job = gcode::plan(doc, &[], &opts);
        let g = gcode::write_gcode(doc, &job, &opts);
        (job, g)
    }

    pub fn preview_test(&mut self, doc: Document) {
        let (job, gcode) = self.test_job(&doc);
        self.preview = Some(Preview::new(job, gcode, Some(doc), &self.doc));
    }

    pub fn frame_test(&mut self, doc: &Document) {
        let opts = gcode::JobOptions { selected_only: false, ..self.job_opts.clone() };
        let job = gcode::plan(doc, &[], &opts);
        if job.bounds.is_none() {
            self.status = tr("Niente da inquadrare.").into();
            return;
        }
        let lines = gcode::frame_gcode(doc, &job, &opts, self.jog_speed, &self.frame_power());
        self.send_lines(lines);
    }

    pub fn start_test(&mut self, doc: &Document) {
        let (job, g) = self.test_job(doc);
        self.send_job(job, g);
    }

    pub fn save_test_gcode(&mut self, doc: &Document, name: &str) {
        let (job, g) = self.test_job(doc);
        self.write_gcode_file(job, g, name);
    }

    pub fn pause_toggle(&mut self) {
        self.laser.send(Cmd::Realtime(if self.paused { b'~' } else { b'!' }));
        self.paused = !self.paused;
    }

    pub fn stop_job(&mut self) {
        self.laser.send(Cmd::StopJob);
        self.paused = false;
    }

    pub fn save_gcode(&mut self) {
        let (job, g) = self.job_gcode();
        self.write_gcode_file(job, g, "design.gc");
    }

    pub fn write_gcode_file(&mut self, job: Job, g: String, name: &str) {
        if job.moves.is_empty() {
            self.status = tr("Niente da emettere: aggiungi forme su un livello con Emissione attiva.").into();
            return;
        }
        let Some(p) = rfd::FileDialog::new().add_filter("G-code", &["gc", "nc", "gcode"]).set_file_name(name).save_file() else { return };
        match std::fs::write(&p, g) {
            Ok(()) => self.status = trf("G-code salvato in {}", &[&p.display()]),
            Err(e) => self.status = trf("Impossibile salvare il G-code: {}", &[&e]),
        }
    }

    pub fn start_job(&mut self) {
        let (job, g) = self.job_gcode();
        self.send_job(job, g);
    }

    pub fn send_job(&mut self, job: Job, g: String) {
        if job.moves.is_empty() {
            self.status = tr("Niente da incidere: aggiungi forme su un livello con Emissione attiva.").into();
            return;
        }
        if self.connected.is_none() {
            self.status = tr("Nessun laser trovato: collegalo con il cavo USB oppure usa Salva G-code.").into();
            self.right_tab = RightTab::Laser;
            return;
        }
        let lines: Vec<String> = g.lines().map(|l| l.to_string()).collect();
        self.laser.send(Cmd::Lines(lines, true));
        self.job_progress = Some((0, 1));
        self.paused = false;
        self.status = tr("Lavoro avviato").into();
    }

    pub fn run_gcode_file(&mut self) {
        if self.connected.is_none() {
            self.status = tr("Nessun laser trovato: collegalo con il cavo USB.").into();
            return;
        }
        if let Some(p) = rfd::FileDialog::new().add_filter("G-code", &["gc", "nc", "gcode", "txt"]).pick_file() {
            if let Ok(s) = std::fs::read_to_string(&p) {
                self.laser.send(Cmd::Lines(s.lines().map(|l| l.to_string()).collect(), true));
                self.job_progress = Some((0, 1));
                self.status = trf("In esecuzione {}", &[&p.display()]);
            }
        }
    }

    fn frame_power(&self) -> String {
        gcode::frame_power_s(self.doc.device.laser_frame, self.doc.device.s_max)
    }

    pub fn frame_job(&mut self) {
        let job = self.plan_job();
        if job.bounds.is_none() {
            self.status = tr("Niente da inquadrare.").into();
            return;
        }
        let lines = gcode::frame_gcode(&self.doc, &job, &self.job_opts, self.jog_speed, &self.frame_power());
        self.send_lines(lines);
    }

    /// Trace the rubber-band outline around the shapes (beam at 1%), at the Move panel speed.
    pub fn frame_outline_job(&mut self) {
        let job = self.plan_job();
        let lines = gcode::outline_frame_gcode(&self.doc, &job, &self.job_opts, self.jog_speed, &self.frame_power());
        if lines.is_empty() {
            self.status = tr("Niente da inquadrare.").into();
            return;
        }
        self.send_lines(lines);
    }

    /// Automatic connection: while no laser answers, try the chosen port (or every port when
    /// "Automatica" is chosen) one at a time; keep a port only when GRBL answers on it.
    pub fn auto_connect(&mut self, ctx: &egui::Context) {
        if self.screenshot.is_some() {
            return;
        }
        let now = Instant::now();
        let ac = &mut self.autocon;
        if let Some(dl) = ac.verify_deadline {
            if ac.verified {
                ac.verify_deadline = None;
            } else if now >= dl {
                ac.verify_deadline = None;
                ac.quiet = true;
                self.laser.send(Cmd::Disconnect);
            } else {
                ctx.request_repaint_after(dl - now);
            }
            return;
        }
        if let Some(c) = &self.connected {
            // the person picked another port: move to it
            if !self.port_sel.is_empty() && *c != self.port_sel && !ac.pending {
                ac.pending = true;
                ac.quiet = true;
                ac.next_try = now;
                self.laser.send(Cmd::Disconnect);
            }
            return;
        }
        if ac.pending {
            return;
        }
        if now < ac.next_try {
            ctx.request_repaint_after(ac.next_try - now);
            return;
        }
        if ac.queue.is_empty() {
            self.ports = crate::laser::list_ports();
            ac.queue = auto_port_order(&self.port_sel, &self.ports);
            if ac.queue.is_empty() {
                ac.next_try = now + AUTOCONNECT_RETRY;
                self.machine_state = tr("In cerca del laser…").into();
                ctx.request_repaint_after(AUTOCONNECT_RETRY);
                return;
            }
        }
        let p = ac.queue.remove(0);
        ac.pending = true;
        ac.quiet = true;
        self.machine_state = tr("In cerca del laser…").into();
        self.laser.send(Cmd::Connect(p, self.doc.device.baud));
    }

    pub fn send_lines(&mut self, lines: Vec<String>) {
        if self.connected.is_none() {
            self.status = tr("Nessun laser collegato.").into();
            return;
        }
        self.laser.send(Cmd::Lines(lines, false));
    }

    pub fn jog(&mut self, dx: f64, dy: f64) {
        self.send_lines(vec![crate::gcode::jog_line(false, dx, dy, self.jog_speed)]);
    }

    pub fn poll_laser(&mut self) {
        for ev in self.laser.poll() {
            match ev {
                Ev::Connected(n) => {
                    self.connected = Some(n);
                    self.autocon.pending = false;
                    self.autocon.verified = false;
                    self.autocon.verify_deadline = Some(Instant::now() + AUTOCONNECT_VERIFY);
                }
                Ev::Disconnected(m) => {
                    let ac = &mut self.autocon;
                    let was_real = ac.verified;
                    ac.pending = false;
                    ac.verified = false;
                    ac.verify_deadline = None;
                    if was_real {
                        ac.queue.clear();
                    }
                    ac.next_try = Instant::now() + if ac.queue.is_empty() && !was_real { AUTOCONNECT_RETRY } else { AUTOCONNECT_NEXT };
                    let quiet = std::mem::replace(&mut ac.quiet, false) && !was_real;
                    self.connected = None;
                    self.job_progress = None;
                    if quiet {
                        self.machine_state = tr("In cerca del laser…").into();
                    } else {
                        self.console.push(m.clone());
                        self.machine_state = tr("Scollegato").into();
                        self.status = m;
                    }
                }
                Ev::Received(l) => {
                    let low = l.to_ascii_lowercase();
                    if !self.autocon.verified && (low.contains("grbl") || low.starts_with('<') || low == "ok") {
                        self.mark_laser_found();
                    }
                    self.push_console(format!("< {l}"))
                }
                Ev::Sent(l) => self.push_console(format!("> {l}")),
                Ev::Progress(a, b) => self.job_progress = Some((a, b.max(1))),
                Ev::JobDone => {
                    self.job_progress = None;
                    self.status = tr("Lavoro terminato").into();
                    self.push_console(tr("Lavoro terminato").into());
                }
                Ev::Status { state, pos } => {
                    if !self.autocon.verified {
                        self.mark_laser_found();
                    }
                    self.machine_state = state;
                    if pos.is_some() {
                        self.head_pos = pos;
                    }
                }
            }
        }
    }

    fn mark_laser_found(&mut self) {
        let Some(n) = self.connected.clone() else { return };
        self.autocon.verified = true;
        self.autocon.verify_deadline = None;
        self.autocon.quiet = false;
        self.autocon.queue.clear();
        self.console.push(trf("Collegato a {}", &[&n]));
        self.status = trf("Collegato a {}", &[&n]);
        self.machine_state = tr("Collegato").into();
    }

    pub fn push_console(&mut self, l: String) {
        self.console.push(l);
        if self.console.len() > 2000 {
            self.console.drain(..500);
        }
    }

    // ---------- edit ----------

    pub fn selected_shapes(&self) -> Vec<Shape> {
        self.doc.shapes.iter().filter(|s| self.selection.contains(&s.id)).cloned().collect()
    }

    pub fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.checkpoint_now();
        let sel = self.selection.clone();
        self.doc.shapes.retain(|s| !sel.contains(&s.id));
        self.selection.clear();
    }

    pub fn copy(&mut self) {
        self.clipboard = self.selected_shapes();
    }

    pub fn cut(&mut self) {
        self.copy();
        self.delete_selection();
    }

    pub fn paste(&mut self, offset: f64) {
        if self.clipboard.is_empty() {
            return;
        }
        self.checkpoint_now();
        let mut ids = Vec::new();
        // pasted copies get their own group ids, so they don't join the originals' groups
        let mut remap: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
        for mut s in self.clipboard.clone() {
            s.id = 0;
            s.tx += offset;
            s.ty -= offset;
            for g in s.groups.iter_mut() {
                let new = *remap.entry(*g).or_insert_with(|| self.doc.alloc_id());
                *g = new;
            }
            ids.push(self.doc.add_shape(s));
        }
        self.selection = ids;
    }

    pub fn duplicate(&mut self) {
        self.copy();
        self.paste(0.0);
        self.status = tr("Duplicato (le copie sono sopra gli originali)").into();
    }

    pub fn select_all(&mut self) {
        self.selection = self.doc.shapes.iter().map(|s| s.id).collect();
    }

    pub fn transform_selection(&mut self, f: impl Fn(&mut Shape, BBox)) {
        if self.selection.is_empty() {
            return;
        }
        self.checkpoint_now();
        let b = self.doc.bbox_of(&self.selection);
        let sel = self.selection.clone();
        for s in self.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
            f(s, b);
        }
    }

    pub fn mirror(&mut self, horizontal: bool) {
        self.transform_selection(|s, b| s.mirror(horizontal, b.center()));
    }

    pub fn rotate_sel(&mut self, deg: f64) {
        self.transform_selection(|s, b| s.rotate_about(deg, b.center()));
    }

    // ---------- groups and arranging (Core_Arrange, MainWindow::on_actionGroup) ----------

    /// Every shape that moves with `id`: the whole outermost group, or the shape alone.
    pub fn group_members(&self, id: u64) -> Vec<u64> {
        match self.doc.find(id).and_then(|s| s.groups.first().copied()) {
            Some(g) => self.doc.shapes.iter().filter(|s| s.groups.first() == Some(&g)).map(|s| s.id).collect(),
            None => vec![id],
        }
    }

    /// Add shape ids to a selection list, expanded to whole groups, keeping the order of picking.
    pub fn expand_into(&self, ids: &[u64], into: &mut Vec<u64>) {
        for id in ids {
            for m in self.group_members(*id) {
                if let Some(i) = into.iter().position(|x| *x == m) {
                    into.remove(i);
                }
                into.push(m);
            }
        }
    }

    /// The selected objects in picking order: a group counts as one object.
    /// The last one is the reference for alignment (Core_Arrange::AlignX 0x1007170cc).
    pub fn selection_items(&self) -> Vec<Vec<u64>> {
        let mut items: Vec<(Option<u64>, Vec<u64>)> = Vec::new();
        for id in &self.selection {
            let Some(s) = self.doc.find(*id) else { continue };
            let key = s.groups.first().copied();
            match key.and_then(|k| items.iter().position(|(g, _)| *g == Some(k))) {
                Some(i) => {
                    // a later pick of the same group moves it to the end
                    let mut it = items.remove(i);
                    it.1.push(*id);
                    items.push(it);
                }
                None => items.push((key, vec![*id])),
            }
        }
        items.into_iter().map(|(_, v)| v).collect()
    }

    fn move_ids(&mut self, ids: &[u64], dx: f64, dy: f64) {
        for s in self.doc.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
            s.tx += dx;
            s.ty += dy;
        }
    }

    /// Group the selected objects (needs 2 or more, MainWindow::on_actionGroup_triggered 0x10067c0d0).
    pub fn group(&mut self) {
        let items = self.selection_items();
        if items.len() < 2 {
            self.status = tr("Seleziona almeno 2 oggetti da raggruppare.").into();
            return;
        }
        self.checkpoint_now();
        let gid = self.doc.alloc_id();
        let sel = self.selection.clone();
        for s in self.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
            s.groups.insert(0, gid);
        }
        self.status = trf("Raggruppati {} oggetti", &[&items.len()]);
    }

    /// Split the selected groups one level (MainWindow::on_actionUngroup_triggered 0x10067c1a0).
    pub fn ungroup(&mut self) {
        let sel = self.selection.clone();
        if !self.doc.shapes.iter().any(|s| sel.contains(&s.id) && !s.groups.is_empty()) {
            self.status = tr("Nessun gruppo selezionato.").into();
            return;
        }
        self.checkpoint_now();
        for s in self.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
            if !s.groups.is_empty() {
                s.groups.remove(0);
            }
        }
        self.status = tr("Gruppo separato").into();
    }

    /// Align to the last selected object. 0 left, 1 right, 2 top, 3 bottom,
    /// 4 horizontal centres, 5 vertical centres, 6 centres (Core_Arrange::AlignX/AlignY/AlignXY).
    pub fn align(&mut self, how: u8) {
        let items = self.selection_items();
        if items.len() < 2 {
            self.status = tr("Seleziona almeno 2 oggetti: si allineano all'ultimo selezionato.").into();
            return;
        }
        self.checkpoint_now();
        let r = self.doc.bbox_of(items.last().unwrap());
        for it in &items[..items.len() - 1] {
            let b = self.doc.bbox_of(it);
            let (dx, dy) = match how {
                0 => (r.min[0] - b.min[0], 0.0),
                1 => (r.max[0] - b.max[0], 0.0),
                2 => (0.0, r.max[1] - b.max[1]),
                3 => (0.0, r.min[1] - b.min[1]),
                4 => (r.center()[0] - b.center()[0], 0.0),
                5 => (0.0, r.center()[1] - b.center()[1]),
                _ => (r.center()[0] - b.center()[0], r.center()[1] - b.center()[1]),
            };
            self.move_ids(it, dx, dy);
        }
    }

    /// Distribute 3 or more objects (Core_Arrange::Distribute 0x1007164c8). `axis` 0 = X, 1 = Y.
    /// spaced: equal gaps between objects; otherwise equal steps between centres.
    pub fn distribute(&mut self, axis: usize, spaced: bool) {
        let items = self.selection_items();
        if items.len() < 3 {
            self.status = tr("Seleziona almeno 3 oggetti da distribuire.").into();
            return;
        }
        self.checkpoint_now();
        let mut boxes: Vec<(Vec<u64>, BBox)> = items.into_iter().map(|it| { let b = self.doc.bbox_of(&it); (it, b) }).collect();
        let n = boxes.len();
        if spaced {
            boxes.sort_by(|a, b| a.1.min[axis].partial_cmp(&b.1.min[axis]).unwrap());
            let mut all = BBox::empty();
            for (_, b) in &boxes {
                all.union(b);
            }
            let total: f64 = boxes.iter().map(|(_, b)| b.max[axis] - b.min[axis]).sum();
            let gap = ((all.max[axis] - all.min[axis]) - total) / (n - 1) as f64;
            let mut pos = boxes[0].1.max[axis] + gap;
            for (ids, b) in boxes.iter().skip(1).take(n - 2) {
                let d = pos - b.min[axis];
                if axis == 0 { self.move_ids(ids, d, 0.0) } else { self.move_ids(ids, 0.0, d) }
                pos += (b.max[axis] - b.min[axis]) + gap;
            }
        } else {
            boxes.sort_by(|a, b| a.1.center()[axis].partial_cmp(&b.1.center()[axis]).unwrap());
            let first = boxes[0].1.center()[axis];
            let step = (boxes[n - 1].1.center()[axis] - first) / (n - 1) as f64;
            for (i, (ids, b)) in boxes.iter().enumerate().skip(1).take(n - 2) {
                let d = first + step * i as f64 - b.center()[axis];
                if axis == 0 { self.move_ids(ids, d, 0.0) } else { self.move_ids(ids, 0.0, d) }
            }
        }
    }

    /// Make every selected object as wide (`axis` 0) or as tall (`axis` 1) as the last selected one
    /// (Core_Arrange::MakeSameWidth 0x100715fac / MakeSameHeight 0x100716230). Each object scales
    /// about its own centre (UndoScaleShapes 0x100119294); with Shift held the other side follows.
    pub fn make_same_size(&mut self, axis: usize, proportional: bool) {
        let items = self.selection_items();
        if items.len() < 2 {
            self.status = tr("Seleziona almeno 2 oggetti: prendono la misura dell'ultimo selezionato.").into();
            return;
        }
        let r = self.doc.bbox_of(items.last().unwrap());
        let target = if axis == 0 { r.w() } else { r.h() };
        self.checkpoint_now();
        for it in &items {
            let b = self.doc.bbox_of(it);
            let mut size = if axis == 0 { b.w() } else { b.h() };
            // a zero-size object is left as it is
            if size.abs() < SAME_SIZE_MIN {
                size = target;
            }
            let f = target / size;
            let (fx, fy) = match (axis, proportional) {
                (_, true) => (f, f),
                (0, false) => (f, 1.0),
                _ => (1.0, f),
            };
            if fx == 1.0 && fy == 1.0 {
                continue;
            }
            let c = b.center();
            for s in self.doc.shapes.iter_mut().filter(|s| it.contains(&s.id)) {
                s.scale_about(fx, fy, c);
            }
        }
        self.status = if axis == 0 { tr("Stessa larghezza").into() } else { tr("Stessa altezza").into() };
    }

    /// The outlines the Offset dialog works on: all selected objects together, or each object on
    /// its own when they overlap (OffsetDialog::DetectIntersections → Offsetter::ComputeIndividually).
    fn offset_sources(&self) -> (Vec<Vec<Polyline>>, bool) {
        let items = self.selection_items();
        let mut per_item: Vec<Vec<Polyline>> = Vec::new();
        let mut boxes: Vec<BBox> = Vec::new();
        for it in &items {
            let mut paths = Vec::new();
            for s in self.doc.shapes.iter().filter(|s| it.contains(&s.id)) {
                if !matches!(s.geom, Geometry::Image { .. }) {
                    paths.extend(s.world_paths());
                }
            }
            if !paths.is_empty() {
                boxes.push(self.doc.bbox_of(it));
                per_item.push(paths);
            }
        }
        // overlap: two objects whose boxes cross without one holding the other (estimated test)
        let mut overlap = false;
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                let (a, b) = (&boxes[i], &boxes[j]);
                if a.intersects(b) && !a.contains(b) && !b.contains(a) {
                    overlap = true;
                }
            }
        }
        if overlap {
            (per_item, true)
        } else {
            (vec![per_item.concat()], false)
        }
    }

    /// Open the Offset dialog (MainWindow::on_actionOffset_triggered 0x10067e2a4).
    pub fn open_offset(&mut self) {
        let (src, overlap) = self.offset_sources();
        if src.iter().all(|g| g.is_empty()) {
            self.status = tr("Seleziona una o più forme da cui creare l'offset.").into();
            return;
        }
        self.offset_overlap = overlap;
        self.offset_open = true;
        self.update_offset_preview();
    }

    pub fn update_offset_preview(&mut self) {
        let (src, _) = self.offset_sources();
        self.offset_preview = crate::offset::offset_all(&src, &self.offset_settings);
    }

    /// OK in the Offset dialog (MainWindow::FinalizeOffset 0x10067e348): add the results on the
    /// layer most used by the originals, then delete originals / select results if asked.
    pub fn commit_offset(&mut self) {
        self.offset_open = false;
        self.update_offset_preview();
        let results = std::mem::take(&mut self.offset_preview);
        if results.is_empty() {
            self.status = tr("L'offset non ha prodotto forme.").into();
            return;
        }
        // Offsetter::GetMostUsedCutSetting 0x10026da00
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for s in self.doc.shapes.iter().filter(|s| self.selection.contains(&s.id)) {
            *counts.entry(s.layer).or_default() += 1;
        }
        let layer = counts.into_iter().max_by_key(|(l, n)| (*n, std::cmp::Reverse(*l))).map(|(l, _)| l).unwrap_or(self.current_layer);
        self.checkpoint_now();
        let mut new_ids = Vec::new();
        for subpaths in results {
            new_ids.push(self.doc.add_shape(Shape::new(0, Geometry::Path { subpaths }, layer, 0.0, 0.0)));
        }
        if self.offset_settings.delete_originals {
            let sel = self.selection.clone();
            self.doc.shapes.retain(|s| !sel.contains(&s.id));
            self.selection.clear();
        }
        if self.offset_settings.select_results {
            self.selection = new_ids.clone();
        }
        self.clean_selection();
        self.status = trf("Offset: create {} forme", &[&new_ids.len()]);
    }

    pub fn cancel_offset(&mut self) {
        self.offset_open = false;
        self.offset_preview.clear();
    }

    /// Pack the objects edge to edge around the last selected one (Core_Arrange::MoveTogether
    /// 0x1007152b4). `axis` 0 = horizontally, 1 = vertically.
    pub fn move_together(&mut self, axis: usize) {
        let items = self.selection_items();
        if items.len() < 2 {
            self.status = tr("Seleziona almeno 2 oggetti.").into();
            return;
        }
        self.checkpoint_now();
        let r = self.doc.bbox_of(items.last().unwrap());
        let rc = r.center()[axis];
        let mut before: Vec<(Vec<u64>, BBox)> = Vec::new();
        let mut after: Vec<(Vec<u64>, BBox)> = Vec::new();
        for it in &items[..items.len() - 1] {
            let b = self.doc.bbox_of(it);
            if b.center()[axis] < rc { before.push((it.clone(), b)) } else { after.push((it.clone(), b)) }
        }
        after.sort_by(|a, b| a.1.center()[axis].partial_cmp(&b.1.center()[axis]).unwrap());
        before.sort_by(|a, b| b.1.center()[axis].partial_cmp(&a.1.center()[axis]).unwrap());
        let mut edge = r.max[axis];
        for (ids, b) in &after {
            let d = edge - b.min[axis];
            if axis == 0 { self.move_ids(ids, d, 0.0) } else { self.move_ids(ids, 0.0, d) }
            edge += b.max[axis] - b.min[axis];
        }
        let mut edge = r.min[axis];
        for (ids, b) in &before {
            let d = edge - b.max[axis];
            if axis == 0 { self.move_ids(ids, d, 0.0) } else { self.move_ids(ids, 0.0, d) }
            edge -= b.max[axis] - b.min[axis];
        }
    }

    pub fn center_on_page(&mut self) {
        let page = [self.doc.device.width / 2.0, self.doc.device.height / 2.0];
        self.transform_selection(|s, b| {
            let c = b.center();
            s.tx += page[0] - c[0];
            s.ty += page[1] - c[1];
        });
    }

    pub fn set_layer_of_selection(&mut self, layer: usize) {
        if self.selection.is_empty() {
            self.current_layer = layer;
            self.status = trf("Le nuove forme vanno sul livello {}", &[&layer_name(layer)]);
            return;
        }
        self.checkpoint_now();
        let sel = self.selection.clone();
        let mut touched: Vec<usize> = self.doc.shapes.iter().filter(|s| sel.contains(&s.id)).map(|s| s.layer).collect();
        for s in self.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
            s.layer = layer;
        }
        touched.push(layer);
        touched.sort_unstable();
        touched.dedup();
        for l in touched {
            self.doc.lock_cut_mode(l);
        }
        self.doc.touch_layer(layer);
        self.current_layer = layer;
        self.selected_row = Some(layer);
    }

    pub fn make_text_shape(&mut self, at: P2) -> u64 {
        let outline = crate::text::text_outline(&self.new_text, self.text_size, &self.text_font);
        let s = Shape::new(
            0,
            Geometry::Text { text: self.new_text.clone(), size: self.text_size, font: self.text_font.clone(), outline },
            self.current_layer,
            at[0],
            at[1],
        );
        self.checkpoint_now();
        self.doc.add_shape(s)
    }

    /// Change the language (None = follow the system), remember it and rebuild the system menu.
    /// Keep the settings from the Impostazioni window and act on the ones BeamForge uses.
    pub fn apply_prefs(&mut self, new: crate::prefs::Prefs, ctx: &egui::Context) {
        use crate::prefs::AppTheme;
        let old = std::mem::replace(&mut self.prefs, new);
        let p = &mut self.prefs;
        // theme: "Usa sfondo scuro" and "Tema dell'applicazione" both pick light or dark
        let mut light = None;
        if p.dark_background != old.dark_background {
            light = Some(!p.dark_background);
            p.theme = if p.dark_background { AppTheme::Dark } else { AppTheme::Light };
        } else if p.theme != old.theme {
            light = match p.theme {
                AppTheme::Light => Some(true),
                AppTheme::Dark => Some(false),
                AppTheme::System => ctx.system_theme().map(|t| t == egui::Theme::Light),
            };
        }
        if let Some(l) = light {
            if l != crate::theme::p().light {
                crate::theme::set_light(ctx, l);
                crate::theme::save_light(l);
            }
            p.dark_background = !l;
        }
        // text defaults for the next text
        if p.default_font != old.default_font || p.default_text_height != old.default_text_height {
            self.apply_text_defaults();
        }
        self.prefs.save();
    }

    /// "Carattere predefinito" and "Altezza predefinita" for new text.
    pub fn apply_text_defaults(&mut self) {
        if crate::text::fonts().iter().any(|f| f.name == self.prefs.default_font) {
            self.text_font = self.prefs.default_font.clone();
        }
        self.text_size = self.prefs.default_text_height;
    }

    /// "Intervallo di salvataggio automatico": every so many minutes, when there are unsaved
    /// changes, a safety copy of the project goes to autosave.beamforge in the settings folder.
    fn autosave(&mut self, ctx: &egui::Context) {
        let mins = self.prefs.autosave_minutes;
        if mins == 0 || self.screenshot.is_some() {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave < mins as f64 * 60.0 {
            return;
        }
        self.last_autosave = now;
        if self.dirty {
            crate::i18n::save_json("autosave.beamforge", &self.doc);
        }
    }

    pub fn set_language(&mut self, choice: Option<crate::i18n::Lang>, ctx: &egui::Context) {
        crate::i18n::save_choice(choice);
        crate::i18n::set(choice.unwrap_or_else(crate::i18n::system_lang));
        if self.system_menu.is_some() {
            // build the new menu first so the old one is only dropped once replaced
            let new_menu = crate::sysmenu::install(ctx);
            self.system_menu = new_menu;
        }
        self.title_cache.clear();
        self.status = tr("Pronto").into();
        ctx.request_repaint();
    }

    pub fn update_title(&mut self, ctx: &egui::Context) {
        // MainWindow::UpdateTitle (0x1006874fc): file name without extension (or "<untitled>"),
        // " *" when changed (0x102204bc9), " - " device name when there is one, " - " program name.
        // The window has no title bar now, but the title still names it in the taskbar and Dock.
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| tr("<senza titolo>").into());
        let mut t = format!("{}{}", name, if self.dirty { " *" } else { "" });
        // the running original shows "No Machine" here when no device is set (measured, MECHANICS.md round 65)
        t.push_str(" - ");
        t.push_str(if self.doc.device.name.is_empty() { tr("Nessuna macchina") } else { &self.doc.device.name });
        t.push_str(" - BeamForge");
        if t != self.title_cache {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
            self.title_cache = t;
        }
    }

    pub fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(ss) = self.screenshot.as_mut() else { return };
        ss.frames += 1;
        ctx.request_repaint();
        // BEAMFORGE_OPEN=editor,fill,switch (for testing): halfway, the layer goes back to Line,
        // to see that the editor keeps the size it had with the Fill options
        if ss.frames == 60 && std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("switch")) {
            self.doc.layers[0].mode = CutMode::Line;
        }
        // BEAMFORGE_OPEN=editor,cycle (for measuring): every 10 frames the editor shows another mode and
        // tab, with tabs on, so each state's size is seen; BEAMFORGE_DUMP_SIZES prints the sizes at the end
        if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("cycle")) && ss.frames % 10 == 0 && ss.frames < 100 {
            let k = (ss.frames / 10) as usize;
            let modes = [CutMode::Line, CutMode::Fill, CutMode::OffsetFill, CutMode::Image];
            self.doc.layers[0].mode = modes[k % 4];
            self.editor_tab = ((k / 4) % 2) as u8;
            self.doc.layers[0].tabs_enabled = true;
            self.doc.layers[0].manual_tabs = k % 2 == 1;
        }
        // BEAMFORGE_OPEN=editor,image,imgmodes (for measuring): every 9 frames the Image editor shows
        // another image mode (all twelve, 3D Slice on one pass to show its warning), so each text's size is seen
        if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("imgmodes")) && ss.frames % 9 == 0 && ss.frames < 108 {
            let k = (ss.frames / 9) as usize;
            let cs = &mut self.doc.layers[0];
            cs.image_mode = ImageMode::ALL[k % ImageMode::ALL.len()];
            cs.passes = 1;
        }
        // BEAMFORGE_OPEN=material,cycle: the test generator opens each of its three inline editors in turn
        if std::env::var("BEAMFORGE_OPEN").is_ok_and(|v| v.contains("material") && v.contains("cycle")) && ss.frames % 20 == 0 && ss.frames < 100 {
            let e = (ss.frames / 20) as usize;
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("mt_editing"), if e <= 3 { Some(e - 1) } else { None::<usize> }));
        }
        if ss.frames == 118 && std::env::var_os("BEAMFORGE_DUMP_SIZES").is_some() {
            println!("{}", crate::theme::dump_sizes());
        }
        if ss.frames >= 120 && !ss.requested {
            ss.requested = true;
            let slot = ss.image.clone();
            ctx.request_screenshot(move |img| {
                *slot.lock().unwrap() = Some(img);
            });
        }
        let got = ss.image.lock().unwrap().take();
        if let Some(img) = got {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&p.to_array());
            }
            if let Some(dir) = ss.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                Some(im) => {
                    if let Err(e) = im.save(&ss.path) {
                        eprintln!("screenshot save failed: {e}");
                    }
                }
                None => eprintln!("screenshot buffer invalid"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ss.frames > 600 {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Mouse input played back for testing: BEAMFORGE_CLICKS="frame:click:x,y;frame:drag:x1,y1,x2,y2"
/// (window points). Each step becomes the pointer events a person's hand would make.
pub fn scripted_input() -> Vec<(u32, egui::Event)> {
    let Ok(script) = std::env::var("BEAMFORGE_CLICKS") else { return Vec::new() };
    let mut out = Vec::new();
    let mv = |x: f32, y: f32| egui::Event::PointerMoved(egui::pos2(x, y));
    let btn = |x: f32, y: f32, pressed: bool| egui::Event::PointerButton { pos: egui::pos2(x, y), button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for step in script.split(';').filter(|s| !s.trim().is_empty()) {
        let parts: Vec<&str> = step.trim().split(':').collect();
        let (Some(f), Some(kind), Some(nums)) = (parts.first().and_then(|f| f.parse::<u32>().ok()), parts.get(1), parts.get(2)) else { continue };
        let n: Vec<f32> = nums.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        match (*kind, n.as_slice()) {
            ("click", [x, y]) => {
                out.push((f, mv(*x, *y)));
                out.push((f + 2, btn(*x, *y, true)));
                out.push((f + 4, btn(*x, *y, false)));
            }
            ("hold", [x, y]) => {
                out.push((f, mv(*x, *y)));
                out.push((f + 2, btn(*x, *y, true)));
                out.push((f + 45, btn(*x, *y, false)));
            }
            ("drag", [x1, y1, x2, y2]) => {
                out.push((f, mv(*x1, *y1)));
                out.push((f + 2, btn(*x1, *y1, true)));
                for k in 1..=10 {
                    let t = k as f32 / 10.0;
                    out.push((f + 2 + k, mv(x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)));
                }
                out.push((f + 14, btn(*x2, *y2, false)));
            }
            _ => {}
        }
    }
    out
}

impl eframe::App for BeamForge {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(p) = self.relaunch_on_exit.take() {
            crate::update::relaunch(&p);
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // BEAMFORGE_SHIFT=1 (for testing): Shift counts as held the whole run
        if std::env::var_os("BEAMFORGE_SHIFT").is_some() {
            raw_input.events.push(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
        }
        if self.input_script.is_empty() {
            return;
        }
        self.input_frame += 1;
        let f = self.input_frame;
        raw_input.events.extend(self.input_script.iter().filter(|(at, _)| *at == f).map(|(_, e)| e.clone()));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_laser();
        self.poll_import(&ctx);
        self.auto_connect(&ctx);
        crate::panels::window_resize_edges(&ctx);
        self.auto_update_check(&ctx);
        crate::sysmenu::handle(self, &ctx);
        self.handle_shortcuts(&ctx);
        self.update_title(&ctx);
        if self.system_menu.is_none() {
            crate::panels::menu_bar(self, ui);
        }
        crate::panels::main_toolbar(self, ui);
        crate::panels::numeric_bar(self, ui);
        crate::panels::status_bar(self, ui);
        crate::panels::palette_bar(self, ui);
        crate::panels::tool_bar(self, ui);
        crate::panels::right_panel(self, ui);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(crate::theme::p().canvas_out)).show(ui, |ui| {
            crate::canvas::canvas(self, ui);
        });
        crate::panels::windows(self, &ctx);
        crate::measure::window(self, &ctx);
        crate::lasertest_ui::windows(self, &ctx);
        self.save_prompt(&ctx);
        // dropped files
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for p in dropped {
            if crate::fileicon::is_project(&p) {
                self.open_path(p, &ctx);
            } else {
                self.import_path(&p);
            }
        }
        self.autosave(&ctx);
        self.handle_screenshot(&ctx);
        if self.connected.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

impl BeamForge {
    /// Once a day, 2 s after start, look for a newer version quietly (MainWindow::runUpdateCheck
    /// 0x10068122c). Off in --screenshot runs and when the person turned it off in Updates.
    fn auto_update_check(&mut self, ctx: &egui::Context) {
        if !self.auto_update_pending {
            return;
        }
        if ctx.input(|i| i.time) < crate::update::AUTO_CHECK_DELAY_S {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            return;
        }
        self.auto_update_pending = false;
        crate::i18n::save_json("update_check_day.json", &crate::update::today());
        self.updates.start(ctx, true);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let typing = ctx.egui_wants_keyboard_input();
        let sc = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        // With the system menu these keys belong to its items (File, Anteprima).
        let menu_keys = self.system_menu.is_none();
        if menu_keys && sc(cmd, Key::N) {
            self.new_doc(ctx);
        }
        if menu_keys && sc(cmd, Key::O) {
            self.open_dialog(ctx);
        }
        if menu_keys && sc(cmd_shift, Key::S) {
            self.save(true);
        }
        if menu_keys && sc(cmd, Key::S) {
            self.save(false);
        }
        if menu_keys && sc(cmd, Key::I) {
            self.import_dialog();
        }
        if menu_keys && sc(Modifiers::ALT, Key::P) {
            self.open_preview();
        }
        if typing {
            return;
        }
        if sc(cmd_shift, Key::Z) || sc(cmd, Key::Y) {
            self.do_redo();
        }
        if sc(cmd, Key::Z) {
            self.do_undo();
        }
        if sc(cmd, Key::C) {
            self.copy();
        }
        if sc(cmd, Key::X) {
            self.cut();
        }
        if sc(cmd, Key::V) {
            self.paste(5.0);
        }
        if sc(cmd, Key::D) {
            self.duplicate();
        }
        if sc(cmd, Key::A) {
            self.select_all();
        }
        // ⌘G / ⇧⌘G: on macOS the system menu owns these keys
        if menu_keys && sc(cmd_shift, Key::G) {
            self.ungroup();
        }
        if menu_keys && sc(cmd, Key::G) {
            self.group();
        }
        let (del, esc, enter) = ctx.input(|i| (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace), i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
        if del {
            self.delete_selection();
        }
        if esc {
            if !self.line_pts.is_empty() {
                self.line_pts.clear();
            } else if self.tool != Tool::Select {
                // Esc ends a tool; for the ruler it also closes Misurazione (MeasureTool::Escape)
                self.measure = crate::measure::MeasureState { inches: self.measure.inches, ..Default::default() };
                self.tool = Tool::Select;
            } else {
                self.selection.clear();
            }
        }
        if enter && self.line_pts.len() > 1 {
            crate::canvas::finish_line(self, false);
        }
        // arrow moves: Impostazioni > Unità e Griglie > Incrementi spostamento forma
        let (shift, cmd) = ctx.input(|i| (i.modifiers.shift, i.modifiers.command));
        let step = if shift { self.prefs.nudge_coarse } else if cmd { self.prefs.nudge_fine } else { self.prefs.nudge };
        let mut d = [0.0, 0.0];
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowLeft) {
                d[0] -= step;
            }
            if i.key_pressed(Key::ArrowRight) {
                d[0] += step;
            }
            if i.key_pressed(Key::ArrowUp) {
                d[1] += step;
            }
            if i.key_pressed(Key::ArrowDown) {
                d[1] -= step;
            }
        });
        if d != [0.0, 0.0] && !self.selection.is_empty() {
            self.transform_selection(|s, _| {
                s.tx += d[0];
                s.ty += d[1];
            });
        }
        let single = |k: Key| ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        if single(Key::Plus) || single(Key::Equals) {
            crate::canvas::zoom_relative(self, crate::canvas::ZOOM_IN_FACTOR);
        }
        if single(Key::Minus) {
            crate::canvas::zoom_relative(self, crate::canvas::ZOOM_OUT_FACTOR);
        }
    }

    pub fn open_preview(&mut self) {
        let (job, gcode) = self.job_gcode();
        self.preview = Some(Preview::new(job, gcode, None, &self.doc));
    }
}
