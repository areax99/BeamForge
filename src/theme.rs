//! BeamForge theme: a graphite "professional tool" look with one warm laser accent.
//! Flat surfaces, hairline borders, small radii; the icons are drawn elsewhere and untouched.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const ACCENT: Color32 = Color32::from_rgb(240, 120, 40);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(150, 62, 18);

/// Every colour that changes between the dark and the light theme.
pub struct Palette {
    /// Window background behind the bars.
    pub bg: Color32,
    /// Dock and panel surfaces.
    pub panel: Color32,
    /// Sunken fields, tables and consoles.
    pub field: Color32,
    /// Hairlines between bars and panels.
    pub line: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub canvas_out: Color32,
    pub canvas_ws: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    /// Inactive and hovered button surfaces.
    pub btn: Color32,
    pub btn_hover: Color32,
    pub btn_stroke: Color32,
    pub hover_stroke: Color32,
    pub window_fill: Color32,
    /// Alternate table rows.
    pub faint: Color32,
    pub dock_header: Color32,
    pub dock_title: Color32,
    /// Selected row in the layer list and its text.
    pub row_selected: Color32,
    pub row_selected_text: Color32,
    /// Toolbar and tool icons, and the strongest ink (hovered text, toolbar icons).
    pub icon: Color32,
    pub ink: Color32,
    /// How layer 00 (black) is drawn on the canvas.
    pub layer0: Color32,
    /// Canvas rulers: background, ticks and numbers, and the corner square.
    pub ruler_bg: Color32,
    pub ruler_tick: Color32,
    pub ruler_corner: Color32,
    /// Hovered row in the tool pop-up menus and its text: the original's QMenu rule
    /// "QMenu::item::selected { background: $Highlight$; color: $HighlightedText$; }"
    /// (Core_Style::_setStyleDefaults 0x100731784, filled in by applyThemeToQss 0x100732b44).
    pub menu_hover: Color32,
    pub menu_hover_text: Color32,
    pub light: bool,
}

/// The dark theme, BeamForge's default. Row colours: the original's Classic Dark theme
/// (Palette.Highlight Inactive #004275, HighlightedText white), a zlib Qt resource
/// :/res/themes/classic_dark.json loaded by Core_Theme(ColorScheme) 0x1007295dc.
pub static DARK: Palette = Palette {
    bg: Color32::from_rgb(30, 32, 36),
    panel: Color32::from_rgb(37, 39, 44),
    field: Color32::from_rgb(24, 25, 29),
    line: Color32::from_rgb(54, 57, 64),
    text: Color32::from_rgb(226, 228, 232),
    text_dim: Color32::from_rgb(148, 152, 160),
    canvas_out: Color32::from_rgb(26, 27, 31),
    canvas_ws: Color32::from_rgb(44, 46, 52),
    grid_minor: Color32::from_rgb(52, 55, 62),
    grid_major: Color32::from_rgb(70, 74, 84),
    btn: Color32::from_rgb(50, 53, 60),
    btn_hover: Color32::from_rgb(64, 68, 77),
    btn_stroke: Color32::from_rgb(62, 66, 74),
    hover_stroke: Color32::from_rgb(96, 100, 110),
    window_fill: Color32::from_rgb(40, 42, 48),
    faint: Color32::from_rgb(42, 44, 50),
    dock_header: Color32::from_rgb(44, 46, 52),
    dock_title: Color32::from_rgb(205, 208, 214),
    row_selected: Color32::from_rgb(0x00, 0x42, 0x75),
    row_selected_text: Color32::WHITE,
    icon: Color32::from_rgb(210, 214, 222),
    ink: Color32::WHITE,
    layer0: Color32::from_rgb(235, 236, 240),
    ruler_bg: Color32::from_rgb(30, 32, 37),
    ruler_tick: Color32::from_rgb(140, 146, 158),
    ruler_corner: Color32::from_rgb(24, 26, 30),
    menu_hover: Color32::from_rgb(52, 56, 66),
    menu_hover_text: Color32::from_rgb(226, 228, 232),
    light: false,
};

/// The light theme, from the original's Classic Light theme (:/res/themes/classic_light.json,
/// zlib Qt resource at file offset 0x1cfb3e0): Window/Button #F0F0F0, Base #FFFFFF, Text #000000,
/// disabled text #787878, Mid #A0A0A0, Midlight #E3E3E3, AlternateBase #E9E7E3, Highlight Inactive
/// #73B5E7 with HighlightedText Inactive #000000, WorkspaceBackground #BDC3C7, GridMinor #8095A5A6,
/// GridMajor #807F8C8D. The bed fill (Base), the header bar and the hover stroke are BeamForge's.
pub static LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF0, 0xF0, 0xF0),
    panel: Color32::from_rgb(0xF0, 0xF0, 0xF0),
    field: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    line: Color32::from_rgb(0xA0, 0xA0, 0xA0),
    text: Color32::from_rgb(0, 0, 0),
    text_dim: Color32::from_rgb(0x78, 0x78, 0x78),
    canvas_out: Color32::from_rgb(0xBD, 0xC3, 0xC7),
    canvas_ws: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    // the person asked for fainter small squares: the original's GridMinor colour at alpha 0x30
    // instead of 0x80; every 10th line keeps GridMajor #807F8C8D as before
    grid_minor: Color32::from_rgba_unmultiplied_const(0x95, 0xA5, 0xA6, 0x30),
    grid_major: Color32::from_rgba_unmultiplied_const(0x7F, 0x8C, 0x8D, 0x80),
    btn: Color32::from_rgb(0xF0, 0xF0, 0xF0),
    btn_hover: Color32::from_rgb(0xE3, 0xE3, 0xE3),
    btn_stroke: Color32::from_rgb(0xA0, 0xA0, 0xA0),
    hover_stroke: Color32::from_rgb(0x69, 0x69, 0x69),
    window_fill: Color32::from_rgb(0xF0, 0xF0, 0xF0),
    faint: Color32::from_rgb(0xE9, 0xE7, 0xE3),
    dock_header: Color32::from_rgb(0xE3, 0xE3, 0xE3),
    dock_title: Color32::from_rgb(0x20, 0x20, 0x20),
    row_selected: Color32::from_rgb(0x73, 0xB5, 0xE7),
    row_selected_text: Color32::from_rgb(0, 0, 0),
    icon: Color32::from_rgb(0x30, 0x32, 0x38),
    ink: Color32::from_rgb(0, 0, 0),
    layer0: Color32::from_rgb(0, 0, 0),
    ruler_bg: Color32::from_rgb(0xE3, 0xE3, 0xE3),
    ruler_tick: Color32::from_rgb(0x50, 0x50, 0x50),
    ruler_corner: Color32::from_rgb(0xD6, 0xD6, 0xD6),
    // Classic Light: Highlight Active #0078D4, HighlightedText Active #FFFFFF
    menu_hover: Color32::from_rgb(0x00, 0x78, 0xD4),
    menu_hover_text: Color32::WHITE,
    light: true,
};

static LIGHT_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The palette in use.
pub fn p() -> &'static Palette {
    if LIGHT_ON.load(std::sync::atomic::Ordering::Relaxed) { &LIGHT } else { &DARK }
}

/// The saved theme choice, like the original: one line in `<settings folder>/themes/theme.ini`
/// (LB_GetThemeFolder 0x100946680 appends "/themes" and creates it, LB_GetThemePrefFilePath 0x100946864
/// appends "/theme.ini", LB_GetCurrentThemeFilePath 0x100946900 reads the first line and deletes the file
/// when it names nothing valid). BeamForge writes `classic_dark` or `classic_light`; no file = dark.
fn theme_ini() -> Option<std::path::PathBuf> {
    crate::i18n::settings_folder().map(|d| d.join("themes").join("theme.ini"))
}

/// Whether the saved choice is the light theme (false when there is none: dark is the default).
pub fn saved_light() -> bool {
    let Some(path) = theme_ini() else { return false };
    let Ok(text) = std::fs::read_to_string(&path) else { return false };
    match text.lines().next().map(str::trim) {
        Some("classic_light") => true,
        Some("classic_dark") => false,
        _ => {
            // unknown content: forget it, as the original does with a stale theme.ini
            let _ = std::fs::remove_file(&path);
            false
        }
    }
}

/// Remember the theme choice (Preferences::SetTheme 0x10098eaf0 writes the chosen theme to theme.ini).
pub fn save_light(light: bool) {
    let Some(path) = theme_ini() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, if light { "classic_light\n" } else { "classic_dark\n" });
}

/// Switch between the light and the dark theme (dark is the default) and restyle the widgets.
pub fn set_light(ctx: &egui::Context, light: bool) {
    LIGHT_ON.store(light, std::sync::atomic::Ordering::Relaxed);
    apply(ctx);
}

pub fn apply(ctx: &egui::Context) {
    let c = p();
    let mut v = if c.light { egui::Visuals::light() } else { egui::Visuals::dark() };
    v.panel_fill = c.panel;
    v.window_fill = c.window_fill;
    v.extreme_bg_color = c.field;
    v.faint_bg_color = c.faint;
    v.code_bg_color = c.field;
    v.override_text_color = Some(c.text);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = Color32::from_rgb(190, 90, 30);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_stroke = Stroke::new(1.0, c.line);
    v.window_corner_radius = CornerRadius::same(6);
    v.window_shadow = egui::Shadow { offset: [0, 6], blur: 22, spread: 0, color: Color32::from_black_alpha(120) };
    v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(110) };
    v.menu_corner_radius = CornerRadius::same(4);
    let r = CornerRadius::same(3);
    v.widgets.noninteractive.bg_fill = c.panel;
    v.widgets.noninteractive.weak_bg_fill = c.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = c.btn;
    v.widgets.inactive.weak_bg_fill = c.btn;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, c.btn_stroke);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, c.text);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = c.btn_hover;
    v.widgets.hovered.weak_bg_fill = c.btn_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.hover_stroke);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, c.ink);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = Color32::from_rgb(200, 95, 32);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(200, 95, 32);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    // also egui's strong (bold) text colour (Visuals::strong_text_color): white on dark, black on light
    v.widgets.active.fg_stroke = Stroke::new(1.0, c.ink);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.bg_fill = c.btn_hover;
    v.widgets.open.weak_bg_fill = c.btn_hover;
    v.widgets.open.bg_stroke = Stroke::new(1.0, c.hover_stroke);
    v.widgets.open.corner_radius = r;
    v.striped = true;
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.spacing.interact_size.y = 22.0;
        s.spacing.window_margin = egui::Margin::same(12);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.indent = 16.0;
    });
}

/// Dock title: a flat bar with a thin accent stripe on the left and small capitals, the way
/// professional tools mark their panels (instead of a loud gradient).
pub fn dock_header(ui: &mut egui::Ui, title: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, crate::theme::p().dock_header);
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, crate::theme::p().line));
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, crate::theme::p().line));
    let stripe = egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height()));
    p.rect_filled(stripe, 0.0, ACCENT);
    p.text(
        rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::proportional(11.5),
        crate::theme::p().dock_title,
    );
}

/// A frame for the bars around the canvas, with a hairline border.
pub fn bar_frame(fill: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, p().line)).inner_margin(egui::Margin::symmetric(8, 4))
}

/// Centre a dialog in the app window each time it opens (not only the first time):
/// it may be dragged while open, but reopening puts it back in the middle.
///
/// The window is anchored by its top-left corner, not its centre (the person's request: a window
/// neither moves nor changes size when options inside it are switched). With a centre anchor a
/// window that grows by one row would move its title bar up by half a row; anchored at the top
/// left, the title bar stays put and any growth only goes down and to the right. The centring is
/// done once, from the window's known size, during the first frames after it opens.
pub fn centered<'a>(ctx: &egui::Context, key: &str, w: egui::Window<'a>) -> egui::Window<'a> {
    let id = egui::Id::new(("bf_dialog_shown", key));
    let opened_id = egui::Id::new(("bf_dialog_opened", key));
    let pass = ctx.cumulative_pass_nr();
    let last: Option<u64> = ctx.data(|d| d.get_temp(id));
    ctx.data_mut(|d| d.insert_temp(id, pass));
    let just_opened = last.map_or(true, |p| p + 1 < pass);
    let frame = ctx.cumulative_frame_nr();
    if just_opened {
        ctx.data_mut(|d| d.insert_temp(opened_id, frame));
    }
    let opened: u64 = ctx.data(|d| d.get_temp(opened_id)).unwrap_or(frame);
    let w = w.pivot(egui::Align2::LEFT_TOP);
    let (w, size) = steady_size(ctx, key, w);
    let area = ctx.content_rect();
    // BEAMFORGE_TRACE_WIN=1 (for testing): print where each dialog sits on every frame
    if std::env::var_os("BEAMFORGE_TRACE_WIN").is_some()
        && let Some(st) = egui::AreaState::load(ctx, egui::Id::new(("bf_win", key)))
    {
        println!("win {key} pass {pass} rect {:?} size {size:?} frame {frame} opened {opened}", st.rect());
    }
    let place = |size: egui::Vec2| {
        let size = size.min(area.size());
        let p = area.center() - size * 0.5;
        egui::pos2(p.x.max(area.min.x), p.y.max(area.min.y))
    };
    // the first frames lay the window out and learn its real size: keep it centred on that size,
    // then leave the position alone (only the person's dragging moves it). egui 0.37 puts a
    // title-bar window back where its stored state says before `current_pos` can act, so the
    // stored state itself is moved too.
    if frame <= opened + CENTRE_PASSES {
        let pos = place(size);
        let win_id = egui::Id::new(("bf_win", key));
        if let Some(mut st) = egui::AreaState::load(ctx, win_id) {
            st.pivot = egui::Align2::LEFT_TOP;
            st.pivot_pos = Some(pos);
            ctx.memory_mut(|m| m.areas_mut().set_state(egui::LayerId::new(egui::Order::Middle, win_id), st));
        }
        w.current_pos(pos)
    } else {
        w.default_pos(place(size))
    }
}

/// Frames after opening during which a dialog is re-centred on its measured size.
const CENTRE_PASSES: u64 = 3;

/// Each dialog opens at its largest size, so nothing grows when options are switched (the person's
/// request). Measured on macOS with the system font by running BeamForge through every state of
/// each window (`BEAMFORGE_OPEN=<window>,cycle BEAMFORGE_DUMP_SIZES=1 beamforge --screenshot …`,
/// MECHANICS.md round 69), outer size in points, rounded up. A larger size seen later (another
/// language, font or system) is still remembered on top of these.
const LARGEST_SIZES: &[(&str, [f32; 2])] = &[
    // all four modes × both tabs, tabs on and off
    ("cut_editor", [556.0, 749.0]),
    // Image page of the cut editor (scan switches, mode list, dither example)
    ("image_editor", [834.0, 706.0]),
    // with each of the three inline editors open
    ("material_test", [966.0, 634.0]),
    ("interval_test", [491.0, 314.0]),
    // + the overlap warning line (≈ 24 pt)
    ("offset", [418.0, 312.0]),
    ("about", [459.0, 637.0]),
    ("devices", [626.0, 553.0]),
    ("device_editor", [429.0, 420.0]),
    // a newer version with notes filling their 180 pt scroll area
    ("updates", [460.0, 480.0]),
    ("combine_assist", [640.0, 225.0]),
    ("measure", [437.0, 344.0]),
];

/// Windows whose size the person sets by dragging keep their own size.
const FREE_SIZE_WINDOWS: &[&str] = &["preview"];
/// A remembered size never takes more than this share of the app window.
const STEADY_MAX_SCREEN_SHARE: f32 = 0.9;

/// The person's rule: a dialog keeps its size when options inside it are turned on or off. Each
/// dialog remembers the largest size it has reached (window_sizes.json in the settings folder, so
/// across sessions too) and never draws smaller than that; a section that appears fills room that
/// is already there, and one that goes leaves the window as it was. `min_size` is the outer size
/// (title bar and margins included), the same thing the window's stored area size measures, so the
/// remembered size and the drawn size settle on the same value.
fn steady_size<'a>(ctx: &egui::Context, key: &str, w: egui::Window<'a>) -> (egui::Window<'a>, egui::Vec2) {
    let id = egui::Id::new(("bf_win", key));
    let w = w.id(id);
    let seen_now = egui::AreaState::load(ctx, id).and_then(|s| s.size).unwrap_or(egui::Vec2::ZERO);
    if FREE_SIZE_WINDOWS.contains(&key) {
        return (w, seen_now);
    }
    let sizes = steady_sizes();
    let mut sizes = sizes.lock().unwrap();
    let screen = ctx.content_rect().size() * STEADY_MAX_SCREEN_SHARE;
    if let Some(seen) = egui::AreaState::load(ctx, id).and_then(|s| s.size) {
        let seen = seen.min(screen);
        let old = sizes.get(key).copied().unwrap_or([0.0, 0.0]);
        let grown = [old[0].max(seen.x), old[1].max(seen.y)];
        if grown[0] > old[0] + 0.5 || grown[1] > old[1] + 0.5 {
            sizes.insert(key.to_string(), grown);
            if !STEADY_NO_SAVE.load(std::sync::atomic::Ordering::Relaxed) {
                crate::i18n::save_json("window_sizes.json", &*sizes);
            }
        }
    }
    match sizes.get(key) {
        Some(s) => {
            let min = egui::vec2(s[0], s[1]).min(screen);
            (w.min_size(min), min.max(seen_now))
        }
        None => (w, seen_now),
    }
}

/// Set for --screenshot runs: checks start from no remembered sizes and leave the person's alone.
pub static STEADY_NO_SAVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn steady_sizes() -> &'static std::sync::Mutex<std::collections::HashMap<String, [f32; 2]>> {
    static SIZES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, [f32; 2]>>> = std::sync::OnceLock::new();
    SIZES.get_or_init(|| {
        let saved: Option<std::collections::HashMap<String, [f32; 2]>> =
            if STEADY_NO_SAVE.load(std::sync::atomic::Ordering::Relaxed) || std::env::var_os("BEAMFORGE_DUMP_SIZES").is_some() {
                None
            } else {
                crate::i18n::load_json("window_sizes.json")
            };
        let mut sizes = saved.unwrap_or_default();
        // measuring runs start from nothing, so they see each window's own size
        if std::env::var_os("BEAMFORGE_DUMP_SIZES").is_none() {
            for (k, s) in LARGEST_SIZES {
                let e = sizes.entry(k.to_string()).or_insert(*s);
                *e = [e[0].max(s[0]), e[1].max(s[1])];
            }
        }
        std::sync::Mutex::new(sizes)
    })
}

/// The interface font of each system, as the person asked: San Francisco on macOS, Segoe UI on
/// Windows, the desktop's own sans-serif on Linux (whatever fontconfig picks, e.g. Ubuntu, Cantarell
/// or Noto Sans). egui's built-in fonts stay behind them for symbols the system font lacks (◆ ✓ ●).
/// A missing file is skipped, so a system without it keeps the built-in look.
pub fn use_system_fonts(ctx: &egui::Context) {
    let (sans, mono) = system_font_files();
    let mut fonts = egui::FontDefinitions::default();
    for (name, family, files) in [("system-sans", egui::FontFamily::Proportional, sans), ("system-mono", egui::FontFamily::Monospace, mono)] {
        if let Some(bytes) = files.iter().find_map(|f| std::fs::read(f).ok()) {
            fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            fonts.families.entry(family).or_default().insert(0, name.to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

fn system_font_files() -> (Vec<String>, Vec<String>) {
    if cfg!(target_os = "macos") {
        (vec!["/System/Library/Fonts/SFNS.ttf".into(), "/System/Library/Fonts/HelveticaNeue.ttc".into()], vec!["/System/Library/Fonts/SFNSMono.ttf".into(), "/System/Library/Fonts/Menlo.ttc".into()])
    } else if cfg!(windows) {
        let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        (vec![format!("{dir}\\Fonts\\segoeui.ttf"), format!("{dir}\\Fonts\\arial.ttf")], vec![format!("{dir}\\Fonts\\consola.ttf"), format!("{dir}\\Fonts\\cour.ttf")])
    } else {
        let fc = |pattern: &str| {
            std::process::Command::new("fc-match")
                .args(["-f", "%{file}", pattern])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|f| !f.is_empty())
        };
        let mut sans: Vec<String> = fc("sans-serif").into_iter().collect();
        sans.extend(["/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/noto/NotoSans-Regular.ttf"].map(String::from));
        let mut mono: Vec<String> = fc("monospace").into_iter().collect();
        mono.extend(["/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".to_string()]);
        (sans, mono)
    }
}

/// The sizes remembered so far, as JSON (for measuring the built-in starting sizes).
pub fn dump_sizes() -> String {
    serde_json::to_string(&*steady_sizes().lock().unwrap()).unwrap_or_default()
}
