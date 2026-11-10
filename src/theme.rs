//! BeamForge theme: a graphite "professional tool" look with one warm laser accent.
//! Flat surfaces, hairline borders, small radii; the icons are drawn elsewhere and untouched.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const ACCENT: Color32 = Color32::from_rgb(240, 120, 40);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(150, 62, 18);
/// Window background behind the bars.
pub const BG: Color32 = Color32::from_rgb(30, 32, 36);
/// Dock and panel surfaces.
pub const PANEL: Color32 = Color32::from_rgb(37, 39, 44);
/// Sunken fields, tables and consoles.
pub const FIELD: Color32 = Color32::from_rgb(24, 25, 29);
/// Hairlines between bars and panels.
pub const LINE: Color32 = Color32::from_rgb(54, 57, 64);
pub const TEXT: Color32 = Color32::from_rgb(226, 228, 232);
pub const TEXT_DIM: Color32 = Color32::from_rgb(148, 152, 160);
pub const CANVAS_OUT: Color32 = Color32::from_rgb(26, 27, 31);
pub const CANVAS_WS: Color32 = Color32::from_rgb(44, 46, 52);
pub const GRID_MINOR: Color32 = Color32::from_rgb(52, 55, 62);
pub const GRID_MAJOR: Color32 = Color32::from_rgb(70, 74, 84);
/// Inactive and hovered button surfaces.
pub const BTN: Color32 = Color32::from_rgb(50, 53, 60);
pub const BTN_HOVER: Color32 = Color32::from_rgb(64, 68, 77);

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = Color32::from_rgb(40, 42, 48);
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = Color32::from_rgb(42, 44, 50);
    v.code_bg_color = FIELD;
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = Color32::from_rgb(190, 90, 30);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(6);
    v.window_shadow = egui::Shadow { offset: [0, 6], blur: 22, spread: 0, color: Color32::from_black_alpha(120) };
    v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(110) };
    v.menu_corner_radius = CornerRadius::same(4);
    let r = CornerRadius::same(3);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = BTN;
    v.widgets.inactive.weak_bg_fill = BTN;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(62, 66, 74));
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = BTN_HOVER;
    v.widgets.hovered.weak_bg_fill = BTN_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(96, 100, 110));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = Color32::from_rgb(200, 95, 32);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(200, 95, 32);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.bg_fill = BTN_HOVER;
    v.widgets.open.weak_bg_fill = BTN_HOVER;
    v.widgets.open.bg_stroke = Stroke::new(1.0, Color32::from_rgb(96, 100, 110));
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
    p.rect_filled(rect, 0.0, Color32::from_rgb(44, 46, 52));
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, LINE));
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, LINE));
    let stripe = egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height()));
    p.rect_filled(stripe, 0.0, ACCENT);
    p.text(
        rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::proportional(11.5),
        Color32::from_rgb(205, 208, 214),
    );
}

/// A frame for the bars around the canvas, with a hairline border.
pub fn bar_frame(fill: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, LINE)).inner_margin(egui::Margin::symmetric(8, 4))
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
