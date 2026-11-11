//! Menus, toolbars, docks and dialogs (Italian labels, like the original's Italian UI).

use crate::i18n::{tr, trf};
use crate::app::{BeamForge, RightTab, Tool};
use crate::canvas::layer_color;
use crate::gcode::StartFrom;
use crate::model::*;
use crate::theme::{self, ACCENT};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

pub fn mode_it(m: CutMode) -> &'static str {
    match m {
        CutMode::Line => tr("Linea"),
        CutMode::Fill => tr("Riempimento"),
        CutMode::OffsetFill => tr("Riempimento offset"),
        CutMode::Image => tr("Immagine"),
    }
}

fn image_mode_it(m: ImageMode) -> &'static str {
    match m {
        ImageMode::Threshold => tr("Soglia"),
        ImageMode::Dithered => tr("Retinato (Floyd-Steinberg)"),
        ImageMode::Grayscale => tr("Scala di grigi"),
    }
}

fn start_it(s: StartFrom) -> &'static str {
    match s {
        StartFrom::AbsoluteCoords => tr("Coordinate assolute"),
        StartFrom::UserOrigin => tr("Origine utente"),
        StartFrom::CurrentPosition => tr("Posizione corrente"),
    }
}

/// The drawing tools, in the order of the toolbar on the left (the shapes are its Rettangolo
/// button's menu). The Strumenti menus list exactly these, with the same names.
pub const TOOLBAR_TOOLS: [Tool; 14] = [
    Tool::Select,
    Tool::Line,
    Tool::Rect,
    Tool::Ellipse,
    Tool::Triangle,
    Tool::Pentagon,
    Tool::Polygon,
    Tool::Octagon,
    Tool::Star,
    Tool::DualStar,
    Tool::Text,
    Tool::Measure,
    Tool::PositionLaser,
    Tool::Tabs,
];

/// Menu id for a tool ("tool_<index in TOOLBAR_TOOLS>").
pub fn tool_menu_id(t: Tool) -> String {
    format!("tool_{}", TOOLBAR_TOOLS.iter().position(|x| *x == t).unwrap_or(0))
}

/// One menu row: an action id and its label, or a separator (None).
pub type MenuRow = Option<(String, &'static str)>;

/// Strumenti: the toolbar's tools, then its edit buttons, in the toolbar's order and words.
pub fn tools_menu_rows() -> Vec<MenuRow> {
    let mut v: Vec<MenuRow> = TOOLBAR_TOOLS.iter().map(|t| Some((tool_menu_id(*t), tool_it(*t)))).collect();
    v.push(None);
    for (id, l) in [
        ("offset", tr("Offset…")),
        ("combine_assist", tr("Assistente combinazione…")),
        ("mirror_h", tr("Specchia orizzontalmente")),
        ("mirror_v", tr("Specchia verticalmente")),
        ("rot_cw", tr("Ruota 90° in senso orario")),
    ] {
        v.push(Some((id.into(), l)));
    }
    v.push(None);
    for (id, l) in [("align_0", tr("Allinea a sinistra")), ("align_1", tr("Allinea a destra")), ("align_2", tr("Allinea in alto")), ("align_3", tr("Allinea in basso"))] {
        v.push(Some((id.into(), l)));
    }
    v
}

/// Strumenti laser: the toolbar's laser tool first, then the laser actions. The same rows in
/// the window's menu and the macOS menu bar.
pub fn laser_menu_rows() -> Vec<MenuRow> {
    let r = |id: &str, l: &'static str| Some((id.to_string(), l));
    vec![
        r(&tool_menu_id(Tool::PositionLaser), tool_it(Tool::PositionLaser)),
        None,
        r("preview", tr("Anteprima")),
        r("frame", tr("Inquadra")),
        r("frame_outline", tr("Inquadra forme")),
        None,
        r("interval_test", tr("Test intervallo")),
        r("material_test", tr("Test materiale")),
        None,
        r("home", tr("Home ($H)")),
        r("unlock", tr("Sblocca ($X)")),
        None,
        r("fire_on", tr("Fuoco acceso")),
        r("fire_off", tr("Fuoco spento")),
    ]
}

/// Draw a top-toolbar or left-tool button greyed out and unclickable when its action can't run now
/// (same rule as the menus: MainWindow::UpdateActionsStatus 0x100c39980 disables the shared QAction).
fn gated(app: &BeamForge, ui: &mut egui::Ui, id: &str, f: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    let on = app.can(id);
    ui.add_enabled_ui(on, |ui| {
        if !on {
            ui.multiply_opacity(DISABLED_OPACITY);
        }
        f(ui)
    })
    .inner
        && on
}

/// Laser-panel buttons stay clickable (the person's choice): the action itself says what's missing.
fn always(_app: &BeamForge, ui: &mut egui::Ui, _id: &str, f: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    f(ui)
}

/// How visible an unavailable button stays (BeamForge's choice; the original greys its Qt icons).
const DISABLED_OPACITY: f32 = 0.3;

/// Draw menu rows inside the window's menu; a click runs the same action as the menu bar.
fn menu_rows(app: &mut BeamForge, ui: &mut egui::Ui, rows: Vec<MenuRow>) {
    for row in rows {
        match row {
            None => {
                ui.separator();
            }
            Some((id, label)) => {
                let selected = id.strip_prefix("tool_").and_then(|n| n.parse::<usize>().ok()).map(|n| TOOLBAR_TOOLS[n] == app.tool);
                let enabled = app.can(&id);
                let clicked = match selected {
                    Some(s) => ui.add_enabled(enabled, egui::Button::selectable(s, label)).clicked(),
                    None => ui.add_enabled(enabled, egui::Button::new(label)).clicked(),
                };
                if clicked {
                    let ctx = ui.ctx().clone();
                    crate::sysmenu::run(app, &ctx, &id);
                    ui.close();
                }
            }
        }
    }
}

pub fn tool_it(t: Tool) -> &'static str {
    match t {
        Tool::Select => tr("Seleziona"),
        Tool::Line => tr("Disegna linee"),
        Tool::Rect => tr("Rettangolo"),
        Tool::Ellipse => tr("Ellisse"),
        Tool::Polygon => tr("Poligono"),
        Tool::Triangle => tr("Triangolo"),
        Tool::Pentagon => tr("Pentagono"),
        Tool::Octagon => tr("Ottagono"),
        Tool::Star => tr("Stella"),
        Tool::DualStar => tr("Stella doppia"),
        Tool::Text => tr("Testo"),
        Tool::Measure => tr("Misura"),
        Tool::PositionLaser => tr("Posiziona laser"),
        Tool::Tabs => tr("Linguette"),
    }
}

fn top_frame() -> egui::Frame {
    theme::bar_frame(crate::theme::p().bg)
}

// ---------------------------------------------------------------- menu

pub fn menu_bar(app: &mut BeamForge, ui: &mut egui::Ui) {
    egui::Panel::top("menu").frame(theme::bar_frame(crate::theme::p().field).inner_margin(egui::Margin::symmetric(8, 3))).show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // The app menu on Linux and Windows: the actions the original gives an app-menu role (MECHANICS.md, "Menu BeamForge").
            ui.menu_button(egui::RichText::new("BeamForge").color(ACCENT).strong().size(14.0), |ui| {
                let ctx = ui.ctx().clone();
                for row in [
                    Some(("about", tr("Informazioni su BeamForge"))),
                    None,
                    Some(("settings", tr("Impostazioni…"))),
                    None,
                    Some(("quit", tr("Esci da BeamForge"))),
                ] {
                    match row {
                        None => {
                            ui.separator();
                        }
                        Some((id, label)) => {
                            if ui.button(label).clicked() {
                                ui.close();
                                crate::sysmenu::run(app, &ctx, id);
                            }
                        }
                    }
                }
            });
            ui.add_space(8.0);
            ui.menu_button(tr("File"), |ui| {
                if ui.button(tr("Nuovo")).clicked() {
                    app.new_doc(ui.ctx());
                }
                if ui.button(tr("Apri…")).clicked() {
                    app.open_dialog(ui.ctx());
                }
                if ui.button(tr("Salva")).clicked() {
                    app.save(false);
                }
                if ui.button(tr("Salva con nome…")).clicked() {
                    app.save(true);
                }
                ui.separator();
                if ui.button(tr("Importa…")).clicked() {
                    app.import_dialog();
                }
                if ui.button(tr("Esporta G-code…")).clicked() {
                    app.save_gcode();
                }
                if ui.button(tr("Esporta SVG…")).clicked() {
                    app.export_svg();
                }
                ui.separator();
                if ui.button(tr("Esci")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(tr("Modifica"), |ui| {
                if ui.add_enabled(!app.undo.is_empty(), egui::Button::new(tr("Annulla"))).clicked() {
                    app.do_undo();
                }
                if ui.add_enabled(!app.redo.is_empty(), egui::Button::new(tr("Ripeti"))).clicked() {
                    app.do_redo();
                }
                ui.separator();
                if ui.button(tr("Taglia")).clicked() {
                    app.cut();
                }
                if ui.button(tr("Copia")).clicked() {
                    app.copy();
                }
                if ui.button(tr("Incolla")).clicked() {
                    app.paste(5.0);
                }
                if ui.button(tr("Duplica")).clicked() {
                    app.duplicate();
                }
                if ui.button(tr("Elimina")).clicked() {
                    app.delete_selection();
                }
                if ui.button(tr("Seleziona tutto")).clicked() {
                    app.select_all();
                }
                ui.separator();
                if ui.button(tr("Periferiche…")).clicked() {
                    app.open_devices();
                }
                if ui.button(tr("Impostazioni dispositivo…")).clicked() {
                    app.show_device = true;
                }
                if ui.button(tr("Impostazioni…")).clicked() {
                    app.show_settings = true;
                }
            });
            ui.menu_button(tr("Strumenti"), |ui| menu_rows(app, ui, tools_menu_rows()));
            ui.menu_button(tr("Organizza"), |ui| {
                ui.menu_button(tr("Combina forme"), |ui| {
                    for op in [crate::combine::CombineOp::Union, crate::combine::CombineOp::Subtract, crate::combine::CombineOp::Intersect] {
                        if ui.button(op.label()).clicked() {
                            app.combine_selection(op);
                            ui.close();
                        }
                    }
                    if ui.button(tr("Assistente combinazione…")).clicked() {
                        app.open_combine_assistant();
                        ui.close();
                    }
                });
                if ui.button(tr("Raggruppa (Ctrl+G)")).clicked() {
                    app.group();
                }
                if ui.button(tr("Separa (Ctrl+Shift+G)")).clicked() {
                    app.ungroup();
                }
                ui.menu_button(tr("Allinea"), |ui| {
                    for (i, l) in [tr("Allinea a sinistra"), tr("Allinea a destra"), tr("Allinea in alto"), tr("Allinea in basso"), tr("Allinea centri orizzontali"), tr("Allinea centri verticali"), tr("Allinea centri")].iter().enumerate() {
                        if ui.button(*l).clicked() {
                            app.align(i as u8);
                        }
                    }
                });
                ui.menu_button(tr("Distribuisci"), |ui| {
                    if ui.button(tr("Spaziatura orizzontale")).clicked() {
                        app.distribute(0, true);
                    }
                    if ui.button(tr("Spaziatura verticale")).clicked() {
                        app.distribute(1, true);
                    }
                    if ui.button(tr("Centri orizzontali")).clicked() {
                        app.distribute(0, false);
                    }
                    if ui.button(tr("Centri verticali")).clicked() {
                        app.distribute(1, false);
                    }
                });
                ui.menu_button(tr("Avvicina"), |ui| {
                    if ui.button(tr("Orizzontalmente")).clicked() {
                        app.move_together(0);
                    }
                    if ui.button(tr("Verticalmente")).clicked() {
                        app.move_together(1);
                    }
                });
                let shift = ui.input(|i| i.modifiers.shift);
                if ui.button(tr("Stessa larghezza")).clicked() {
                    app.make_same_size(0, shift);
                }
                if ui.button(tr("Stessa altezza")).clicked() {
                    app.make_same_size(1, shift);
                }
                ui.separator();
                if ui.button(tr("Offset…")).clicked() {
                    app.open_offset();
                }
                if ui.button(tr("Specchia orizzontalmente")).clicked() {
                    app.mirror(true);
                }
                if ui.button(tr("Specchia verticalmente")).clicked() {
                    app.mirror(false);
                }
                ui.separator();
                if ui.button(tr("Ruota 90° in senso orario")).clicked() {
                    app.rotate_sel(-90.0);
                }
                if ui.button(tr("Ruota 90° in senso antiorario")).clicked() {
                    app.rotate_sel(90.0);
                }
                ui.separator();
                for (i, l) in [tr("Allinea a sinistra"), tr("Allinea a destra"), tr("Allinea in alto"), tr("Allinea in basso"), tr("Centra orizzontalmente"), tr("Centra verticalmente")].iter().enumerate() {
                    if ui.button(*l).clicked() {
                        app.align(i as u8);
                    }
                }
                ui.separator();
                if ui.button(tr("Sposta al centro della pagina")).clicked() {
                    app.center_on_page();
                }
            });
            ui.menu_button(tr("Strumenti laser"), |ui| menu_rows(app, ui, laser_menu_rows()));
            ui.menu_button(tr("Finestra"), |ui| {
                if ui.button(tr("Adatta alla pagina")).clicked() {
                    app.view.fit_pending = true;
                }
                if ui.button(tr("Ingrandisci")).clicked() {
                    crate::canvas::zoom_relative(app, crate::canvas::ZOOM_IN_FACTOR);
                }
                if ui.button(tr("Riduci")).clicked() {
                    crate::canvas::zoom_relative(app, crate::canvas::ZOOM_OUT_FACTOR);
                }
                ui.separator();
                for (t, l) in [(RightTab::Laser, tr("Laser")), (RightTab::Move, tr("Sposta")), (RightTab::Console, tr("Console")), (RightTab::Shape, tr("Proprietà forma"))] {
                    if ui.button(l).clicked() {
                        app.right_tab = t;
                    }
                }
                ui.separator();
                if ui.button(tr("Reimposta disposizione predefinita")).clicked() {
                    app.reset_layout = true;
                }
            });
            ui.menu_button(tr("Aiuto"), |ui| {
                if ui.button(tr("Informazioni su BeamForge")).clicked() {
                    app.show_about = true;
                }
            });
            // no system title bar on Linux and Windows: the window buttons live here, and the
            // empty part of the bar moves the window
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                window_buttons(ui);
                window_drag_area(ui);
            });
        });
    });
}

/// The empty rest of a bar: drag it to move the window, double-click it to maximise or restore.
pub fn window_drag_area(ui: &mut egui::Ui) {
    let size = vec2(ui.available_width().max(0.0), ui.available_height().max(20.0));
    let (_, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
}

/// Close, maximise and minimise, drawn by BeamForge (laid out right to left).
fn window_buttons(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let ink = crate::theme::p().ink;
    let button = |ui: &mut egui::Ui, hover: Color32, tip: &str| {
        let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 24.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 3.0, hover);
        }
        (rect.center(), resp.on_hover_text(tip))
    };
    let st = Stroke::new(1.3, ink);
    let (c, r) = button(ui, Color32::from_rgb(196, 43, 28), tr("Chiudi"));
    ui.painter().line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], st);
    ui.painter().line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], st);
    if r.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let (c, r) = button(ui, crate::theme::p().btn_hover, if maximized { tr("Ripristina") } else { tr("Ingrandisci finestra") });
    ui.painter().rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, st, StrokeKind::Middle);
    if r.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    let (c, r) = button(ui, crate::theme::p().btn_hover, tr("Riduci a icona"));
    ui.painter().line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], st);
    if r.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

/// Without a system frame (Linux, Windows) the window is resized from a thin band along its edges.
pub fn window_resize_edges(ctx: &egui::Context) {
    if cfg!(target_os = "macos") || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    const BAND: f32 = 5.0;
    let screen = ctx.content_rect();
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let (l, r) = (p.x - screen.left() < BAND, screen.right() - p.x < BAND);
    let (t, b) = (p.y - screen.top() < BAND, screen.bottom() - p.y < BAND);
    use egui::viewport::ResizeDirection as D;
    let dir = match (l, r, t, b) {
        (true, _, true, _) => D::NorthWest,
        (_, true, true, _) => D::NorthEast,
        (true, _, _, true) => D::SouthWest,
        (_, true, _, true) => D::SouthEast,
        (true, ..) => D::West,
        (_, true, ..) => D::East,
        (_, _, true, _) => D::North,
        (_, _, _, true) => D::South,
        _ => return,
    };
    let icon = match dir {
        D::North | D::South => egui::CursorIcon::ResizeVertical,
        D::East | D::West => egui::CursorIcon::ResizeHorizontal,
        D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
        D::NorthEast | D::SouthWest => egui::CursorIcon::ResizeNeSw,
    };
    ctx.set_cursor_icon(icon);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
    }
}

// ---------------------------------------------------------------- main toolbar

#[derive(Clone, Copy)]
enum TbIcon {
    New,
    Open,
    Save,
    Import,
    Export,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    Fit,
    ZoomIn,
    ZoomOut,
    Preview,
    Device,
    Center,
}

const TB_ICON_STROKE: f32 = 1.4;

/// White outline icons drawn with thin strokes and filled folded corners.
fn tb(ui: &mut egui::Ui, icon: TbIcon, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 28.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 4.0, crate::theme::p().btn_hover);
    }
    let c = rect.center();
    let fg = if resp.is_pointer_button_down_on() { crate::theme::p().text_dim } else { crate::theme::p().ink };
    let st = Stroke::new(TB_ICON_STROKE, fg);
    let q = |x: f32, y: f32| pos2(c.x + x, c.y + y);
    let path = |pts: &[[f32; 2]], closed: bool| {
        let v: Vec<Pos2> = pts.iter().map(|a| q(a[0], a[1])).collect();
        if closed {
            p.add(egui::Shape::closed_line(v, st));
        } else {
            p.add(egui::Shape::line(v, st));
        }
    };
    let seg = |a: [f32; 2], b: [f32; 2]| {
        p.line_segment([q(a[0], a[1]), q(b[0], b[1])], st);
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=16).map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / 16.0;
            q(cx + r * a.cos(), cy + r * a.sin())
        }).collect()
    };
    let rrect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_stroke(Rect::from_min_max(q(x0, y0), q(x1, y1)), 2.0, st, StrokeKind::Middle);
    };
    // a page with a filled folded corner, like the attached style
    let doc = |dx: f32, dy: f32, w: f32, h: f32| {
        let f = 4.0;
        path(&[[dx, dy], [dx + w - f, dy], [dx + w, dy + f], [dx + w, dy + h], [dx, dy + h]], true);
        p.add(egui::Shape::convex_polygon(vec![q(dx + w - f, dy), q(dx + w, dy + f), q(dx + w - f, dy + f)], fg, Stroke::NONE));
    };
    let arrow_head = |tip: [f32; 2], dir: [f32; 2]| {
        let (dx, dy) = (dir[0], dir[1]);
        let (nx, ny) = (-dy, dx);
        p.add(egui::Shape::convex_polygon(
            vec![q(tip[0], tip[1]), q(tip[0] - dx * 4.5 + nx * 3.5, tip[1] - dy * 4.5 + ny * 3.5), q(tip[0] - dx * 4.5 - nx * 3.5, tip[1] - dy * 4.5 - ny * 3.5)],
            fg,
            Stroke::NONE,
        ));
    };
    use std::f32::consts::PI;
    match icon {
        TbIcon::New => {
            doc(-7.0, -9.0, 14.0, 18.0);
        }
        TbIcon::Open => {
            path(&[[-9.0, 7.0], [-9.0, -7.0], [-4.0, -7.0], [-2.0, -5.0], [7.0, -5.0], [7.0, -2.0]], false);
            path(&[[-9.0, 7.0], [-6.0, -2.0], [10.0, -2.0], [7.0, 7.0]], true);
        }
        TbIcon::Save => {
            path(&[[-8.0, -8.0], [5.0, -8.0], [8.0, -5.0], [8.0, 8.0], [-8.0, 8.0]], true);
            rrect(-4.5, -8.0, 3.5, -3.0);
            rrect(-4.5, 2.0, 4.5, 8.0);
        }
        TbIcon::Import => {
            path(&[[-9.0, 2.0], [-9.0, 8.0], [9.0, 8.0], [9.0, 2.0]], false);
            seg([0.0, -9.0], [0.0, 1.0]);
            arrow_head([0.0, 4.5], [0.0, 1.0]);
        }
        TbIcon::Export => {
            path(&[[-9.0, 2.0], [-9.0, 8.0], [9.0, 8.0], [9.0, 2.0]], false);
            seg([0.0, 4.0], [0.0, -5.0]);
            arrow_head([0.0, -9.0], [0.0, -1.0]);
        }
        TbIcon::Undo | TbIcon::Redo => {
            let m = if matches!(icon, TbIcon::Undo) { 1.0 } else { -1.0 };
            let pts: Vec<Pos2> = arc(0.0, 2.0, 7.0, PI, PI * 2.2).into_iter().map(|v| pos2(c.x + (v.x - c.x) * m, v.y)).collect();
            p.add(egui::Shape::line(pts, st));
            arrow_head([-7.0 * m, 6.0], [0.0, 1.0]);
        }
        TbIcon::Cut => {
            p.circle_stroke(q(-4.5, 6.0), 3.0, st);
            p.circle_stroke(q(4.5, 6.0), 3.0, st);
            seg([-2.8, 3.5], [5.0, -9.0]);
            seg([2.8, 3.5], [-5.0, -9.0]);
        }
        TbIcon::Copy => {
            path(&[[-3.0, 9.0], [-9.0, 9.0], [-9.0, -5.0]], false);
            doc(-4.0, -9.0, 12.0, 14.5);
        }
        TbIcon::Paste => {
            path(&[[-3.5, -7.0], [-8.0, -7.0], [-8.0, 9.0], [8.0, 9.0], [8.0, -7.0], [3.5, -7.0]], false);
            rrect(-3.5, -9.5, 3.5, -5.0);
            seg([-4.0, 0.0], [4.0, 0.0]);
            seg([-4.0, 4.5], [4.0, 4.5]);
        }
        TbIcon::Delete => {
            seg([-9.0, -6.0], [9.0, -6.0]);
            path(&[[-3.0, -6.0], [-3.0, -9.0], [3.0, -9.0], [3.0, -6.0]], false);
            path(&[[-7.0, -6.0], [-6.0, 9.0], [6.0, 9.0], [7.0, -6.0]], false);
            seg([-2.0, -2.0], [-2.0, 5.5]);
            seg([2.0, -2.0], [2.0, 5.5]);
        }
        TbIcon::Fit => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                path(&[[sx * 9.0, sy * 3.5], [sx * 9.0, sy * 9.0], [sx * 3.5, sy * 9.0]], false);
            }
            rrect(-4.0, -4.0, 4.0, 4.0);
        }
        TbIcon::ZoomIn | TbIcon::ZoomOut => {
            p.circle_stroke(q(-2.0, -2.0), 6.5, st);
            p.line_segment([q(2.8, 2.8), q(9.0, 9.0)], st);
            seg([-5.0, -2.0], [1.0, -2.0]);
            if matches!(icon, TbIcon::ZoomIn) {
                seg([-2.0, -5.0], [-2.0, 1.0]);
            }
        }
        TbIcon::Preview => {
            let top = arc(0.0, 7.0, 13.0, PI * 1.22, PI * 1.78);
            let bot = arc(0.0, -7.0, 13.0, PI * 0.22, PI * 0.78);
            p.add(egui::Shape::line(top, st));
            p.add(egui::Shape::line(bot, st));
            p.circle_filled(c, 3.2, fg);
        }
        TbIcon::Device => {
            p.circle_stroke(c, 3.0, st);
            let ring = arc(0.0, 0.0, 6.5, 0.0, PI * 2.0);
            p.add(egui::Shape::closed_line(ring, st));
            for k in 0..8 {
                let a = k as f32 * PI / 4.0;
                p.line_segment([q(6.5 * a.cos(), 6.5 * a.sin()), q(9.5 * a.cos(), 9.5 * a.sin())], st);
            }
        }
        TbIcon::Center => {
            rrect(-9.0, -9.0, 9.0, 9.0);
            p.circle_stroke(c, 3.5, st);
            seg([0.0, -9.0], [0.0, -5.5]);
            seg([0.0, 9.0], [0.0, 5.5]);
            seg([-9.0, 0.0], [-5.5, 0.0]);
            seg([9.0, 0.0], [5.5, 0.0]);
        }
    }
    resp.on_hover_text(tip).clicked()
}

/// Room left for the macOS close/minimise/zoom buttons at the toolbar's left end (estimated).
const MAC_WINDOW_BUTTONS_WIDTH: f32 = 70.0;

pub fn main_toolbar(app: &mut BeamForge, ui: &mut egui::Ui) {
    egui::Panel::top("main_toolbar").frame(top_frame()).show(ui, |ui| {
        ui.horizontal(|ui| {
            // macOS: the three window buttons sit over this bar's left end (no title bar)
            if cfg!(target_os = "macos") {
                ui.add_space(MAC_WINDOW_BUTTONS_WIDTH);
            }
            if tb(ui, TbIcon::New, tr("Nuovo progetto")) {
                app.new_doc(ui.ctx());
            }
            if tb(ui, TbIcon::Open, tr("Apri progetto")) {
                app.open_dialog(ui.ctx());
            }
            if tb(ui, TbIcon::Save, tr("Salva")) {
                app.save(false);
            }
            if tb(ui, TbIcon::Import, tr("Importa SVG o immagine")) {
                app.import_dialog();
            }
            if gated(app, ui, "export_gcode", |ui| tb(ui, TbIcon::Export, tr("Esporta G-code"))) {
                app.save_gcode();
            }
            ui.separator();
            if gated(app, ui, "undo", |ui| tb(ui, TbIcon::Undo, tr("Annulla"))) {
                app.do_undo();
            }
            if gated(app, ui, "redo", |ui| tb(ui, TbIcon::Redo, tr("Ripeti"))) {
                app.do_redo();
            }
            ui.separator();
            if gated(app, ui, "cut", |ui| tb(ui, TbIcon::Cut, tr("Taglia"))) {
                app.cut();
            }
            if gated(app, ui, "copy", |ui| tb(ui, TbIcon::Copy, tr("Copia"))) {
                app.copy();
            }
            if gated(app, ui, "paste", |ui| tb(ui, TbIcon::Paste, tr("Incolla"))) {
                app.paste(5.0);
            }
            if gated(app, ui, "delete", |ui| tb(ui, TbIcon::Delete, tr("Elimina"))) {
                app.delete_selection();
            }
            ui.separator();
            if tb(ui, TbIcon::Fit, tr("Adatta alla pagina")) {
                app.view.fit_pending = true;
            }
            if tb(ui, TbIcon::ZoomIn, tr("Ingrandisci")) {
                crate::canvas::zoom_relative(app, crate::canvas::ZOOM_IN_FACTOR);
            }
            if tb(ui, TbIcon::ZoomOut, tr("Riduci")) {
                crate::canvas::zoom_relative(app, crate::canvas::ZOOM_OUT_FACTOR);
            }
            ui.separator();
            if gated(app, ui, "preview", |ui| tb(ui, TbIcon::Preview, tr("Anteprima del lavoro (Alt+P)"))) {
                app.open_preview();
            }
            if tb(ui, TbIcon::Device, tr("Impostazioni dispositivo")) {
                app.show_device = true;
            }
            ui.separator();
            if gated(app, ui, "mirror_h", |ui| mirror_icon(ui, false, vec2(30.0, 28.0), tr("Specchia orizzontalmente"))) {
                app.mirror(true);
            }
            if gated(app, ui, "mirror_v", |ui| mirror_icon(ui, true, vec2(30.0, 28.0), tr("Specchia verticalmente"))) {
                app.mirror(false);
            }
            if gated(app, ui, "center_page", |ui| tb(ui, TbIcon::Center, tr("Centra nella pagina"))) {
                app.center_on_page();
            }
            ui.separator();
            arrange_toolbar(app, ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (txt, col) = match &app.connected {
                    Some(p) => (format!("● {}  {}", p, app.machine_state), Color32::from_rgb(90, 210, 110)),
                    None => (tr("○ Nessun laser collegato").to_string(), crate::theme::p().text_dim),
                };
                // when the toolbar is full only the status dot stays, the words move to its tooltip
                let room = ui.available_width();
                let full = egui::WidgetText::from(egui::RichText::new(&txt)).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body).size().x;
                if room >= full + 8.0 {
                    ui.label(egui::RichText::new(txt).color(col));
                } else {
                    let dot = if app.connected.is_some() { "●" } else { "○" };
                    ui.label(egui::RichText::new(dot).color(col)).on_hover_text(txt);
                }
                // macOS has no title bar to grab: the toolbar's empty part moves the window
                if cfg!(target_os = "macos") {
                    window_drag_area(ui);
                }
            });
        });
    });
}

// ---------------------------------------------------------------- numeric edits

fn anchor_grid(ui: &mut egui::Ui, value: &mut usize, id: &str) -> bool {
    let mut changed = false;
    ui.push_id(id, |ui| {
        const STEP: f32 = 16.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(STEP * 3.0 + 4.0, STEP * 3.0 + 4.0), Sense::click());
        let dot = |i: usize| pos2(rect.min.x + 2.0 + STEP * (0.5 + (i % 3) as f32), rect.min.y + 2.0 + STEP * (0.5 + (i / 3) as f32));
        // the whole cell answers a click, not only the small dot
        let cell_of = |pp: Pos2| -> usize {
            let cx = (((pp.x - rect.min.x - 2.0) / STEP).floor() as i32).clamp(0, 2) as usize;
            let cy = (((pp.y - rect.min.y - 2.0) / STEP).floor() as i32).clamp(0, 2) as usize;
            cy * 3 + cx
        };
        let hover = resp.hover_pos().map(cell_of);
        if resp.clicked() {
            if let Some(pp) = resp.interact_pointer_pos() {
                let i = cell_of(pp);
                if i != *value {
                    *value = i;
                    changed = true;
                }
            }
        }
        let p = ui.painter();
        p.rect_filled(rect, 4.0, crate::theme::p().field);
        for i in 0..9 {
            let c = dot(i);
            if i == *value {
                p.circle_filled(c, 5.0, ACCENT);
            } else if hover == Some(i) {
                p.circle_filled(c, 4.5, Color32::from_rgb(120, 70, 40));
                p.circle_stroke(c, 4.5, Stroke::new(1.0, ACCENT));
            } else {
                p.circle_stroke(c, 4.5, Stroke::new(1.0, crate::theme::p().text_dim));
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.on_hover_text(tr("Punto di riferimento: posizione, dimensioni e rotazione partono da qui"));
    });
    changed
}

/// Height of one row of the numeric bar; every group uses two rows so all labels line up.
const NB_ROW: f32 = 24.0;
const NB_GAP: f32 = 4.0;

fn nb_grid(id: &str) -> egui::Grid {
    egui::Grid::new(id).num_columns(2).spacing(vec2(6.0, NB_GAP)).min_row_height(NB_ROW)
}

fn nb_label(ui: &mut egui::Ui, text: &str) {
    ui.add_sized(vec2(0.0, NB_ROW), egui::Label::new(egui::RichText::new(text).color(crate::theme::p().text_dim)));
}

fn nb_field(ui: &mut egui::Ui, w: f32, dv: egui::DragValue<'_>) -> egui::Response {
    ui.add_sized(vec2(w, NB_ROW), dv)
}

pub fn numeric_bar(app: &mut BeamForge, ui: &mut egui::Ui) {
    let frame_nr = ui.ctx().cumulative_frame_nr();
    let bar_h = NB_ROW * 2.0 + NB_GAP;
    egui::Panel::top("numeric").frame(top_frame().fill(crate::theme::p().panel).inner_margin(egui::Margin::symmetric(10, 6))).show(ui, |ui| {
        // every group is exactly two rows tall and starts at the top, so labels and fields line up
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.spacing_mut().interact_size.y = NB_ROW;
            let b = app.doc.bbox_of(&app.selection);
            let has = b.is_valid();
            let a = if has { b.anchor(app.anchor) } else { [0.0, 0.0] };
            let (mut x, mut y, mut w, mut h) = (a[0], a[1], if has { b.w() } else { 0.0 }, if has { b.h() } else { 0.0 });
            if app.rot_for != app.selection {
                // the Ruota box holds an amount to turn by, not the shape's angle, so it is kept
                app.rot_for = app.selection.clone();
            }
            // a typed value counts on Enter / leaving the box, a dragged one when the drag ends
            let committed = |r: &egui::Response| r.drag_stopped() || (r.changed() && !r.dragged());
            ui.add_enabled_ui(has, |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                nb_grid("pos").show(ui, |ui| {
                    nb_label(ui, "PosX");
                    let rx = nb_field(ui, 92.0, crate::i18n::dv(&mut x).speed(0.1).max_decimals(3).suffix(" mm"));
                    ui.end_row();
                    nb_label(ui, "PosY");
                    let ry = nb_field(ui, 92.0, crate::i18n::dv(&mut y).speed(0.1).max_decimals(3).suffix(" mm"));
                    ui.end_row();
                    if rx.changed() || ry.changed() {
                        let (dx, dy) = (x - a[0], y - a[1]);
                        app.checkpoint(frame_nr);
                        let sel = app.selection.clone();
                        for s in app.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
                            s.tx += dx;
                            s.ty += dy;
                        }
                    }
                });
                if crate::icons::button(ui, crate::icons::Ic::Lock(app.lock_ratio), vec2(30.0, bar_h), 18.0, tr("Blocca proporzioni")) {
                    app.lock_ratio = !app.lock_ratio;
                }
                nb_grid("size").show(ui, |ui| {
                    nb_label(ui, tr("Larghezza"));
                    let rw = nb_field(ui, 92.0, crate::i18n::dv(&mut w).speed(0.1).max_decimals(3).range(0.0..=10000.0).suffix(" mm"));
                    ui.end_row();
                    nb_label(ui, tr("Altezza"));
                    let rh = nb_field(ui, 92.0, crate::i18n::dv(&mut h).speed(0.1).max_decimals(3).range(0.0..=10000.0).suffix(" mm"));
                    ui.end_row();
                    if (rw.changed() || rh.changed()) && has && w > 0.001 && h > 0.001 {
                        let mut fx = if b.w() < 1e-9 { 1.0 } else { w / b.w() };
                        let mut fy = if b.h() < 1e-9 { 1.0 } else { h / b.h() };
                        if app.lock_ratio {
                            if rw.changed() {
                                fy = fx;
                            } else {
                                fx = fy;
                            }
                        }
                        app.checkpoint(frame_nr);
                        let sel = app.selection.clone();
                        for s in app.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
                            s.scale_about(fx, fy, a);
                        }
                    }
                });
                // scale in %, about the reference point (QXY_ValueChanged 0x100c540e0)
                nb_grid("scale").show(ui, |ui| {
                    let rx = nb_field(ui, 92.0, crate::i18n::dv(&mut app.scale_pct[0]).speed(0.5).max_decimals(3).range(0.001..=100000.0).suffix(" %"));
                    ui.end_row();
                    let ry = nb_field(ui, 92.0, crate::i18n::dv(&mut app.scale_pct[1]).speed(0.5).max_decimals(3).range(0.001..=100000.0).suffix(" %"));
                    ui.end_row();
                    let (cx, cy) = (committed(&rx), committed(&ry));
                    if (cx || cy) && has {
                        let mut fx = app.scale_pct[0] / 100.0;
                        let mut fy = app.scale_pct[1] / 100.0;
                        if !cx {
                            fx = 1.0;
                        }
                        if !cy {
                            fy = 1.0;
                        }
                        if app.lock_ratio {
                            if cx {
                                fy = fx;
                            } else {
                                fx = fy;
                            }
                        }
                        if fx != 1.0 || fy != 1.0 {
                            app.checkpoint(frame_nr);
                            let sel = app.selection.clone();
                            for s in app.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
                                s.scale_about(fx, fy, a);
                            }
                        }
                        // the boxes go back to 100 % after scaling (estimated)
                        app.scale_pct = [100.0, 100.0];
                    } else if !rx.dragged() && !ry.dragged() && !rx.has_focus() && !ry.has_focus() {
                        app.scale_pct = [100.0, 100.0];
                    }
                });
                nb_grid("rot").show(ui, |ui| {
                    nb_label(ui, tr("Ruota"));
                    let r = nb_field(ui, 72.0, crate::i18n::dv(&mut app.rot_input).speed(0.5).max_decimals(2).range(-360.0..=360.0).suffix("°"));
                    // QNumEdit_ValueChanged 0x100c53f38: a non-zero value turns the selection by that
                    // many degrees around the reference point chosen in the 9-dot grid
                    if committed(&r) && app.rot_input != 0.0 && has {
                        let d = app.rot_input;
                        app.checkpoint(frame_nr);
                        let sel = app.selection.clone();
                        for s in app.doc.shapes.iter_mut().filter(|s| sel.contains(&s.id)) {
                            s.rotate_about(d, a);
                        }
                    }
                    ui.end_row();
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                });
            });
            anchor_grid(ui, &mut app.anchor, "anchor");
            ui.add(egui::Separator::default().vertical().spacing(8.0));
            // text options, like the original's text toolbar
            let mut sel_text: Option<u64> = None;
            if app.selection.len() == 1 {
                if let Some(s) = app.doc.find(app.selection[0]) {
                    if let Geometry::Text { text, size, font, .. } = &s.geom {
                        sel_text = Some(s.id);
                        if app.tool == Tool::Select && app.rot_for == app.selection && !ui.ctx().egui_wants_keyboard_input() {
                            app.new_text = text.clone();
                            app.text_size = *size;
                            app.text_font = font.clone();
                        }
                    }
                }
            }
            let mut changed = false;
            nb_grid("textopts").show(ui, |ui| {
                nb_label(ui, tr("Carattere"));
                egui::ComboBox::from_id_salt("font").width(170.0).selected_text(app.text_font.clone()).show_ui(ui, |ui| {
                    for f in crate::text::fonts() {
                        if ui.selectable_label(app.text_font == f.name, &f.name).clicked() {
                            app.text_font = f.name.clone();
                            changed = true;
                        }
                    }
                });
                ui.end_row();
                nb_label(ui, tr("Altezza"));
                changed |= nb_field(ui, 92.0, crate::i18n::dv(&mut app.text_size).speed(0.2).range(0.5..=1000.0).suffix(" mm")).changed();
                ui.end_row();
            });
            nb_grid("textopts2").show(ui, |ui| {
                nb_label(ui, tr("Testo"));
                changed |= ui.add_sized(vec2(150.0, NB_ROW), egui::TextEdit::singleline(&mut app.new_text).vertical_align(egui::Align::Center)).changed();
                ui.end_row();
                nb_label(ui, tr("Lati"));
                nb_field(ui, 60.0, crate::i18n::dv(&mut app.polygon_sides).range(3..=64)).on_hover_text(tr("Lati del poligono"));
                ui.end_row();
            });
            if changed {
                if let Some(id) = sel_text {
                    app.checkpoint(frame_nr);
                    let outline = crate::text::text_outline(&app.new_text, app.text_size, &app.text_font);
                    let (t, sz, f) = (app.new_text.clone(), app.text_size, app.text_font.clone());
                    if let Some(s) = app.doc.find_mut(id) {
                        s.geom = Geometry::Text { text: t, size: sz, font: f, outline };
                    }
                }
            }
        });
    });
}

// ---------------------------------------------------------------- status & palette

pub fn status_bar(app: &mut BeamForge, ui: &mut egui::Ui) {
    egui::Panel::bottom("status").frame(theme::bar_frame(crate::theme::p().field).inner_margin(egui::Margin::symmetric(10, 3))).show(ui, |ui| {
        ui.horizontal(|ui| {
            let pos = app.cursor_world.map(|p| format!("x: {:.2}  y: {:.2} mm", p[0], p[1])).unwrap_or_else(|| "x: –  y: – mm".into());
            ui.label(egui::RichText::new(pos).monospace().color(crate::theme::p().text_dim));
            ui.separator();
            ui.label(egui::RichText::new(trf("{} forme · {} selezionate", &[&app.doc.shapes.len(), &app.selection.len()])).color(crate::theme::p().text_dim));
            ui.separator();
            ui.label(&app.status);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(tr("BeamForge 1.0 · software libero, licenza Apache 2.0")).color(crate::theme::p().text_dim));
            });
        });
    });
}

pub fn palette_bar(app: &mut BeamForge, ui: &mut egui::Ui) {
    egui::Panel::bottom("palette").frame(theme::bar_frame(crate::theme::p().bg).inner_margin(egui::Margin::symmetric(8, 5))).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for i in 0..LAYER_COUNT {
                if i == CUT_LAYER_COUNT {
                    ui.add_space(8.0);
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 24.0), Sense::click());
                let c = layer_color(i);
                let p = ui.painter();
                p.rect_filled(rect, 3.0, c);
                if app.current_layer == i {
                    p.rect_stroke(rect.expand(1.5), 4.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Outside);
                } else if resp.hovered() {
                    p.rect_stroke(rect, 3.0, Stroke::new(1.0, ACCENT), StrokeKind::Outside);
                }
                let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
                let tc = if lum > 140.0 { Color32::BLACK } else { Color32::WHITE };
                p.text(rect.center(), egui::Align2::CENTER_CENTER, if i < CUT_LAYER_COUNT { format!("{:02}", i) } else { layer_name(i) }, egui::FontId::proportional(13.0), tc);
                let resp = resp.on_hover_text(if is_tool_layer(i) { tr("Livello strumento (non viene inciso)") } else { tr("Assegna il livello alle forme selezionate") });
                if resp.clicked() {
                    app.set_layer_of_selection(i);
                }
            }
        });
    });
}

// ---------------------------------------------------------------- left tools

fn tool_button(ui: &mut egui::Ui, t: Tool, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 34.0), Sense::click());
    let p = ui.painter();
    if active {
        p.rect_filled(rect, 6.0, Color32::from_rgb(120, 52, 18));
        p.rect_stroke(rect, 6.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, crate::theme::p().btn_hover);
    }
    let col = if active { Color32::WHITE } else { crate::theme::p().icon };
    paint_tool_icon(p, rect.center(), t, col, 1.0);
    resp.on_hover_text(tool_it(t))
}

/// The shapes button (like the original's MultiActionButton): it shows the last shape picked,
/// with a small corner mark; a click picks that shape and opens the menu of all eight.
/// How long a press on a button with a menu must last before the menu opens, seconds
/// (MultiActionButton::mousePressEvent 0x1008a2464: startTimer(250 ms) → timerEvent 0x1008a24e0 →
/// DoMenuPopup 0x1008a255c).
const MENU_HOLD_SECS: f64 = 0.250;

/// BeamForge's rule for a button with a menu, simpler than the original's at the person's request
/// (the original opens it only on a 250 ms hold or a click in its corner, MultiActionButton
/// button_clicked 0x1008a293c): any click opens or closes the menu at once, and a press held
/// MENU_HOLD_SECS opens it while still held. Returns true when a click (not a hold) happened.
fn multi_button_input(ui: &egui::Ui, resp: &egui::Response, menu_open: &mut bool, hold_opened: &mut bool) -> bool {
    if resp.is_pointer_button_down_on() && !*hold_opened {
        let held = ui.input(|i| i.pointer.press_start_time().map(|t| i.time - t)).unwrap_or(0.0);
        if held >= MENU_HOLD_SECS {
            *hold_opened = true;
            *menu_open = true;
        } else {
            // the app only redraws on input: ask for a redraw so a still, held press opens on time
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(MENU_HOLD_SECS - held));
        }
    }
    if !(resp.clicked() || resp.secondary_clicked()) {
        return false;
    }
    if !std::mem::take(hold_opened) {
        *menu_open = !*menu_open; // a release after a hold leaves the menu open
    }
    true
}

/// Row height of the tool bar's menus and their frame margin (points).
const MENU_ROW_H: f32 = 28.0;
const MENU_MARGIN: f32 = 4.0;

/// Size of a tool bar menu with `rows` rows of `row_w` points.
fn menu_size(rows: usize, row_w: f32) -> egui::Vec2 {
    vec2(row_w + 2.0 * MENU_MARGIN, rows as f32 * (MENU_ROW_H + 4.0) + 2.0 * MENU_MARGIN)
}

/// Where a button's menu opens (MultiActionButton::GetBestMenuPosition 0x1008a26e4, vertical tool
/// bar, left-to-right layout): just right of the button, its top 1 point below the button's top;
/// on the left of the button when it would pass the right edge of the window; moved up or down to
/// stay inside the window.
fn menu_position(ctx: &egui::Context, button: Rect, size: egui::Vec2) -> Pos2 {
    let screen = ctx.content_rect();
    let x = if button.right() + size.x <= screen.right() { button.right() } else { button.left() - size.x };
    let y = (button.top() + 1.0).min(screen.bottom() - size.y).max(screen.top());
    pos2(x, y)
}

fn shapes_button(app: &mut BeamForge, ui: &mut egui::Ui) {
    let shape_active = crate::app::SHAPE_TOOLS.contains(&app.tool);
    let resp = tool_button(ui, app.shape_tool, shape_active);
    let r = resp.rect;
    // corner mark: there are more shapes behind this button
    ui.painter().add(egui::Shape::convex_polygon(vec![r.right_bottom() + vec2(-3.0, -10.0), r.right_bottom() + vec2(-3.0, -3.0), r.right_bottom() + vec2(-10.0, -3.0)], ACCENT, Stroke::NONE));
    let resp = resp.on_hover_text(tr("Clicca per scegliere la forma"));
    let (mut open, mut hold) = (app.shape_menu_open, app.menu_hold_opened);
    if multi_button_input(ui, &resp, &mut open, &mut hold) {
        // the button also picks the shape it shows, ready to draw
        app.tool = app.shape_tool;
        app.line_pts.clear();
    }
    app.shape_menu_open = open;
    app.menu_hold_opened = hold;
    if !app.shape_menu_open {
        return;
    }
    let ctx = ui.ctx().clone();
    let pos = menu_position(&ctx, r, menu_size(crate::app::SHAPE_TOOLS.len(), 170.0));
    let area = egui::Area::new(egui::Id::new("shapes_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            ui.set_min_width(170.0);
            for t in crate::app::SHAPE_TOOLS {
                let (row, rr) = ui.allocate_exact_size(vec2(170.0, 28.0), Sense::click());
                let p = ui.painter();
                let ink = if t == app.tool {
                    p.rect_filled(row, 4.0, Color32::from_rgb(120, 52, 18));
                    Color32::WHITE
                } else if rr.hovered() {
                    p.rect_filled(row, 4.0, crate::theme::p().menu_hover);
                    crate::theme::p().menu_hover_text
                } else {
                    crate::theme::p().text
                };
                paint_tool_icon(p, pos2(row.left() + 16.0, row.center().y), t, ink, 0.75);
                p.text(pos2(row.left() + 34.0, row.center().y), egui::Align2::LEFT_CENTER, tool_it(t), egui::FontId::proportional(14.0), ink);
                if rr.clicked() {
                    app.tool = t;
                    app.shape_tool = t;
                    app.line_pts.clear();
                    app.shape_menu_open = false;
                }
            }
        });
    });
    // close on Esc or a click anywhere else
    // close on a press outside both the menu and its button (the menu's rows cover its area, so
    // the area itself never reports a hover: test the pointer position instead)
    let (menu_rect, button_rect) = (area.response.rect, resp.rect);
    let clicked_elsewhere = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !menu_rect.contains(p) && !button_rect.contains(p)));
    if clicked_elsewhere || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.shape_menu_open = false;
    }
}

/// The combine-objects button: a click opens its menu (Unisci, Sottrai, Interseca, Assistente),
/// like the original's MultiActionButton for these actions.
fn combine_button(app: &mut BeamForge, ui: &mut egui::Ui) {
    let on = app.can("combine");
    if !on {
        app.combine_menu_open = false;
    }
    ui.add_enabled_ui(on, |ui| {
        if !on {
            ui.multiply_opacity(DISABLED_OPACITY);
        }
        combine_button_inner(app, ui)
    });
}

fn combine_button_inner(app: &mut BeamForge, ui: &mut egui::Ui) {
    use crate::combine::{CombineOp, paint_icon};
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 34.0), Sense::click());
    let p = ui.painter();
    if app.combine_menu_open || app.combine_assist.is_some() {
        p.rect_filled(rect, 6.0, Color32::from_rgb(120, 52, 18));
        p.rect_stroke(rect, 6.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, crate::theme::p().btn_hover);
    }
    paint_icon(p, rect.center(), app.combine_current, crate::theme::p().icon, 1.1);
    p.add(egui::Shape::convex_polygon(vec![rect.right_bottom() + vec2(-3.0, -10.0), rect.right_bottom() + vec2(-3.0, -3.0), rect.right_bottom() + vec2(-10.0, -3.0)], ACCENT, Stroke::NONE));
    let resp = resp.on_hover_text(tr("Combina forme (2 oggetti selezionati): clicca per scegliere l'operazione"));
    let (mut open, mut hold) = (app.combine_menu_open, app.menu_hold_opened);
    // a click only opens the menu: the operation runs when it is picked there
    multi_button_input(ui, &resp, &mut open, &mut hold);
    app.combine_menu_open = open;
    app.menu_hold_opened = hold;
    if !app.combine_menu_open {
        return;
    }
    let ctx = ui.ctx().clone();
    let mut pick: Option<Option<CombineOp>> = None;
    let pos = menu_position(&ctx, rect, menu_size(4, 230.0));
    let area = egui::Area::new(egui::Id::new("combine_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            let rows: [(Option<CombineOp>, &str); 4] = [
                (Some(CombineOp::Union), CombineOp::Union.label()),
                (Some(CombineOp::Subtract), CombineOp::Subtract.label()),
                (Some(CombineOp::Intersect), CombineOp::Intersect.label()),
                (None, tr("Assistente combinazione…")),
            ];
            for (op, name) in rows {
                let (row, rr) = ui.allocate_exact_size(vec2(230.0, 28.0), Sense::click());
                let p = ui.painter();
                let ink = if rr.hovered() {
                    p.rect_filled(row, 4.0, crate::theme::p().menu_hover);
                    crate::theme::p().menu_hover_text
                } else {
                    crate::theme::p().text
                };
                paint_icon(p, pos2(row.left() + 16.0, row.center().y), op, ink, 0.8);
                p.text(pos2(row.left() + 34.0, row.center().y), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(14.0), ink);
                if rr.clicked() {
                    pick = Some(op);
                }
            }
        });
    });
    if let Some(op) = pick {
        // the picked operation becomes the one the button shows (SetCurrentAction 0x1008a1e3c)
        app.combine_current = op;
        app.combine_menu_open = false;
        match op {
            Some(op) => app.combine_selection(op),
            None => app.open_combine_assistant(),
        }
    }
    // close on a press outside both the menu and its button (the menu's rows cover its area, so
    // the area itself never reports a hover: test the pointer position instead)
    let (menu_rect, button_rect) = (area.response.rect, resp.rect);
    let clicked_elsewhere = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !menu_rect.contains(p) && !button_rect.contains(p)));
    if clicked_elsewhere || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.combine_menu_open = false;
    }
}

/// Regular n-gon outline oriented like the original's polygon tool (vertices at (i + 0.5)·2π/n,
/// see model::polygon_box_pts), radius `r` in screen units.
fn ngon_pts(c: Pos2, n: usize, r: f32) -> Vec<Pos2> {
    (0..n)
        .map(|i| {
            let a = (i as f32 + 0.5) * std::f32::consts::TAU / n as f32;
            pos2(c.x + r * a.sin(), c.y + r * a.cos())
        })
        .collect()
}

/// A tool's line icon centred on `c`; `k` scales it (1.0 = tool bar size).
fn paint_tool_icon(p: &egui::Painter, c: Pos2, t: Tool, col: Color32, k: f32) {
    let st = Stroke::new(1.8 * k.max(0.8), col);
    let o = |x: f32, y: f32| pos2(c.x + x * k, c.y + y * k);
    match t {
        Tool::Select => {
            let pts = vec![o(-6.0, -10.0), o(6.0, 2.0), o(1.0, 2.5), o(-6.0, 8.0)];
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
            p.line_segment([o(0.5, 2.0), o(4.0, 9.0)], Stroke::new(2.5, col));
        }
        Tool::Line => {
            p.line_segment([o(-8.0, 8.0), o(7.0, -7.0)], st);
            p.circle_filled(o(-8.0, 8.0), 2.5, ACCENT);
            p.circle_filled(o(7.0, -7.0), 2.5, ACCENT);
        }
        Tool::Rect => {
            p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0) * k), 1.0, st, StrokeKind::Middle);
        }
        Tool::Ellipse => {
            p.circle_stroke(c, 8.5 * k, st);
        }
        Tool::Polygon => {
            p.add(egui::Shape::closed_line(ngon_pts(c, 6, 9.0 * k), st));
        }
        Tool::Text => {
            p.text(c, egui::Align2::CENTER_CENTER, "A", egui::FontId::proportional(21.0 * k), col);
        }
        Tool::Measure => {
            // ruler drawn diagonally with tick marks, like the original's ruler icon
            let (a, b) = (o(-10.0, 6.0), o(6.0, -10.0));
            let n = vec2(4.0, 4.0);
            p.add(egui::Shape::closed_line(vec![a, b, b + n, a + n], st));
            for k in 1..6 {
                let t = k as f32 / 6.0;
                let q = a + (b - a) * t;
                let l = if k % 2 == 0 { 3.0 } else { 1.8 };
                p.line_segment([q, q + n * (l / 4.0)], Stroke::new(1.2, col));
            }
        }
        Tool::PositionLaser => {
            p.circle_stroke(c, 7.0 * k, st);
            p.line_segment([o(-11.0, 0.0), o(11.0, 0.0)], Stroke::new(1.2, col));
            p.line_segment([o(0.0, -11.0), o(0.0, 11.0)], Stroke::new(1.2, col));
            p.circle_filled(c, 2.0 * k, Color32::from_rgb(90, 210, 110));
        }
        Tool::Tabs => {
            p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0) * k), 1.0, st, StrokeKind::Middle);
            p.line_segment([o(-3.0, -7.0), o(3.0, -7.0)], Stroke::new(3.0, crate::theme::p().canvas_out));
            p.circle_filled(o(0.0, -7.0), 2.2, ACCENT);
        }
        Tool::Triangle | Tool::Pentagon | Tool::Octagon => {
            let n = t.fixed_sides().unwrap_or(3) as usize;
            let r = if n == 3 { 10.0 } else { 9.0 } * k;
            let cc = if n == 3 { c + vec2(0.0, 2.0 * k) } else { c };
            p.add(egui::Shape::closed_line(ngon_pts(cc, n, r), st));
        }
        Tool::Star | Tool::DualStar => {
            let middle = (t == Tool::DualStar).then_some(DUAL_STAR_MIDDLE);
            let pts: Vec<Pos2> = star_pts(STAR_POINTS, 10.0, 10.0, STAR_INNER, middle).into_iter().map(|q| o(q[0] as f32, -q[1] as f32 + 0.5)).collect();
            p.add(egui::Shape::closed_line(pts, st));
        }
    }
}

/// Offset tool button: a dashed shape with a rounded outline around it.
fn offset_icon(ui: &mut egui::Ui, active: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 34.0), Sense::click());
    let p = ui.painter();
    if active {
        p.rect_filled(rect, 6.0, Color32::from_rgb(120, 52, 18));
        p.rect_stroke(rect, 6.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, crate::theme::p().btn_hover);
    }
    let c = rect.center();
    let col = if active { Color32::WHITE } else { crate::theme::p().icon };
    p.rect_stroke(Rect::from_center_size(c, vec2(22.0, 18.0)), 5.0, Stroke::new(1.6, col), StrokeKind::Middle);
    let inner = Rect::from_center_size(c, vec2(12.0, 8.0));
    p.extend(egui::Shape::dashed_line(&[inner.left_top(), inner.right_top(), inner.right_bottom(), inner.left_bottom(), inner.left_top()], Stroke::new(1.3, ACCENT), 2.0, 1.5));
    resp.on_hover_text(tip).clicked()
}

fn small_icon(ui: &mut egui::Ui, icon: &str, tip: &str) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(icon).size(16.0)).min_size(vec2(36.0, 30.0)).frame_when_inactive(false)).on_hover_text(tip).clicked()
}

/// Mirror icon drawn with shapes: two triangles facing a dashed axis (no emoji font needed).
#[derive(Clone, Copy)]
enum ArrangeIcon {
    Group,
    Ungroup,
    Align(u8),
    Distribute(usize, bool),
    Together(usize),
    SameSize(usize),
}

/// Arrange buttons drawn with shapes (the original's "Arrange" toolbar).
fn arrange_icon(ui: &mut egui::Ui, kind: ArrangeIcon, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 5.0, crate::theme::p().btn_hover);
    }
    let c = rect.center();
    let fg = crate::theme::p().icon;
    let st = Stroke::new(1.4, fg);
    let guide = Stroke::new(1.4, ACCENT);
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(c.x + x0, c.y + y0), pos2(c.x + x1, c.y + y1));
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_filled(r(x0, y0, x1, y1), 1.0, fg);
    };
    let line = |a: [f32; 2], b: [f32; 2], s: Stroke| {
        p.line_segment([pos2(c.x + a[0], c.y + a[1]), pos2(c.x + b[0], c.y + b[1])], s);
    };
    match kind {
        ArrangeIcon::Group => {
            p.rect_stroke(r(-8.0, -8.0, 1.0, 1.0), 1.0, st, StrokeKind::Middle);
            p.rect_stroke(r(-1.0, -1.0, 8.0, 8.0), 1.0, st, StrokeKind::Middle);
            p.extend(egui::Shape::dashed_line(&[pos2(c.x - 10.0, c.y - 10.0), pos2(c.x + 10.0, c.y - 10.0), pos2(c.x + 10.0, c.y + 10.0), pos2(c.x - 10.0, c.y + 10.0), pos2(c.x - 10.0, c.y - 10.0)], guide, 2.0, 2.0));
        }
        ArrangeIcon::Ungroup => {
            p.rect_stroke(r(-10.0, -10.0, -2.0, -2.0), 1.0, st, StrokeKind::Middle);
            p.rect_stroke(r(2.0, 2.0, 10.0, 10.0), 1.0, st, StrokeKind::Middle);
            line([-2.0, 2.0], [2.0, -2.0], guide);
        }
        ArrangeIcon::Align(how) => match how {
            0 => {
                line([-9.0, -10.0], [-9.0, 10.0], guide);
                bar(-7.0, -7.0, 8.0, -2.0);
                bar(-7.0, 2.0, 3.0, 7.0);
            }
            1 => {
                line([9.0, -10.0], [9.0, 10.0], guide);
                bar(-8.0, -7.0, 7.0, -2.0);
                bar(-3.0, 2.0, 7.0, 7.0);
            }
            2 => {
                line([-10.0, -9.0], [10.0, -9.0], guide);
                bar(-7.0, -7.0, -2.0, 8.0);
                bar(2.0, -7.0, 7.0, 3.0);
            }
            3 => {
                line([-10.0, 9.0], [10.0, 9.0], guide);
                bar(-7.0, -8.0, -2.0, 7.0);
                bar(2.0, -3.0, 7.0, 7.0);
            }
            4 => {
                line([0.0, -10.0], [0.0, 10.0], guide);
                bar(-8.0, -7.0, 8.0, -2.0);
                bar(-5.0, 2.0, 5.0, 7.0);
            }
            5 => {
                line([-10.0, 0.0], [10.0, 0.0], guide);
                bar(-7.0, -8.0, -2.0, 8.0);
                bar(2.0, -5.0, 7.0, 5.0);
            }
            _ => {
                p.rect_stroke(r(-8.0, -8.0, 8.0, 8.0), 1.0, st, StrokeKind::Middle);
                p.rect_stroke(r(-4.0, -4.0, 4.0, 4.0), 1.0, st, StrokeKind::Middle);
                line([0.0, -11.0], [0.0, 11.0], guide);
                line([-11.0, 0.0], [11.0, 0.0], guide);
            }
        },
        ArrangeIcon::Distribute(axis, spaced) => {
            for k in [-1.0f32, 0.0, 1.0] {
                let o = k * 7.0;
                let (w, h) = if spaced { (2.5, 6.0) } else { (1.5, 7.0) };
                if axis == 0 {
                    bar(o - w, -h, o + w, h);
                } else {
                    bar(-h, o - w, h, o + w);
                }
            }
            if axis == 0 {
                line([-10.0, 10.0], [10.0, 10.0], guide);
            } else {
                line([10.0, -10.0], [10.0, 10.0], guide);
            }
        }
        ArrangeIcon::Together(axis) => {
            if axis == 0 {
                bar(-9.0, -5.0, -1.0, 5.0);
                bar(1.0, -5.0, 9.0, 5.0);
                line([-11.0, 8.0], [-3.0, 8.0], guide);
                line([11.0, 8.0], [3.0, 8.0], guide);
            } else {
                bar(-5.0, -9.0, 5.0, -1.0);
                bar(-5.0, 1.0, 5.0, 9.0);
                line([8.0, -11.0], [8.0, -3.0], guide);
                line([8.0, 11.0], [8.0, 3.0], guide);
            }
        }
        ArrangeIcon::SameSize(axis) => {
            // two objects of different size with an orange measure across the matching side
            if axis == 0 {
                p.rect_stroke(r(-9.0, -9.0, 9.0, -3.0), 1.0, st, StrokeKind::Inside);
                p.rect_stroke(r(-9.0, 0.0, 9.0, 9.0), 1.0, st, StrokeKind::Inside);
                line([-11.0, -11.0], [-11.0, 11.0], guide);
                line([11.0, -11.0], [11.0, 11.0], guide);
            } else {
                p.rect_stroke(r(-9.0, -9.0, -3.0, 9.0), 1.0, st, StrokeKind::Inside);
                p.rect_stroke(r(0.0, -9.0, 9.0, 9.0), 1.0, st, StrokeKind::Inside);
                line([-11.0, -11.0], [11.0, -11.0], guide);
                line([-11.0, 11.0], [11.0, 11.0], guide);
            }
        }
    }
    resp.on_hover_text(tip).clicked()
}

fn arrange_toolbar(app: &mut BeamForge, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = 2.0;
    if gated(app, ui, "group", |ui| arrange_icon(ui, ArrangeIcon::Group, tr("Raggruppa (⌘G)"))) {
        app.group();
    }
    if gated(app, ui, "ungroup", |ui| arrange_icon(ui, ArrangeIcon::Ungroup, tr("Separa (⇧⌘G)"))) {
        app.ungroup();
    }
    ui.separator();
    let aligns = [
        (0u8, tr("Allinea a sinistra (all'ultimo selezionato)")),
        (4, tr("Allinea centri orizzontali")),
        (1, tr("Allinea a destra")),
        (2, tr("Allinea in alto")),
        (5, tr("Allinea centri verticali")),
        (3, tr("Allinea in basso")),
        (6, tr("Allinea centri")),
    ];
    for (how, tip) in aligns {
        if gated(app, ui, "align_0", |ui| arrange_icon(ui, ArrangeIcon::Align(how), tip)) {
            app.align(how);
        }
    }
    ui.separator();
    if gated(app, ui, "dist_h_spaced", |ui| arrange_icon(ui, ArrangeIcon::Distribute(0, true), tr("Distribuisci spaziatura orizzontale"))) {
        app.distribute(0, true);
    }
    if gated(app, ui, "dist_v_spaced", |ui| arrange_icon(ui, ArrangeIcon::Distribute(1, true), tr("Distribuisci spaziatura verticale"))) {
        app.distribute(1, true);
    }
    if gated(app, ui, "dist_h_centered", |ui| arrange_icon(ui, ArrangeIcon::Distribute(0, false), tr("Distribuisci centri orizzontali"))) {
        app.distribute(0, false);
    }
    if gated(app, ui, "dist_v_centered", |ui| arrange_icon(ui, ArrangeIcon::Distribute(1, false), tr("Distribuisci centri verticali"))) {
        app.distribute(1, false);
    }
    ui.separator();
    if gated(app, ui, "together_h", |ui| arrange_icon(ui, ArrangeIcon::Together(0), tr("Avvicina orizzontalmente"))) {
        app.move_together(0);
    }
    if gated(app, ui, "together_v", |ui| arrange_icon(ui, ArrangeIcon::Together(1), tr("Avvicina verticalmente"))) {
        app.move_together(1);
    }
    ui.separator();
    // Shift keeps the proportions (queryKeyboardModifiers & ShiftModifier in 0x100715fac)
    let shift = ui.input(|i| i.modifiers.shift);
    if gated(app, ui, "same_w", |ui| arrange_icon(ui, ArrangeIcon::SameSize(0), tr("Stessa larghezza (dell'ultimo selezionato; Maiusc: in proporzione)"))) {
        app.make_same_size(0, shift);
    }
    if gated(app, ui, "same_h", |ui| arrange_icon(ui, ArrangeIcon::SameSize(1), tr("Stessa altezza (dell'ultimo selezionato; Maiusc: in proporzione)"))) {
        app.make_same_size(1, shift);
    }
}

fn mirror_icon(ui: &mut egui::Ui, vertical: bool, size: egui::Vec2, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 4.0, crate::theme::p().btn_hover);
    }
    let c = rect.center();
    let col = crate::theme::p().icon;
    let tri = |a: [f32; 2], b: [f32; 2], d: [f32; 2], fill: Color32| {
        let m = |q: [f32; 2]| if vertical { pos2(c.x + q[1], c.y + q[0]) } else { pos2(c.x + q[0], c.y + q[1]) };
        egui::Shape::convex_polygon(vec![m(a), m(b), m(d)], fill, Stroke::new(1.2, col))
    };
    p.add(tri([-2.5, -7.0], [-2.5, 7.0], [-10.0, 7.0], col));
    p.add(tri([2.5, -7.0], [2.5, 7.0], [10.0, 7.0], Color32::TRANSPARENT));
    let (a, b) = if vertical { (pos2(c.x - 10.0, c.y), pos2(c.x + 10.0, c.y)) } else { (pos2(c.x, c.y - 10.0), pos2(c.x, c.y + 10.0)) };
    p.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(1.0, ACCENT), 2.0, 2.0));
    resp.on_hover_text(tip).clicked()
}

pub fn tool_bar(app: &mut BeamForge, ui: &mut egui::Ui) {
    egui::Panel::left("tools").resizable(false).exact_size(54.0).frame(theme::bar_frame(crate::theme::p().bg).inner_margin(egui::Margin::symmetric(8, 8))).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for t in [Tool::Select, Tool::Line, Tool::Rect, Tool::Text, Tool::Measure, Tool::PositionLaser, Tool::Tabs] {
                if t == Tool::Rect {
                    shapes_button(app, ui);
                    continue;
                }
                if tool_button(ui, t, app.tool == t).clicked() {
                    app.tool = t;
                    app.line_pts.clear();
                }
            }
            ui.separator();
            let offset_open = app.offset_open;
            if gated(app, ui, "offset", |ui| offset_icon(ui, offset_open, tr("Offset (crea un contorno a distanza dalle forme selezionate)"))) {
                app.open_offset();
            }
            combine_button(app, ui);
            if gated(app, ui, "mirror_h", |ui| mirror_icon(ui, false, vec2(36.0, 30.0), tr("Specchia orizzontalmente"))) {
                app.mirror(true);
            }
            if gated(app, ui, "mirror_v", |ui| mirror_icon(ui, true, vec2(36.0, 30.0), tr("Specchia verticalmente"))) {
                app.mirror(false);
            }
            if gated(app, ui, "rot_cw", |ui| small_icon(ui, "⟳", tr("Ruota 90° in senso orario"))) {
                app.rotate_sel(-90.0);
            }
            ui.separator();
            if gated(app, ui, "align_0", |ui| small_icon(ui, "⇤", tr("Allinea a sinistra"))) {
                app.align(0);
            }
            if gated(app, ui, "align_1", |ui| small_icon(ui, "⇥", tr("Allinea a destra"))) {
                app.align(1);
            }
            if gated(app, ui, "align_2", |ui| small_icon(ui, "⏶", tr("Allinea in alto"))) {
                app.align(2);
            }
            if gated(app, ui, "align_3", |ui| small_icon(ui, "⏷", tr("Allinea in basso"))) {
                app.align(3);
            }
        });
    });
}

// ---------------------------------------------------------------- right docks

/// Third-party components shown on the About page with their licences (from `cargo metadata`).
const THIRD_PARTY: &[(&str, &str, &str)] = &[
    ("egui / eframe", "MIT OR Apache-2.0", "https://github.com/emilk/egui"),
    ("resvg / usvg", "Apache-2.0 OR MIT", "https://github.com/linebender/resvg"),
    ("image", "MIT OR Apache-2.0", "https://github.com/image-rs/image"),
    ("serialport", "MPL-2.0", "https://github.com/serialport/serialport-rs"),
    ("ttf-parser", "MIT OR Apache-2.0", "https://github.com/harfbuzz/ttf-parser"),
    ("iOverlay", "MIT OR Apache-2.0", "https://github.com/iShape-Rust/iOverlay"),
    ("roxmltree", "MIT OR Apache-2.0", "https://github.com/RazrFalcon/roxmltree"),
    ("flate2", "MIT OR Apache-2.0", "https://github.com/rust-lang/flate2-rs"),
    ("zip", "MIT", "https://github.com/zip-rs/zip2"),
    ("muda", "Apache-2.0 OR MIT", "https://github.com/tauri-apps/muda"),
    ("rfd", "MIT", "https://github.com/PolyMeilex/rfd"),
    ("serde / serde_json", "MIT OR Apache-2.0", "https://github.com/serde-rs/serde"),
    ("sys-locale", "MIT OR Apache-2.0", "https://github.com/1Password/sys-locale"),
    ("epaint default fonts", "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0", "https://github.com/emilk/egui"),
];

/// Right options panel width (px): drag its left edge between these limits.
pub const RIGHT_WIDTH_DEFAULT: f32 = 512.0;
pub const RIGHT_WIDTH_MIN: f32 = 380.0;
pub const RIGHT_WIDTH_MAX: f32 = 1100.0;

pub fn right_panel(app: &mut BeamForge, ui: &mut egui::Ui) {
    let max_w = RIGHT_WIDTH_MAX.min((ui.available_width() - 260.0).max(RIGHT_WIDTH_MIN));
    let panel = egui::Panel::right("docks").resizable(true);
    // Reset to Default Layout (ResetWindowLayout 0x100c43d24 → ResetRightDockLayout 0x100c3f600
    // restores the dock state saved at start-up): force the default width for one frame.
    let panel = if std::mem::take(&mut app.reset_layout) {
        app.right_tab = RightTab::Laser;
        panel.exact_size(RIGHT_WIDTH_DEFAULT.min(max_w))
    } else {
        panel.default_size(app.right_width.min(max_w)).size_range(RIGHT_WIDTH_MIN..=max_w)
    };
    let resp = panel
        .frame(egui::Frame::new().fill(crate::theme::p().panel).stroke(Stroke::new(1.0, crate::theme::p().line)))
        .show(ui, |ui| {
        cuts_dock(app, ui);
        ui.add_space(6.0);
        theme::dock_header(ui, match app.right_tab {
            RightTab::Laser => tr("Laser"),
            RightTab::Move => tr("Sposta"),
            RightTab::Console => tr("Console"),
            RightTab::Shape => tr("Proprietà forma"),
        });
        egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (t, l) in [(RightTab::Laser, tr("Laser")), (RightTab::Move, tr("Sposta")), (RightTab::Console, tr("Console")), (RightTab::Shape, tr("Forma"))] {
                    ui.selectable_value(&mut app.right_tab, t, l);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("dock_scroll").show(ui, |ui| match app.right_tab {
                RightTab::Laser => laser_tab(app, ui),
                RightTab::Move => move_tab(app, ui),
                RightTab::Console => console_tab(app, ui),
                RightTab::Shape => shape_tab(app, ui),
            });
        });
    });
    // remember the width once the person lets go of the edge
    let w = resp.response.rect.width();
    if (w - app.right_width).abs() > 0.5 && !ui.ctx().input(|i| i.pointer.any_down()) {
        app.right_width = w;
        crate::i18n::save_json("panel_width.json", &w);
    }
}

fn cuts_dock(app: &mut BeamForge, ui: &mut egui::Ui) {
    theme::dock_header(ui, tr("Tagli / Livelli"));
    let frame_nr = ui.ctx().cumulative_frame_nr();
    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
        let used = app.doc.used_layers();
        // the person's request: when the canvas selection changes, select the row of the last selected
        // object's layer (the original only keeps the previous row: UpdateCutList 0x100c993a8)
        if app.selection != app.row_follows {
            app.row_follows = app.selection.clone();
            if let Some(layer) = app.selection.last().and_then(|id| app.doc.shapes.iter().find(|s| s.id == *id)).map(|s| s.layer) {
                app.selected_row = Some(layer);
            }
        }
        if app.selected_row.is_none_or(|r| !used.contains(&r)) {
            app.selected_row = used.first().copied();
        }
        let sel_grid_row = app.selected_row.and_then(|r| used.iter().position(|&l| l == r)).map(|i| i + 1);
        ui.horizontal(|ui| {
            let table_w = (ui.available_width() - 56.0).max(200.0);
            egui::Frame::new().fill(crate::theme::p().field).corner_radius(4).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_width(table_w - 8.0);
                ui.set_max_width(table_w - 8.0);
                ui.set_min_height(190.0);
                ui.vertical(|ui| egui::ScrollArea::both().id_salt("cuts").max_height(190.0).max_width(table_w - 8.0).auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("cuts_grid")
                        .num_columns(7)
                        .with_row_color(move |row, style| {
                            if Some(row) == sel_grid_row {
                                Some(crate::theme::p().row_selected)
                            } else if row % 2 == 1 {
                                Some(style.visuals.faint_bg_color)
                            } else {
                                None
                            }
                        })
                        .spacing(vec2(12.0, 4.0))
                        .show(ui, |ui| {
                        for h in ["#", tr("Livello"), tr("Modalità"), tr("Vel./Pot. (mm/min / %)"), tr("Risultato"), tr("Mostra"), tr("Aria")] {
                            ui.label(egui::RichText::new(h).color(crate::theme::p().text_dim).strong());
                        }
                        ui.end_row();
                        for &l in &used {
                            let selected = app.selected_row == Some(l);
                            // the whole row is tinted (ROW_SELECTED) instead of an orange box per cell
                            let cell = |ui: &mut egui::Ui, text: String| {
                                let t = egui::RichText::new(text).color(if selected { crate::theme::p().row_selected_text } else { crate::theme::p().text });
                                ui.add(egui::Label::new(t).selectable(false).sense(Sense::click()))
                            };
                            let r1 = cell(ui, layer_name(l).to_string());
                            let (rect, r2) = ui.allocate_exact_size(vec2(46.0, 16.0), Sense::click());
                            ui.painter().rect_filled(rect, 3.0, layer_color(l));
                            let cs = &app.doc.layers[l];
                            let r3 = cell(ui, mode_it(cs.mode).to_string());
                            let r4 = cell(ui, format!("{:.0} / {:.0}", cs.speed, cs.max_power));
                            let (mut o, mut s, mut a) = (cs.output, cs.show, cs.air);
                            let mut ch = false;
                            ch |= ui.checkbox(&mut o, "").changed();
                            ch |= ui.checkbox(&mut s, "").changed();
                            ch |= ui.checkbox(&mut a, "").changed();
                            ui.end_row();
                            if ch {
                                app.checkpoint_now();
                                let cs = &mut app.doc.layers[l];
                                cs.output = o;
                                cs.show = s;
                                cs.air = a;
                            }
                            for r in [&r1, &r2, &r3, &r4] {
                                if r.clicked() {
                                    app.selected_row = Some(l);
                                }
                                if r.double_clicked() {
                                    app.editor_layer = Some(l);
                                }
                            }
                        }
                    });
                    if used.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(tr("Disegna o importa qualcosa:\ni livelli compaiono qui.")).color(crate::theme::p().text_dim));
                        });
                    }
                }));
            });
            ui.vertical(|ui| {
                if crate::icons::button(ui, crate::icons::Ic::Arrow(0, -1), vec2(32.0, 26.0), 14.0, tr("Sposta su il livello selezionato")) {
                    move_layer(app, -1);
                }
                if crate::icons::button(ui, crate::icons::Ic::Arrow(0, 1), vec2(32.0, 26.0), 14.0, tr("Sposta giù il livello selezionato")) {
                    move_layer(app, 1);
                }
                ui.add_space(10.0);
                if crate::icons::button(ui, crate::icons::Ic::Trash, vec2(32.0, 26.0), 15.0, tr("Elimina il livello selezionato e il suo contenuto")) {
                    if let Some(l) = app.selected_row {
                        app.checkpoint_now();
                        app.doc.shapes.retain(|s| s.layer != l);
                        app.selection.clear();
                    }
                }
                ui.add_space(10.0);
                if crate::icons::button(ui, crate::icons::Ic::Pencil, vec2(32.0, 26.0), 15.0, tr("Modifica impostazioni di taglio")) {
                    app.editor_layer = app.selected_row;
                }
            });
        });
        ui.add_space(6.0);
        let Some(l) = app.selected_row else {
            quick_edit_disabled(ui);
            return;
        };
        let mut cs = app.doc.layers[l].clone();
        let mut changed = false;
        egui::Grid::new("quick").num_columns(4).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
            ui.label(tr("Colore livello"));
            let (rect, _) = ui.allocate_exact_size(vec2(90.0, 18.0), Sense::hover());
            ui.painter().rect_filled(rect, 3.0, layer_color(l));
            ui.label(tr("Velocità (mm/min)"));
            changed |= ui.add(crate::i18n::dv(&mut cs.speed).speed(10.0).range(0.1..=60000.0).max_decimals(2)).changed();
            ui.end_row();
            ui.label(tr("Numero passaggi"));
            changed |= ui.add(crate::i18n::dv(&mut cs.passes).range(1..=100)).changed();
            ui.label(tr("Potenza massima (%)"));
            changed |= ui.add(crate::i18n::dv(&mut cs.max_power).speed(0.5).range(0.0..=100.0).max_decimals(2)).changed();
            ui.end_row();
            ui.label(tr("Intervallo (mm)"));
            changed |= ui.add_enabled(cs.mode != CutMode::Line, crate::i18n::dv(&mut cs.interval).speed(0.005).range(0.01..=5.0).max_decimals(3)).changed();
            let min_on = app.doc.device.supports_min_power();
            if !min_on {
                cs.min_power = cs.max_power;
            }
            ui.add_enabled(min_on, egui::Label::new(tr("Potenza min. (%)")));
            changed |= ui.add_enabled(min_on, crate::i18n::dv(&mut cs.min_power).speed(0.5).range(0.0..=100.0).max_decimals(2)).changed();
            ui.end_row();
            ui.label(tr("Modalità"));
            egui::ComboBox::from_id_salt("quick_mode").selected_text(mode_it(cs.mode)).show_ui(ui, |ui| {
                for m in [CutMode::Line, CutMode::Fill, CutMode::OffsetFill, CutMode::Image] {
                    changed |= ui.selectable_value(&mut cs.mode, m, mode_it(m)).changed();
                }
            });
            ui.end_row();
        });
        if ui.button(tr("Altre impostazioni…")).clicked() {
            app.editor_layer = Some(l);
        }
        if changed {
            if cs.min_power > cs.max_power {
                cs.min_power = cs.max_power;
            }
            app.checkpoint(frame_nr);
            app.doc.layers[l] = cs;
        }
    });
}

fn quick_edit_disabled(ui: &mut egui::Ui) {
    ui.add_enabled_ui(false, |ui| {
        egui::Grid::new("quick_off").num_columns(4).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
            let mut z = 0.0;
            let mut one = 1u32;
            let mut iv = 0.1;
            ui.label(tr("Colore livello"));
            ui.label("");
            ui.label(tr("Velocità (mm/min)"));
            ui.add(crate::i18n::dv(&mut z));
            ui.end_row();
            ui.label(tr("Numero passaggi"));
            ui.add(crate::i18n::dv(&mut one));
            ui.label(tr("Potenza massima (%)"));
            ui.add(crate::i18n::dv(&mut z));
            ui.end_row();
            ui.label(tr("Intervallo (mm)"));
            ui.add(crate::i18n::dv(&mut iv));
            ui.label(tr("Potenza min. (%)"));
            ui.add(crate::i18n::dv(&mut z));
            ui.end_row();
        });
    });
}

fn move_layer(app: &mut BeamForge, dir: i32) {
    let Some(l) = app.selected_row else { return };
    let mut order = app.doc.used_layers();
    let Some(i) = order.iter().position(|x| *x == l) else { return };
    let j = i as i32 + dir;
    if j < 0 || j as usize >= order.len() {
        return;
    }
    app.checkpoint_now();
    order.swap(i, j as usize);
    let rest: Vec<usize> = app.doc.layer_order.iter().copied().filter(|x| !order.contains(x)).collect();
    order.extend(rest);
    app.doc.layer_order = order;
}

fn big_icon_button(ui: &mut egui::Ui, ic: crate::icons::Ic, text: &str, fill: Option<Color32>, w: f32) -> bool {
    crate::icons::text_button(ui, ic, text, vec2(w, 30.0), fill, 15.0, text)
}

fn big_button(ui: &mut egui::Ui, text: &str, fill: Option<Color32>, w: f32) -> bool {
    // fixed size: long translations are shortened with "…" instead of widening the panel
    // coloured buttons (Avvia, Ferma) keep white text in both themes
    let rt = egui::RichText::new(text).size(14.0);
    let rt = if fill.is_some() { rt.color(Color32::WHITE) } else { rt };
    let mut b = egui::Button::new(rt).truncate();
    if let Some(f) = fill {
        b = b.fill(f);
    }
    ui.add_sized(vec2(w, 30.0), b).on_hover_text(text).clicked()
}

fn laser_tab(app: &mut BeamForge, ui: &mut egui::Ui) {
    let w = (ui.available_width() - 16.0) / 3.0;
    ui.horizontal(|ui| {
        if always(app, ui, "start", |ui| big_button(ui, tr("Avvia"), Some(Color32::from_rgb(170, 70, 20)), w)) {
            app.start_job();
        }
        let pause_label = if app.paused { tr("Riprendi") } else { tr("Pausa") };
        if always(app, ui, "pause", |ui| big_button(ui, pause_label, None, w)) && app.can("pause") {
            if app.paused {
                app.laser.send(crate::laser::Cmd::Realtime(b'~'));
            } else {
                app.laser.send(crate::laser::Cmd::Realtime(b'!'));
            }
            app.paused = !app.paused;
        }
        if always(app, ui, "stop", |ui| big_button(ui, tr("Ferma"), Some(Color32::from_rgb(120, 30, 30)), w)) && app.can("stop") {
            app.laser.send(crate::laser::Cmd::StopJob);
            app.paused = false;
        }
    });
    let w2 = (3.0 * w + 8.0) / 2.0;
    ui.horizontal(|ui| {
        if always(app, ui, "frame", |ui| big_icon_button(ui, crate::icons::Ic::Frame, tr("Inquadra"), None, w2)) {
            app.frame_job();
        }
        if always(app, ui, "frame_outline", |ui| big_icon_button(ui, crate::icons::Ic::FrameOutline, tr("Inquadra forme"), None, w2)) {
            app.frame_outline_job();
        }
    });
    ui.horizontal(|ui| {
        if always(app, ui, "export_gcode", |ui| big_icon_button(ui, crate::icons::Ic::Save, tr("Salva G-code"), None, w)) {
            app.save_gcode();
        }
        if always(app, ui, "run_gcode", |ui| big_button(ui, tr("Esegui G-code"), None, w)) {
            app.run_gcode_file();
        }
        if always(app, ui, "preview", |ui| big_icon_button(ui, crate::icons::Ic::Monitor, tr("Anteprima"), None, w)) {
            app.open_preview();
        }
    });
    ui.horizontal(|ui| {
        if always(app, ui, "home", |ui| big_icon_button(ui, crate::icons::Ic::Home, tr("Home"), None, w2)) {
            app.send_lines(vec![crate::gcode::HOME_CMD.into()]);
        }
        if always(app, ui, "goto_origin", |ui| big_icon_button(ui, crate::icons::Ic::Origin, tr("Vai all'origine"), None, w2)) {
            app.send_lines(vec![crate::gcode::jog_line(true, 0.0, 0.0, app.jog_speed)]);
        }
    });
    if let Some((a, b)) = app.job_progress {
        ui.add(egui::ProgressBar::new(a as f32 / b as f32).text(trf("Invio {} / {} righe", &[&a, &b])).fill(ACCENT));
    }
    ui.add_space(6.0);
    egui::Grid::new("laser_opts").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.label(tr("Inizia da:"));
        egui::ComboBox::from_id_salt("start_from").width(200.0).selected_text(start_it(app.job_opts.start_from)).show_ui(ui, |ui| {
            for s in [StartFrom::AbsoluteCoords, StartFrom::UserOrigin, StartFrom::CurrentPosition] {
                ui.selectable_value(&mut app.job_opts.start_from, s, start_it(s));
            }
        });
        ui.end_row();
        ui.label(tr("Origine lavoro"));
        ui.add_enabled_ui(app.job_opts.start_from != StartFrom::AbsoluteCoords, |ui| {
            anchor_grid(ui, &mut app.job_opts.job_origin, "job_origin");
        });
        ui.end_row();
    });
    ui.checkbox(&mut app.job_opts.selected_only, tr("Taglia la grafica selezionata"));
    ui.checkbox(&mut app.job_opts.optimize, tr("Ottimizza il percorso (interni prima, percorso più breve)"));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(tr("Periferiche")).min_size(vec2(100.0, 26.0))).clicked() {
            app.open_devices();
        }
        let shown = app.device_sel.and_then(|i| app.device_list.devices.get(i)).map(|d| d.name.clone()).unwrap_or_else(|| tr("Nessuna macchina").to_string());
        let mut pick = None;
        egui::ComboBox::from_id_salt("device_pick").width((ui.available_width() - 8.0).max(120.0)).selected_text(shown).show_ui(ui, |ui| {
            for (i, d) in app.device_list.devices.iter().enumerate() {
                if ui.selectable_label(app.device_sel == Some(i), format!("{}  ({})", d.name, d.controller)).clicked() {
                    pick = Some(i);
                }
            }
            if app.device_list.devices.is_empty() {
                ui.label(egui::RichText::new(tr("Nessuna periferica: apri Periferiche")).color(crate::theme::p().text_dim));
            }
        });
        if let Some(i) = pick {
            app.checkpoint_now();
            app.use_device(i);
        }
    });
    ui.horizontal(|ui| {
        let auto = tr("Automatica (la prima trovata)").to_string();
        egui::ComboBox::from_id_salt("port").width(220.0).selected_text(if app.port_sel.is_empty() { auto.clone() } else { app.port_sel.clone() }).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.port_sel, String::new(), auto);
            for p in app.ports.clone() {
                ui.selectable_value(&mut app.port_sel, p.clone(), p);
            }
        });
        if crate::icons::button(ui, crate::icons::Ic::Refresh, vec2(28.0, 22.0), 14.0, tr("Cerca di nuovo le porte")) {
            app.ports = crate::laser::list_ports();
            app.autocon.queue.clear();
            app.autocon.next_try = std::time::Instant::now();
        }
        let (dot, txt) = match (&app.connected, app.autocon.verified) {
            (Some(n), true) => (egui::Color32::from_rgb(70, 190, 90), trf("Collegato a {}", &[n])),
            _ => (egui::Color32::from_rgb(220, 160, 50), tr("In cerca del laser…").to_string()),
        };
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 4.5, dot);
        ui.label(egui::RichText::new(txt).color(crate::theme::p().text_dim));
    });
    ui.label(egui::RichText::new(trf("Dispositivo: {} ({} × {} mm) — stato: {}", &[&app.doc.device.name, &format!("{:.0}", app.doc.device.width), &format!("{:.0}", app.doc.device.height), &app.machine_state])).color(crate::theme::p().text_dim));
}

fn move_tab(app: &mut BeamForge, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(tr("Distanza"));
        ui.add(crate::i18n::dv(&mut app.jog_dist).speed(0.5).range(0.01..=500.0).suffix(" mm"));
        ui.label(tr("Velocità"));
        ui.add(crate::i18n::dv(&mut app.jog_speed).speed(10.0).range(1.0..=60000.0).suffix(" mm/min"));
    });
    ui.add_space(6.0);
    let d = app.jog_dist;
    egui::Grid::new("jog").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
        let cell = |ui: &mut egui::Ui, ic: crate::icons::Ic| crate::icons::button(ui, ic, vec2(48.0, 40.0), 18.0, "");
        if cell(ui, crate::icons::Ic::Arrow(-1, -1)) {
            app.jog(-d, d);
        }
        if cell(ui, crate::icons::Ic::Arrow(0, -1)) {
            app.jog(0.0, d);
        }
        if cell(ui, crate::icons::Ic::Arrow(1, -1)) {
            app.jog(d, d);
        }
        ui.end_row();
        if cell(ui, crate::icons::Ic::Arrow(-1, 0)) {
            app.jog(-d, 0.0);
        }
        if cell(ui, crate::icons::Ic::Home) {
            app.send_lines(vec![crate::gcode::HOME_CMD.into()]);
        }
        if cell(ui, crate::icons::Ic::Arrow(1, 0)) {
            app.jog(d, 0.0);
        }
        ui.end_row();
        if cell(ui, crate::icons::Ic::Arrow(-1, 1)) {
            app.jog(-d, -d);
        }
        if cell(ui, crate::icons::Ic::Arrow(0, 1)) {
            app.jog(0.0, -d);
        }
        if cell(ui, crate::icons::Ic::Arrow(1, 1)) {
            app.jog(d, -d);
        }
        ui.end_row();
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(tr("Leggi posizione")).clicked() {
            app.laser.send(crate::laser::Cmd::Realtime(b'?'));
        }
        if ui.button(tr("Imposta origine")).clicked() {
            app.send_lines(crate::gcode::zero_axes_lines());
        }
        if ui.button(tr("Vai all'origine")).clicked() {
            app.send_lines(vec![crate::gcode::jog_line(true, 0.0, 0.0, app.jog_speed)]);
        }
    });
    if let Some(p) = app.head_pos {
        ui.label(trf("Posizione testa: X {}  Y {}", &[&format!("{:.2}", p[0]), &format!("{:.2}", p[1])]));
    }
}

fn console_tab(app: &mut BeamForge, ui: &mut egui::Ui) {
    egui::Frame::new().fill(crate::theme::p().field).corner_radius(4).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
        ui.set_min_height(200.0);
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().id_salt("console").max_height(240.0).stick_to_bottom(true).show(ui, |ui| {
            for l in &app.console {
                ui.label(egui::RichText::new(l).monospace().size(12.0));
            }
        });
    });
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut app.console_input).hint_text(tr("Comando G-code…")).desired_width((ui.available_width() - 90.0).max(100.0)));
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button(tr("Invia")).clicked() || enter) && !app.console_input.trim().is_empty() {
            let c = std::mem::take(&mut app.console_input);
            app.send_lines(vec![c]);
        }
    });
    ui.horizontal(|ui| {
        for (l, c) in [("$$", "$$"), (tr("$X sblocca"), "$X"), (tr("$I info"), "$I"), (tr("? stato"), "?")] {
            if ui.small_button(l).clicked() {
                if c == "?" {
                    app.laser.send(crate::laser::Cmd::Realtime(b'?'));
                } else {
                    app.send_lines(vec![c.into()]);
                }
            }
        }
    });
}

fn shape_tab(app: &mut BeamForge, ui: &mut egui::Ui) {
    let frame_nr = ui.ctx().cumulative_frame_nr();
    if app.selection.len() != 1 {
        ui.label(egui::RichText::new(if app.selection.is_empty() { tr("Nessuna forma selezionata.") } else { tr("Più forme selezionate.") }).color(crate::theme::p().text_dim));
        return;
    }
    let id = app.selection[0];
    let Some(mut s) = app.doc.find(id).cloned() else { return };
    let mut changed = false;
    egui::Grid::new("shape_props").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.label(tr("Tipo"));
        ui.label(match &s.geom {
            Geometry::Rect { .. } => tr("Rettangolo"),
            Geometry::Ellipse { .. } => tr("Ellisse"),
            Geometry::Polygon { .. } => tr("Poligono"),
            Geometry::Star { middle: None, .. } => tr("Stella"),
            Geometry::Star { .. } => tr("Stella doppia"),
            Geometry::Path { .. } => tr("Percorso"),
            Geometry::Text { .. } => tr("Testo"),
            Geometry::Image { .. } => tr("Immagine"),
        });
        ui.end_row();
        ui.label(tr("Livello"));
        egui::ComboBox::from_id_salt("shape_layer").selected_text(layer_name(s.layer)).show_ui(ui, |ui| {
            for i in 0..LAYER_COUNT {
                changed |= ui.selectable_value(&mut s.layer, i, egui::RichText::new(format!("■ {}", layer_name(i))).color(layer_color(i))).changed();
            }
        });
        ui.end_row();
        match &mut s.geom {
            Geometry::Rect { radius, w, h } => {
                ui.label(tr("Raggio angoli"));
                let max = w.min(*h) / 2.0;
                changed |= ui.add(crate::i18n::dv(radius).speed(0.1).range(0.0..=max).suffix(" mm")).changed();
                ui.end_row();
            }
            Geometry::Polygon { sides, .. } => {
                ui.label(tr("Lati"));
                changed |= ui.add(crate::i18n::dv(sides).range(3..=64)).changed();
                ui.end_row();
            }
            Geometry::Text { text, size, font, outline } => {
                ui.label(tr("Testo"));
                let r = ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(240.0));
                ui.end_row();
                ui.label(tr("Altezza"));
                let r2 = ui.add(crate::i18n::dv(size).speed(0.2).range(0.5..=1000.0).suffix(" mm"));
                ui.end_row();
                ui.label(tr("Carattere"));
                let mut fc = false;
                egui::ComboBox::from_id_salt("shape_font").width(220.0).selected_text(font.clone()).show_ui(ui, |ui| {
                    for f in crate::text::fonts() {
                        if ui.selectable_label(*font == f.name, &f.name).clicked() {
                            *font = f.name.clone();
                            fc = true;
                        }
                    }
                });
                ui.end_row();
                if r.changed() || r2.changed() || fc {
                    *outline = crate::text::text_outline(text, *size, font);
                    changed = true;
                }
            }
            Geometry::Image { px_w, px_h, .. } => {
                ui.label(tr("Pixel"));
                ui.label(format!("{} × {}", px_w, px_h));
                ui.end_row();
            }
            Geometry::Path { subpaths } => {
                ui.label(tr("Sotto-percorsi"));
                ui.label(format!("{}", subpaths.len()));
                ui.end_row();
            }
            Geometry::Ellipse { .. } => {}
            Geometry::Star { points, inner, middle, .. } => {
                ui.label(tr("Punte"));
                changed |= ui.add(crate::i18n::dv(points).range(2..=64)).changed();
                ui.end_row();
                ui.label(tr("Raggio interno (%)"));
                let mut pct = *inner * 100.0;
                if ui.add(crate::i18n::dv(&mut pct).speed(0.5).range(1.0..=100.0)).changed() {
                    *inner = pct / 100.0;
                    changed = true;
                }
                ui.end_row();
                if let Some(m) = middle {
                    ui.label(tr("Raggio medio (%)"));
                    let mut pct = *m * 100.0;
                    if ui.add(crate::i18n::dv(&mut pct).speed(0.5).range(1.0..=100.0)).changed() {
                        *m = pct / 100.0;
                        changed = true;
                    }
                    ui.end_row();
                }
            }
        }
    });
    if changed {
        app.checkpoint(frame_nr);
        app.doc.touch_layer(s.layer);
        if let Some(t) = app.doc.find_mut(id) {
            *t = s;
        }
    }
}

// ---------------------------------------------------------------- windows

/// The Offset dialog, laid out like the original's: distance on top, then Direzione, Stile angolo
/// and Opzioni side by side, Cancel / OK at the bottom. Every change redraws the preview.
fn offset_window(app: &mut BeamForge, ctx: &egui::Context) {
    use crate::offset::{Corner, Direction, OFFSET_MAX_MM, OFFSET_MIN_MM};
    if !app.offset_open {
        return;
    }
    let mut open = true;
    let mut changed = false;
    let mut action: Option<bool> = None;
    let s = &mut app.offset_settings;
    theme::centered(ctx, "offset", egui::Window::new(tr("Offset")).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Distanza offset"));
                changed |= ui.add(crate::i18n::dv(&mut s.distance).speed(0.1).max_decimals(2).min_decimals(2).range(OFFSET_MIN_MM..=OFFSET_MAX_MM).suffix(" mm")).changed();
            });
        });
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            let group = |ui: &mut egui::Ui, title: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).strong());
                    egui::Frame::new().fill(crate::theme::p().field).corner_radius(4.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                        ui.set_min_height(92.0);
                        add(ui);
                    });
                });
            };
            group(ui, tr("Direzione"), &mut |ui| {
                for (d, l) in [(Direction::Outward, tr("Esterno")), (Direction::Inward, tr("Interno")), (Direction::Both, tr("Entrambe"))] {
                    changed |= ui.radio_value(&mut s.direction, d, l).changed();
                }
            });
            group(ui, tr("Stile angolo"), &mut |ui| {
                for (c, l) in [(Corner::Round, tr("Arrotondato")), (Corner::Square, tr("Smussatura")), (Corner::Miter, tr("Angolo vivo"))] {
                    changed |= ui.radio_value(&mut s.corner, c, l).changed();
                }
            });
            group(ui, tr("Opzioni"), &mut |ui| {
                changed |= ui.checkbox(&mut s.outer_only, tr("Solo forme esterne")).changed();
                changed |= ui.checkbox(&mut s.select_results, tr("Seleziona oggetti risultanti")).changed();
                changed |= ui.checkbox(&mut s.delete_originals, tr("Elimina oggetti originali")).changed();
                changed |= ui.checkbox(&mut s.optimize, tr("Ottimizza / Semplifica risultati")).changed();
            });
        });
        if app.offset_overlap {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tr("Alcune forme selezionate si sovrappongono: l'offset viene applicato a ciascuna separatamente.")).color(Color32::from_rgb(240, 180, 90)).small());
        }
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 28.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("OK").strong()).min_size(vec2(90.0, 26.0))).clicked() {
                action = Some(true);
            }
            if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                action = Some(false);
            }
        });
    });
    if changed {
        app.update_offset_preview();
    }
    if !open {
        action = Some(false);
    }
    match action {
        Some(true) => app.commit_offset(),
        Some(false) => app.cancel_offset(),
        None => {}
    }
}

/// Common / Advanced tabs of the cut settings editor for Fill and Offset Fill
/// (CutSettingEdit::on_cbCutMode_currentIndexChanged 0x100b89f20 and the handlers it enables).
/// Returns whether anything changed.
fn fill_editor(ui: &mut egui::Ui, cs: &mut CutSetting, tab: u8) -> bool {
    let dv = crate::i18n::dv;
    let mut changed = false;
    let scan = cs.mode == CutMode::Fill;
    if tab == 0 {
        ui.horizontal_top(|ui| {
            // scan pattern picture (CutSettingEdit::UpdateScanPattern 0x100b87070)
            let (r, _) = ui.allocate_exact_size(vec2(120.0, 150.0), Sense::hover());
            scan_pattern(ui.painter(), r, cs);
            ui.vertical(|ui| {
                egui::Grid::new("editor_fill").num_columns(3).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                    ui.add_enabled_ui(scan, |ui| {
                        changed |= ui.checkbox(&mut cs.bidir, tr("Riempimento bidirezionale")).changed();
                    });
                    ui.add_enabled_ui(scan, |ui| {
                        changed |= ui.checkbox(&mut cs.crosshatch, tr("Tratteggio incrociato")).changed();
                    });
                    ui.label("");
                    ui.end_row();
                    ui.add_enabled_ui(scan, |ui| {
                        changed |= ui.checkbox(&mut cs.overscan, tr("Sovrascansione")).changed();
                    });
                    changed |= ui.add_enabled(scan && cs.overscan, dv(&mut cs.overscan_percent).speed(0.1).range(0.0..=50.0).max_decimals(1).suffix("%")).changed();
                    // CutSettingEdit::UpdateOverscanDistance 0x100b877f4: speed (mm/s) × % / 100, "%2.2f"
                    ui.label(egui::RichText::new(format!("{:.2}mm", overscan_mm(cs))).color(crate::theme::p().text_dim));
                    ui.end_row();
                    ui.label(tr("Intervallo linea (mm)"));
                    changed |= ui.add(dv(&mut cs.interval).speed(0.005).range(0.01..=5.0).min_decimals(4).max_decimals(4)).changed();
                    ui.label("");
                    ui.end_row();
                    // on_sbLinesPerInch_valueChanged 0x100b8a7dc: interval = 25.4 / lines per inch
                    ui.label(tr("Linee per pollice"));
                    let mut lpi = lines_per_inch(cs.interval);
                    if ui.add(dv(&mut lpi).speed(1.0).range(5.08..=2540.0).min_decimals(2).max_decimals(2)).changed() {
                        cs.interval = interval_from_lpi(lpi);
                        changed = true;
                    }
                    ui.label("");
                    ui.end_row();
                    ui.label(tr("Angolo di scansione (gradi)"));
                    changed |= ui.add_enabled(scan, dv(&mut cs.angle).speed(1.0).range(-360.0..=360.0).max_decimals(0)).changed();
                    ui.label("");
                    ui.end_row();
                    ui.label(tr("Numero di passaggi"));
                    changed |= ui.add(egui::DragValue::new(&mut cs.passes).range(1..=1000)).changed();
                    ui.label("");
                    ui.end_row();
                    // Z offset / Z step need a Z axis, which BeamForge's devices don't have
                    ui.add_enabled(false, egui::Label::new(tr("Offset Z (mm)")));
                    ui.add_enabled(false, dv(&mut cs.z_offset).max_decimals(2).min_decimals(2));
                    ui.label(egui::RichText::new(tr("(nessuno)")).color(crate::theme::p().text_dim));
                    ui.end_row();
                    ui.add_enabled(false, egui::Label::new(tr("Fase Z per passaggio (mm)")));
                    ui.add_enabled(false, dv(&mut cs.z_per_pass).max_decimals(2).min_decimals(2));
                    ui.label(egui::RichText::new(tr("(nessuno)")).color(crate::theme::p().text_dim));
                    ui.end_row();
                });
            });
        });
        ui.separator();
        // scanOpt radios; off for Offset Fill and while Flood Fill is on
        let grouping_on = scan && !cs.flood_fill;
        ui.add_enabled_ui(grouping_on, |ui| {
            let tip = if cs.flood_fill { Some(tr("Per cambiare il raggruppamento disattiva Riempimento completo nella scheda Avanzate")) } else { None };
            for (g, l) in [
                (FillGrouping::MergeAll, tr("Riempi tutte le forme contemporaneamente")),
                (FillGrouping::ByGroup, tr("Riempi i gruppi insieme")),
                (FillGrouping::Individual, tr("Riempi le forme singolarmente")),
            ] {
                let r = ui.radio_value(&mut cs.fill_grouping, g, l);
                let r = match tip {
                    Some(t) => r.on_disabled_hover_text(t),
                    None => r,
                };
                changed |= r.changed();
            }
        });
    } else if scan {
        egui::Grid::new("editor_fill_adv").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            ui.label(tr("Lunghezza rampa"));
            changed |= ui.add(dv(&mut cs.ramp_length).speed(0.05).range(0.0..=100.0).min_decimals(2).max_decimals(2)).on_hover_text(tr("La potenza sale dal minimo al massimo entro questa distanza dal bordo (mm)")).changed();
            ui.end_row();
            ui.label("");
            ui.add_enabled_ui(cs.ramp_length > 0.0, |ui| {
                changed |= ui.checkbox(&mut cs.ramp_outer, tr("Bordo esterno rampa")).on_hover_text(tr("Rampa solo sui bordi esterni, non sui fori")).changed();
            });
            ui.end_row();
        });
        ui.separator();
        changed |= ui.checkbox(&mut cs.flood_fill, tr("Riempimento completo")).on_hover_text(tr("Riempie zona per zona, senza attraversare gli spazi vuoti")).changed();
    } else {
        ui.label(egui::RichText::new(tr("Nessuna opzione avanzata per il riempimento offset.")).color(crate::theme::p().text_dim));
    }
    changed
}

/// Lines per inch for a scan interval (25.4 mm per inch).
pub fn lines_per_inch(interval_mm: f64) -> f64 {
    MM_PER_INCH / interval_mm.max(1e-6)
}

/// Scan interval for a lines-per-inch value (on_sbLinesPerInch_valueChanged 0x100b8a7dc).
pub fn interval_from_lpi(lpi: f64) -> f64 {
    MM_PER_INCH / lpi.max(1e-6)
}

pub const MM_PER_INCH: f64 = 25.4;

/// Overscan distance in mm: speed in mm/s × percent / 100 (UpdateOverscanDistance 0x100b877f4).
pub fn overscan_mm(cs: &CutSetting) -> f64 {
    cs.speed / 60.0 * cs.overscan_percent / 100.0
}

/// Small picture of the scan pattern: zig-zag arrows for Fill (one way when not bidirectional,
/// a faint second pass at 90° with cross-hatch), a spiral for Offset Fill.
fn scan_pattern(p: &egui::Painter, r: Rect, cs: &CutSetting) {
    let ink = crate::theme::p().icon;
    let start = Color32::from_rgb(40, 220, 60);
    let red = Color32::from_rgb(220, 50, 50);
    let c = r.center();
    if cs.mode == CutMode::OffsetFill {
        let pts: Vec<Pos2> = (0..=220)
            .map(|i| {
                let t = i as f32 / 220.0 * std::f32::consts::TAU * 2.6;
                let rad = 4.0 + t * 3.1;
                pos2(c.x + rad * t.cos(), c.y + rad * t.sin())
            })
            .collect();
        let last = *pts.last().unwrap();
        let prev = pts[pts.len() - 4];
        p.add(egui::Shape::line(pts, Stroke::new(2.5, ink)));
        arrow_head(p, prev, last, ink);
        p.circle_filled(c, 5.0, start);
        p.circle_stroke(c, 5.0, Stroke::new(1.0, Color32::BLACK));
        return;
    }
    let draw = |angle: f32, alpha: f32| {
        let (s, co) = angle.to_radians().sin_cos();
        let rot = |x: f32, y: f32| pos2(c.x + x * co - y * s, c.y + x * s + y * co);
        let col = ink.gamma_multiply(alpha);
        let mut forward = true;
        for k in 0..6 {
            let y = 30.0 - k as f32 * 12.0;
            let (x0, x1) = if forward { (-36.0, 36.0) } else { (36.0, -36.0) };
            p.line_segment([rot(x0, y), rot(x1, y)], Stroke::new(2.5, col));
            arrow_head(p, rot(x0, y), rot(x1, y), col);
            if k < 5 {
                let ny = y - 12.0;
                let (rx0, rx1) = if cs.bidir { (x1, x1) } else { (x1, x0) };
                p.line_segment([rot(rx0, y), rot(rx1, ny)], Stroke::new(1.2, red.gamma_multiply(alpha)));
            }
            if cs.bidir {
                forward = !forward;
            }
        }
        p.circle_filled(rot(-36.0, 30.0), 3.0, start.gamma_multiply(alpha));
    };
    // the picture turns with the scan angle, upside down in screen space
    draw(-cs.angle as f32, 1.0);
    if cs.crosshatch {
        draw(90.0 - cs.angle as f32, 0.4);
    }
}

fn arrow_head(p: &egui::Painter, from: Pos2, to: Pos2, col: Color32) {
    let d = (to - from).normalized();
    let n = vec2(-d.y, d.x);
    let a = to - d * 9.0 + n * 5.0;
    let b = to - d * 9.0 - n * 5.0;
    p.add(egui::Shape::convex_polygon(vec![to, a, b], col, Stroke::NONE));
}

pub fn windows(app: &mut BeamForge, ctx: &egui::Context) {
    offset_window(app, ctx);
    if let Some(l) = app.editor_layer {
        // snapshot the layer when the editor opens, so Annulla can put it back
        if app.editor_orig.as_ref().map(|(i, _)| *i) != Some(l) {
            app.editor_orig = Some((l, app.doc.layers[l].clone()));
        }
        let mut open = true;
        let mut cs = app.doc.layers[l].clone();
        let mut changed = false;
        let mut tab = app.editor_tab;
        let manual_tab_count: usize = app.doc.shapes.iter().filter(|s| s.layer == l).map(|s| s.tabs.len()).sum();
        let confirm_id = egui::Id::new("confirm_auto_tabs");
        let mut confirm_auto = ctx.data(|d| d.get_temp::<bool>(confirm_id)).unwrap_or(false);
        let mut action: Option<&'static str> = None;
        let dv = crate::i18n::dv;
        theme::centered(ctx, "cut_editor", egui::Window::new(trf("Editor impostazioni di taglio — {}", &[&layer_name(l)])).open(&mut open).resizable(false).collapsible(false).min_width(440.0)).show(ctx, |ui| {
            egui::Grid::new("editor").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label(tr("Nome"));
                changed |= ui.text_edit_singleline(&mut cs.name).changed();
                ui.end_row();
                ui.label(tr("Nome sottolivello"));
                changed |= ui.text_edit_singleline(&mut cs.sub_name).changed();
                ui.end_row();
                ui.label(tr("Modalità"));
                egui::ComboBox::from_id_salt("ed_mode").selected_text(mode_it(cs.mode)).show_ui(ui, |ui| {
                    for m in [CutMode::Line, CutMode::Fill, CutMode::OffsetFill, CutMode::Image] {
                        changed |= ui.selectable_value(&mut cs.mode, m, mode_it(m)).changed();
                    }
                });
                ui.end_row();
                ui.label(tr("Velocità (mm/min)"));
                changed |= ui.add(dv(&mut cs.speed).speed(10.0).range(0.1..=60000.0)).changed();
                ui.end_row();
                ui.label(tr("Potenza massima (%)"));
                changed |= ui.add(dv(&mut cs.max_power).speed(0.5).range(0.0..=100.0)).changed();
                ui.end_row();
                // greyed out without a separate minimum power (UpdatePowerControls 0x100b86830);
                // the box then shows the maximum, as the original does
                let min_on = app.doc.device.supports_min_power();
                if !min_on {
                    cs.min_power = cs.max_power;
                }
                ui.add_enabled(min_on, egui::Label::new(tr("Potenza minima (%)")));
                changed |= ui.add_enabled(min_on, dv(&mut cs.min_power).speed(0.5).range(0.0..=100.0).min_decimals(2).max_decimals(2)).changed();
                ui.end_row();
                ui.label("");
                changed |= ui.checkbox(&mut cs.constant_power, tr("Modalità potenza costante")).on_hover_text(tr("Il laser resta a potenza costante (M3) anche nelle curve")).changed();
                ui.end_row();
                ui.label(tr("Opzioni"));
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut cs.output, tr("Emissione")).changed();
                    changed |= ui.checkbox(&mut cs.show, tr("Mostra")).changed();
                    changed |= ui.checkbox(&mut cs.air, tr("Aria")).changed();
                });
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut tab, 0, tr("Comune"));
                ui.selectable_value(&mut tab, 1, tr("Avanzate"));
            });
            ui.separator();
            let line = cs.mode == CutMode::Line;
            if matches!(cs.mode, CutMode::Fill | CutMode::OffsetFill) {
                changed |= fill_editor(ui, &mut cs, tab);
            } else if tab == 0 {
                egui::Grid::new("editor_common").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                    ui.label(tr("Numero passaggi"));
                    changed |= ui.add(egui::DragValue::new(&mut cs.passes).range(1..=100)).changed();
                    ui.end_row();
                    if line {
                        ui.label(tr("Offset kerf (mm)"));
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut cs.kerf_enabled, "").changed();
                            changed |= ui.add_enabled(cs.kerf_enabled, dv(&mut cs.kerf).speed(0.01).range(-5.0..=5.0).max_decimals(3)).changed();
                            // UpdateKerfLabel 0x100b8a98c: the full value is applied; its sign picks the side
                            let dir = if !cs.kerf_enabled || cs.kerf == 0.0 {
                                tr("(spento)")
                            } else if cs.kerf > 0.0 {
                                tr("Verso l'esterno")
                            } else {
                                tr("Verso l'interno")
                            };
                            ui.label(egui::RichText::new(dir).color(crate::theme::p().text_dim));
                        });
                        ui.end_row();
                        ui.label(tr("Modalità perforazione"));
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut cs.perforate, "").changed();
                            ui.label(tr("Taglio"));
                            changed |= ui.add_enabled(cs.perforate, dv(&mut cs.perf_len).speed(0.01).range(0.01..=100.0).max_decimals(2)).changed();
                            ui.label(tr("Salta"));
                            changed |= ui.add_enabled(cs.perforate, dv(&mut cs.perf_skip).speed(0.01).range(0.01..=100.0).max_decimals(2)).changed();
                        });
                        ui.end_row();
                    } else {
                        ui.label(tr("Intervallo (mm)"));
                        ui.horizontal(|ui| {
                            changed |= ui.add(dv(&mut cs.interval).speed(0.005).range(0.01..=5.0).max_decimals(3)).changed();
                            ui.label(egui::RichText::new(trf("= {} linee per pollice", &[&format!("{:.0}", 25.4 / cs.interval)])).color(crate::theme::p().text_dim));
                        });
                        ui.end_row();
                        ui.label(tr("Angolo di scansione (°)"));
                        changed |= ui.add(dv(&mut cs.angle).speed(1.0).range(-180.0..=180.0)).changed();
                        ui.end_row();
                        ui.label(tr("Scansione"));
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut cs.bidir, tr("Bidirezionale")).changed();
                            if cs.mode == CutMode::Fill {
                                changed |= ui.checkbox(&mut cs.crosshatch, tr("Tratteggio incrociato")).changed();
                            }
                        });
                        ui.end_row();
                    }
                    if cs.mode == CutMode::Image {
                        ui.label(tr("Modalità immagine"));
                        egui::ComboBox::from_id_salt("ed_img").selected_text(image_mode_it(cs.image_mode)).show_ui(ui, |ui| {
                            for m in [ImageMode::Threshold, ImageMode::Dithered, ImageMode::Grayscale] {
                                changed |= ui.selectable_value(&mut cs.image_mode, m, image_mode_it(m)).changed();
                            }
                        });
                        ui.end_row();
                        ui.label(tr("Negativo"));
                        changed |= ui.checkbox(&mut cs.negative, tr("Inverti l'immagine")).changed();
                        ui.end_row();
                    }
                });
                if line {
                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut cs.tabs_enabled, egui::RichText::new(tr("Linguette / Ponti")).strong()).changed();
                        });
                        ui.add_enabled_ui(cs.tabs_enabled, |ui| {
                            egui::Grid::new("editor_tabs").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                                ui.label(tr("Generazione"));
                                ui.horizontal(|ui| {
                                    // TabGenChanged 0x100b8af30: leaving manual mode with tabs placed asks first
                                    if ui.radio(!cs.manual_tabs, tr("Automatica")).clicked() && cs.manual_tabs {
                                        if manual_tab_count > 0 {
                                            confirm_auto = true;
                                        } else {
                                            cs.manual_tabs = false;
                                            changed = true;
                                        }
                                    }
                                    changed |= ui.radio_value(&mut cs.manual_tabs, true, tr("Manuale")).changed();
                                });
                                ui.end_row();
                                ui.label(tr("Dimensione linguetta (mm)"));
                                changed |= ui.add(dv(&mut cs.tab_size).speed(0.01).range(0.01..=50.0).max_decimals(2)).changed();
                                ui.end_row();
                                ui.add_enabled_ui(!cs.manual_tabs, |ui| {
                                    changed |= ui.radio_value(&mut cs.tabs_use_spacing, true, tr("Spaziatura uniforme (mm)")).changed();
                                });
                                changed |= ui.add_enabled(!cs.manual_tabs && cs.tabs_use_spacing, dv(&mut cs.tab_spacing).speed(0.5).range(1.0..=1000.0).max_decimals(2)).changed();
                                ui.end_row();
                                ui.add_enabled_ui(!cs.manual_tabs, |ui| {
                                    // UpdateTabsUI 0x100b87bf0: "Tab Count" when spacing is off, "Min Tabs" when it is on
                                    changed |= ui.radio_value(&mut cs.tabs_use_spacing, false, tr("Linguette per forma")).changed();
                                });
                                ui.horizontal(|ui| {
                                    ui.label(if cs.tabs_use_spacing { tr("Min linguette") } else { tr("Numero linguette") });
                                    if ui.add_enabled(!cs.manual_tabs, egui::DragValue::new(&mut cs.tab_count).range(1..=100)).changed() {
                                        changed = true;
                                        // on_sbTabCount 0x100b8bcf8: max never below the count
                                        if cs.tab_count_max < cs.tab_count {
                                            cs.tab_count_max = cs.tab_count;
                                        }
                                    }
                                });
                                ui.end_row();
                                ui.add_enabled_ui(!cs.manual_tabs, |ui| {
                                    changed |= ui.checkbox(&mut cs.limit_max_tabs, tr("Limita linguette max")).changed();
                                });
                                if ui.add_enabled(!cs.manual_tabs && cs.tabs_use_spacing && cs.limit_max_tabs, egui::DragValue::new(&mut cs.tab_count_max).range(1..=100)).changed() {
                                    changed = true;
                                    if cs.tab_count_max < cs.tab_count {
                                        cs.tab_count_max = cs.tab_count;
                                    }
                                }
                                ui.end_row();
                                changed |= ui.checkbox(&mut cs.tab_power_enabled, tr("Potenza taglio linguette (%)")).changed();
                                changed |= ui.add_enabled(cs.tab_power_enabled, dv(&mut cs.tab_cut_power).speed(0.5).range(0.0..=100.0)).changed();
                                ui.end_row();
                            });
                            ui.horizontal(|ui| {
                                if ui.button(tr("Cancella linguette")).clicked() {
                                    action = Some("clear_tabs");
                                }
                                changed |= ui.checkbox(&mut cs.skip_inner_tabs, tr("Ignora forme interne")).changed();
                            });
                            if cs.manual_tabs {
                                ui.horizontal(|ui| {
                                    ui.label(trf("Linguette posizionate: {}", &[&manual_tab_count.to_string()]));
                                    if ui.button(tr("Posiziona sul disegno")).clicked() {
                                        action = Some("tab_tool");
                                    }
                                });
                                ui.label(egui::RichText::new(tr("Strumento Linguette: clic sul contorno per aggiungere, trascina per spostare, Ctrl/Cmd-clic o doppio clic per togliere.")).color(crate::theme::p().text_dim).small());
                            }
                        });
                    });
                }
            } else {
                egui::Grid::new("editor_adv").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                    if line {
                        ui.label(tr("Sovrataglio (mm)"));
                        changed |= ui.add(dv(&mut cs.overcut).speed(0.05).range(0.0..=100.0).max_decimals(2)).changed();
                        ui.end_row();
                        changed |= ui.checkbox(&mut cs.lead_in, tr("Aggiungi lead-in")).changed();
                        ui.horizontal(|ui| {
                            ui.label(tr("Lunghezza"));
                            changed |= ui.add_enabled(cs.lead_in, dv(&mut cs.lead_in_len).speed(0.05).range(0.0..=100.0).max_decimals(2)).changed();
                            ui.label(tr("Angolo"));
                            changed |= ui.add_enabled(cs.lead_in, dv(&mut cs.lead_in_angle).speed(1.0).range(0.0..=180.0)).changed();
                        });
                        ui.end_row();
                        changed |= ui.checkbox(&mut cs.lead_out, tr("Aggiungi lead-out")).changed();
                        ui.horizontal(|ui| {
                            ui.label(tr("Lunghezza"));
                            changed |= ui.add_enabled(cs.lead_out, dv(&mut cs.lead_out_len).speed(0.05).range(0.0..=100.0).max_decimals(2)).changed();
                        });
                        ui.end_row();
                        ui.label(tr("Stile Lead In/Out"));
                        ui.add_enabled_ui(cs.lead_in || cs.lead_out, |ui| {
                            ui.horizontal(|ui| {
                                changed |= ui.radio_value(&mut cs.lead_style, LeadStyle::Line, tr("Linea")).changed();
                                changed |= ui.radio_value(&mut cs.lead_style, LeadStyle::Arc, tr("Arco")).changed();
                            });
                        });
                        ui.end_row();
                    } else {
                        ui.label(tr("Overscan"));
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut cs.overscan, "").changed();
                            changed |= ui.add_enabled(cs.overscan, dv(&mut cs.overscan_percent).speed(0.1).range(0.0..=50.0).suffix(tr(" % della velocità"))).changed();
                        });
                        ui.end_row();
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr("Resetta a predefinito")).clicked() {
                    action = Some("reset");
                }
                if ui.button(tr("Rendi predefinito")).clicked() {
                    action = Some("default");
                }
                if ui.button(tr("Rendi predefinito per tutti")).clicked() {
                    action = Some("default_all");
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(tr("OK")).clicked() {
                    action = Some("ok");
                }
                if ui.button(crate::i18n::cancel()).clicked() {
                    action = Some("cancel");
                }
            });
        });
        app.editor_tab = tab;
        if confirm_auto {
            let mut answer: Option<bool> = None;
            theme::centered(ctx, "confirm_auto_tabs_win", egui::Window::new(tr("Linguette automatiche")).collapsible(false).resizable(false)).show(ctx, |ui| {
                ui.label(tr("Passando alle linguette automatiche verranno eliminate le linguette manuali. Continuare?"));
                ui.horizontal(|ui| {
                    if ui.button(tr("Sì")).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(tr("No")).clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(yes) = answer {
                confirm_auto = false;
                if yes {
                    action = Some("auto_tabs");
                }
            }
        }
        ctx.data_mut(|d| d.insert_temp(confirm_id, confirm_auto));
        match action {
            Some("clear_tabs") | Some("auto_tabs") => {
                app.checkpoint_now();
                for s in app.doc.shapes.iter_mut().filter(|s| s.layer == l) {
                    s.tabs.clear();
                }
                if action == Some("auto_tabs") {
                    cs.manual_tabs = false;
                } else if !cs.manual_tabs {
                    cs.tabs_enabled = false;
                }
                changed = true;
            }
            Some("tab_tool") => {
                app.tool = Tool::Tabs;
            }
            Some("reset") => {
                let name = cs.name.clone();
                cs = app.cut_defaults.get(&l).cloned().unwrap_or_default();
                cs.name = name;
                changed = true;
            }
            Some("default") => {
                app.cut_defaults.insert(l, cs.clone());
                crate::i18n::save_cut_defaults(&app.cut_defaults);
                app.status = tr("Impostazioni salvate come predefinite per questo livello").to_string();
            }
            Some("default_all") => {
                for i in 0..app.doc.layers.len() {
                    let mut d = cs.clone();
                    d.name = app.doc.layers[i].name.clone();
                    app.cut_defaults.insert(i, d);
                }
                crate::i18n::save_cut_defaults(&app.cut_defaults);
                app.status = tr("Impostazioni salvate come predefinite per tutti i livelli").to_string();
            }
            _ => {}
        }
        if changed {
            if cs.min_power > cs.max_power {
                cs.min_power = cs.max_power;
            }
            app.checkpoint(ctx.cumulative_frame_nr());
            app.doc.layers[l] = cs;
        }
        let cancel = action == Some("cancel") || !open;
        if cancel {
            if let Some((i, orig)) = app.editor_orig.take() {
                if serde_json::to_string(&app.doc.layers[i]).ok() != serde_json::to_string(&orig).ok() {
                    app.checkpoint(ctx.cumulative_frame_nr());
                    app.doc.layers[i] = orig;
                }
            }
        }
        if cancel || action == Some("ok") {
            app.editor_layer = None;
            app.editor_orig = None;
        }
    }

    // Device settings work on a draft: OK applies, Annulla or ✕ throws it away
    // (MainWindow::on_actionDevice_Settings_triggered 0x100688400: QDialog::exec, applied on accept).
    if app.show_device {
        let mut open = true;
        let mut d = app.device_draft.take().unwrap_or_else(|| app.doc.device.clone());
        let mut ok = false;
        let mut cancel = false;
        theme::centered(ctx, "device", egui::Window::new(tr("Impostazioni dispositivo")).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
            egui::Grid::new("device").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label(tr("Nome"));
                ui.text_edit_singleline(&mut d.name);
                ui.end_row();
                ui.label(tr("Larghezza area (mm)"));
                ui.add(crate::i18n::dv(&mut d.width).speed(1.0).range(10.0..=15000.0));
                ui.end_row();
                ui.label(tr("Altezza area (mm)"));
                ui.add(crate::i18n::dv(&mut d.height).speed(1.0).range(10.0..=15000.0));
                ui.end_row();
                let names = [
                    (OriginCorner::FrontLeft, tr("Anteriore sinistra")),
                    (OriginCorner::RearLeft, tr("Posteriore sinistra")),
                    (OriginCorner::FrontRight, tr("Anteriore destra")),
                    (OriginCorner::RearRight, tr("Posteriore destra")),
                ];
                ui.label(tr("Origine macchina"));
                let cur = names.iter().find(|n| n.0 == d.origin).map(|n| n.1).unwrap_or("");
                egui::ComboBox::from_id_salt("origin").selected_text(cur).show_ui(ui, |ui| {
                    for (o, n) in names {
                        ui.selectable_value(&mut d.origin, o, n);
                    }
                });
                ui.end_row();
                ui.label(tr("Valore S massimo ($30)"));
                ui.add(crate::i18n::dv(&mut d.s_max).speed(10.0).range(1.0..=100000.0));
                ui.end_row();
                ui.label(tr("Potenza dinamica"));
                ui.checkbox(&mut d.dynamic_power, tr("M4 (spenta: M3 potenza costante)"));
                ui.end_row();
                ui.label(tr("Aria compressa"));
                ui.checkbox(&mut d.air_assist_m7, tr("Usa M7 invece di M8"));
                ui.end_row();
                ui.label(tr("Potenza Fuoco (%)"));
                ui.add(crate::i18n::dv(&mut d.fire_power).speed(0.1).range(0.0..=100.0));
                ui.end_row();
                ui.label(tr("Inquadra con laser acceso"));
                ui.checkbox(&mut d.laser_frame, tr("Raggio continuo all'1%"));
                ui.end_row();
                ui.label(tr("Velocità rapidi (mm/s)"));
                ui.add(crate::i18n::dv(&mut d.rapid_speed).speed(1.0).range(1.0..=2000.0));
                ui.end_row();
                ui.label(tr("Accelerazione (mm/s²)"));
                ui.add(crate::i18n::dv(&mut d.accel).speed(10.0).range(10.0..=50000.0));
                ui.end_row();
                ui.label(tr("Baud"));
                egui::ComboBox::from_id_salt("baud").selected_text(d.baud.to_string()).show_ui(ui, |ui| {
                    for b in [9600u32, 57600, 115200, 230400, 250000] {
                        ui.selectable_value(&mut d.baud, b, b.to_string());
                    }
                });
                ui.end_row();
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(tr("OK")).fill(Color32::from_rgb(170, 70, 20)).min_size(vec2(90.0, 26.0))).clicked() {
                    ok = true;
                }
                if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                    cancel = true;
                }
            });
        });
        if ok {
            if d.width != app.doc.device.width || d.height != app.doc.device.height {
                app.view.fit_pending = true;
            }
            app.checkpoint_now();
            // keep the saved laser in Periferiche in step with its settings
            if let Some(slot) = app.device_sel.and_then(|i| app.device_list.devices.get_mut(i)) {
                *slot = d.clone();
                app.device_list.save();
            }
            app.doc.device = d;
            app.show_device = false;
        } else if cancel || !open {
            app.show_device = false;
        } else {
            app.device_draft = Some(d);
        }
    }

    if app.show_settings {
        let mut open = true;
        let current_choice = crate::i18n::load_choice();
        let mut picked: Option<Option<crate::i18n::Lang>> = None;
        theme::centered(ctx, "settings", egui::Window::new(tr("Impostazioni")).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
                ui.label(tr("Lingua"));
                let shown = match current_choice {
                    None => tr("Automatica (lingua di sistema)").to_string(),
                    Some(l) => l.native_name().to_string(),
                };
                egui::ComboBox::from_id_salt("language").width(240.0).selected_text(shown).show_ui(ui, |ui| {
                    if ui.selectable_label(current_choice.is_none(), tr("Automatica (lingua di sistema)")).clicked() {
                        picked = Some(None);
                    }
                    for l in crate::i18n::ALL {
                        if ui.selectable_label(current_choice == Some(l), l.native_name()).clicked() {
                            picked = Some(Some(l));
                        }
                    }
                });
                ui.end_row();
                // light/dark theme (the original offers Classic Light / Classic Dark, Core_Theme 0x1007295dc)
                ui.label(tr("Tema"));
                let light = theme::p().light;
                ui.horizontal(|ui| {
                    if ui.selectable_label(!light, tr("Scuro")).clicked() && light {
                        theme::set_light(ctx, false);
                        theme::save_light(false);
                    }
                    if ui.selectable_label(light, tr("Chiaro")).clicked() && !light {
                        theme::set_light(ctx, true);
                        theme::save_light(true);
                    }
                });
                ui.end_row();
            });
            ui.label(egui::RichText::new(trf("Lingua di sistema rilevata: {}", &[&crate::i18n::system_lang().native_name()])).color(crate::theme::p().text_dim));
            ui.label(egui::RichText::new(tr("La lingua cambia subito in tutta l'app e viene ricordata.")).color(crate::theme::p().text_dim));
            ui.separator();
            if ui.button(tr("Impostazioni dispositivo…")).clicked() {
                app.show_device = true;
            }
        });
        if let Some(choice) = picked {
            app.set_language(choice, ctx);
        }
        if !open {
            app.show_settings = false;
        }
    }

    if app.show_about {
        let mut open = true;
        theme::centered(ctx, "about", egui::Window::new(tr("Informazioni su BeamForge")).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
            // same order as the original's About page (HelpDialog::HelpDialog 0x100c1a2b8): who made it,
            // when it was built (UTC), the licence, then the third-party components with their licences.
            // The original's "License Validation Date" lines are left out: BeamForge needs no licence key.
            ui.heading(egui::RichText::new("◆ BeamForge 1.0").color(ACCENT));
            ui.label(tr("Software libero per progettare e controllare laser."));
            ui.add_space(6.0);
            ui.label("Copyright © 2026 servimmagine");
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tr("Compilato il:")).strong());
                ui.label(format!("{} UTC", env!("BEAMFORGE_BUILD_UTC")));
            });
            ui.add_space(6.0);
            ui.label(tr("Licenza open source Apache 2.0: libero da usare, copiare e modificare. Nessuna prova a tempo, nessuna attivazione."));
            ui.label(egui::RichText::new(tr("Controller supportati: GRBL (G-code via porta seriale).")).color(crate::theme::p().text_dim));
            ui.add_space(8.0);
            ui.label(egui::RichText::new(tr("Componenti di terze parti")).strong());
            for (name, licence, url) in THIRD_PARTY {
                ui.horizontal(|ui| {
                    ui.label("•");
                    ui.hyperlink_to(egui::RichText::new(*name).strong(), *url);
                    ui.label(egui::RichText::new(*licence).color(crate::theme::p().text_dim));
                });
            }
        });
        if !open {
            app.show_about = false;
        }
    }

    crate::devices::window(app, ctx);
    crate::combine::assistant_window(app, ctx);
    crate::preview::window(app, ctx);
}
