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
pub fn centered<'a>(ctx: &egui::Context, key: &str, w: egui::Window<'a>) -> egui::Window<'a> {
    let id = egui::Id::new(("bf_dialog_shown", key));
    let pass = ctx.cumulative_pass_nr();
    let last: Option<u64> = ctx.data(|d| d.get_temp(id));
    ctx.data_mut(|d| d.insert_temp(id, pass));
    let w = w.pivot(egui::Align2::CENTER_CENTER);
    let just_opened = last.map_or(true, |p| p + 1 < pass);
    if just_opened {
        w.current_pos(ctx.content_rect().center())
    } else {
        w.default_pos(ctx.content_rect().center())
    }
}
