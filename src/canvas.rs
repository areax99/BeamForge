//! The design canvas: workspace grid, rulers, shapes, selection and drawing tools.

use crate::app::{BeamForge, Drag, Tool};
use crate::model::*;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

const RULER: f32 = 20.0;

pub struct Xf {
    pub origin: Pos2,
    pub zoom: f32,
}

impl Xf {
    pub fn to_screen(&self, p: P2) -> Pos2 {
        pos2(self.origin.x + p[0] as f32 * self.zoom, self.origin.y - p[1] as f32 * self.zoom)
    }
    pub fn to_world(&self, s: Pos2) -> P2 {
        [((s.x - self.origin.x) / self.zoom) as f64, ((self.origin.y - s.y) / self.zoom) as f64]
    }
}

pub fn layer_color(i: usize) -> Color32 {
    let c = PALETTE[i.min(LAYER_COUNT - 1)];
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Colour used to draw a layer on the canvas (layer 00 is black: drawn light on the dark theme).
pub fn display_color(i: usize) -> Color32 {
    // test cells (past the palette) are made with the 00 material setting
    if i == 0 || i >= LAYER_COUNT { crate::theme::p().layer0 } else { layer_color(i) }
}

/// Light theme small-square rule from the original (EditWindow::drawGrid 0x1009fd344, light branch):
/// small squares are hidden while a big square is at most 80 px wide on screen; above that their grey
/// fades from 248 (GRID_MINOR_BASE) towards halfway to the big-square grey 224 (GridShade default,
/// Preferences ctor 0x10098b514), reached when t = (px - 60) / 120 = 1. The dark theme keeps its colour.
pub const GRID_MINOR_HIDE_PX: f32 = 80.0;
pub const GRID_FADE_START_PX: f32 = 60.0;
pub const GRID_FADE_RANGE_PX: f32 = 120.0;
pub const GRID_MINOR_BASE: f32 = 248.0;
pub const GRID_SHADE: f32 = 224.0;

fn minor_grid_color(major_px: f32) -> Option<Color32> {
    if !crate::theme::p().light {
        return Some(crate::theme::p().grid_minor);
    }
    if major_px <= GRID_MINOR_HIDE_PX {
        return None;
    }
    let t = ((major_px - GRID_FADE_START_PX) / GRID_FADE_RANGE_PX).min(1.0);
    let full = (GRID_MINOR_BASE + GRID_SHADE) / 2.0;
    Some(Color32::from_gray((GRID_MINOR_BASE + (full - GRID_MINOR_BASE) * t).round() as u8))
}

fn nice_step(zoom: f32) -> f64 {
    // grid step in mm so that lines are at least ~8px apart
    for s in [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0] {
        if s as f32 * zoom >= 8.0 {
            return s;
        }
    }
    1000.0
}

/// MainWindow::on_actionZoom_In_triggered (0x100c571a8)
pub const ZOOM_IN_FACTOR: f32 = 1.25;
/// MainWindow::on_actionZoom_Out_triggered (0x100c571b8)
pub const ZOOM_OUT_FACTOR: f32 = 0.8;
/// EditWindow::ZoomRelative / ZoomTo clamp (0x1009fbe00, 0x1009fc1e4)
pub const ZOOM_MIN: f32 = 0.001;
pub const ZOOM_MAX: f32 = 5000.0;
/// EditWindow::ZoomTo fit margin (0x1009fc1e4)
pub const ZOOM_FIT_MARGIN: f32 = 0.96;

/// Toolbar/menu zoom: scales about the view centre (EditWindow::ZoomRelative(float)).
pub fn zoom_relative(app: &mut BeamForge, factor: f32) {
    let c = app.view.last_area / 2.0;
    let wx = (c.x - app.view.offset.x) / app.view.zoom;
    let wy = (app.view.offset.y - c.y) / app.view.zoom;
    let nz = (app.view.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    app.view.zoom = nz;
    app.view.auto_fit = false;
    app.view.offset = vec2(c.x - wx * nz, c.y + wy * nz);
}

pub fn fit_view(app: &mut BeamForge, area: Rect) {
    let w = (app.doc.device.width as f32).max(1.0);
    let h = (app.doc.device.height as f32).max(1.0);
    let zoom = ((area.width() / w).min(area.height() / h) * ZOOM_FIT_MARGIN).clamp(ZOOM_MIN, ZOOM_MAX);
    app.view.zoom = zoom;
    let ox = (area.width() - w * zoom) / 2.0;
    let oy = (area.height() + h * zoom) / 2.0;
    app.view.offset = vec2(ox, oy);
}

pub fn zoom_to(app: &mut BeamForge, area: Rect, b: BBox) {
    if !b.is_valid() {
        return;
    }
    let w = (b.w() as f32).max(1.0);
    let h = (b.h() as f32).max(1.0);
    let zoom = ((area.width() / w).min(area.height() / h) * ZOOM_FIT_MARGIN).clamp(ZOOM_MIN, ZOOM_MAX);
    app.view.zoom = zoom;
    let c = b.center();
    app.view.offset = vec2(area.width() / 2.0 - c[0] as f32 * zoom, area.height() / 2.0 + c[1] as f32 * zoom);
}

/// The rotate handle sits this many screen pixels outside each corner, across and down
/// (SelectionTool::Draw 0x100779234, handle type 4: ±8.0 on both axes).
pub(crate) const ROTATE_HANDLE_OFFSET_PX: f32 = 8.0;
/// How close (screen pixels) the pointer must be to a rotate handle to grab it. The original uses
/// 8 px (SelectionTool::Update, nearest handle / gViewScale < 8.0); BeamForge allows 14 so the arrow
/// is easy to catch, as the person had trouble grabbing it (estimated).
pub(crate) const ROTATE_GRAB_PX: f32 = 14.0;

/// Screen point of the rotate handle at a corner, pushed outward from the selection's middle.
pub(crate) fn rotate_handle_pos(corner: Pos2, mid: Pos2) -> Pos2 {
    let d = corner - mid;
    corner + vec2(d.x.signum(), d.y.signum()) * ROTATE_HANDLE_OFFSET_PX
}

/// Degrees shown as -180..180 (0x10077793c..0x100777960).
fn wrap_degrees(d: f64) -> f64 {
    let d = if d <= -180.0 { d + 360.0 } else { d };
    if d > 180.0 { d - 360.0 } else { d }
}

/// A curved double arrow round a corner, its middle on the rotate handle point.
fn draw_rotate_arrow(painter: &egui::Painter, corner: Pos2, handle: Pos2) {
    let out = handle - corner;
    let r = out.length();
    let base = out.y.atan2(out.x);
    let span = std::f32::consts::FRAC_PI_4 * 1.3;
    let at = |t: f32| corner + vec2(t.cos(), t.sin()) * r;
    let pts: Vec<Pos2> = (0..=12).map(|i| at(base - span + 2.0 * span * i as f32 / 12.0)).collect();
    painter.add(egui::Shape::line(pts.clone(), Stroke::new(4.0, Color32::from_black_alpha(140))));
    painter.add(egui::Shape::line(pts, Stroke::new(2.0, crate::theme::ACCENT)));
    for (t, sgn) in [(base - span, -1.0f32), (base + span, 1.0)] {
        let tip = at(t);
        let tangent = vec2(-t.sin(), t.cos()) * sgn;
        let normal = vec2(t.cos(), t.sin());
        let back = tip - tangent * 5.0;
        painter.add(egui::Shape::convex_polygon(vec![tip + tangent * 1.5, back + normal * 4.0, back - normal * 4.0], crate::theme::ACCENT, Stroke::NONE));
    }
}

fn handle_points(b: &BBox) -> [P2; 8] {
    let c = b.center();
    [
        [b.min[0], b.max[1]],
        [c[0], b.max[1]],
        [b.max[0], b.max[1]],
        [b.max[0], c[1]],
        [b.max[0], b.min[1]],
        [c[0], b.min[1]],
        [b.min[0], b.min[1]],
        [b.min[0], c[1]],
    ]
}

/// Top-most shape under a point: outlines first, then the smallest closed shape containing it.
pub fn hit_test(app: &BeamForge, p: P2, tol: f64) -> Option<u64> {
    for s in app.doc.shapes.iter().rev() {
        if !app.doc.layers[s.layer].show {
            continue;
        }
        let b = s.bbox();
        if p[0] < b.min[0] - tol || p[0] > b.max[0] + tol || p[1] < b.min[1] - tol || p[1] > b.max[1] + tol {
            continue;
        }
        if matches!(s.geom, Geometry::Image { .. }) {
            continue;
        }
        for pl in s.world_paths() {
            let n = pl.pts.len();
            for i in 0..n {
                if i + 1 == n && !pl.closed {
                    break;
                }
                if dist_seg(p, pl.pts[i], pl.pts[(i + 1) % n]) <= tol {
                    return Some(s.id);
                }
            }
        }
    }
    let mut best: Option<(u64, f64)> = None;
    for s in app.doc.shapes.iter().rev() {
        if !app.doc.layers[s.layer].show {
            continue;
        }
        let b = s.bbox();
        let inside = if matches!(s.geom, Geometry::Image { .. }) {
            p[0] >= b.min[0] && p[0] <= b.max[0] && p[1] >= b.min[1] && p[1] <= b.max[1]
        } else {
            point_in_rings(p, &s.world_paths())
        };
        if inside {
            let a = b.w() * b.h();
            if best.is_none_or(|(_, ba)| a < ba) {
                best = Some((s.id, a));
            }
        }
    }
    best.map(|b| b.0)
}

/// Where the next line point goes: with Shift held, the pointer is kept on the nearest of
/// horizontal, vertical or 45° from the last point (LineTool::AxisSnap 0x100766010).
fn line_cursor(app: &BeamForge, cursor: Option<P2>, shift: bool) -> Option<P2> {
    let c = cursor?;
    match app.line_pts.last() {
        Some(last) if shift => Some(line_axis_snap(*last, c)),
        _ => Some(c),
    }
}

/// LineTool::AxisSnap (0x100766010): horizontal when |dy| is the smallest miss, vertical when |dx| is,
/// otherwise 45° with both sides set to the longer of |dx| and |dy| (MECHANICS.md, "Linea con Shift").
pub(crate) fn line_axis_snap(a: P2, b: P2) -> P2 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let diag_miss = (dx.abs() - dy.abs()).abs();
    if dy.abs() <= dx.abs() && dy.abs() <= diag_miss {
        [b[0], a[1]]
    } else if dx.abs() <= dy.abs() && dx.abs() <= diag_miss {
        [a[0], b[1]]
    } else {
        let m = dx.abs().max(dy.abs());
        [a[0] + m * dx.signum(), a[1] + m * dy.signum()]
    }
}

pub fn finish_line(app: &mut BeamForge, closed: bool) {
    let pts = std::mem::take(&mut app.line_pts);
    if pts.len() < 2 {
        return;
    }
    let mut b = BBox::empty();
    for p in &pts {
        b.add(*p);
    }
    let c = b.center();
    let local = pts.iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect();
    let s = Shape::new(0, Geometry::Path { subpaths: vec![Polyline { pts: local, closed }] }, app.current_layer, c[0], c[1]);
    app.checkpoint_now();
    let id = app.doc.add_shape(s);
    app.selection = vec![id];
    // "Passa alla modalità Seleziona dopo aver disegnato una forma"
    if app.prefs.select_after_drawing {
        app.tool = Tool::Select;
    }
}

/// The key that holds a dragged shape to a 1:1 box: only Option (Alt), as the person asked
/// (the original keeps the box square with the larger side, BasicShapeTool::Update 0x10075694c).
/// Bounds sides at or below this many mm are not scaled (0x38d1b717 = 0.0001, 0x100777f0c).
pub const MIN_SCALE_SIDE: f64 = 0.0001;

pub fn square_held(m: &egui::Modifiers) -> bool {
    m.alt
}

/// The far corner of a dragged box; with `square` both sides take the larger one,
/// keeping the drag's direction.
pub fn constrain_corner(a: P2, b: P2, square: bool) -> P2 {
    if !square {
        return b;
    }
    let (w, h) = (b[0] - a[0], b[1] - a[1]);
    let m = w.abs().max(h.abs());
    let sx = if w < 0.0 { -1.0 } else { 1.0 };
    let sy = if h < 0.0 { -1.0 } else { 1.0 };
    [a[0] + m * sx, a[1] + m * sy]
}

/// The two corners of a dragged box (BasicShapeTool::Update 0x10075694c): half sizes are
/// |dx| and |dy| (1:1 → both the larger); with `centred` (Cmd, the original's Ctrl bit
/// 0x4000000) the press point is the centre and the box is twice as large, otherwise it is
/// the corner the drag started from.
pub fn drag_box(start: P2, cur: P2, square: bool, centred: bool) -> (P2, P2) {
    let b = constrain_corner(start, cur, square);
    if centred {
        let (hx, hy) = (b[0] - start[0], b[1] - start[1]);
        ([start[0] - hx, start[1] - hy], b)
    } else {
        (start, b)
    }
}

fn create_shape(app: &mut BeamForge, a: P2, b: P2, square: bool, centred: bool) {
    let (a, b) = drag_box(a, b, square, centred);
    let w = b[0] - a[0];
    let h = b[1] - a[1];
    let c = [a[0] + w / 2.0, a[1] + h / 2.0];
    let geom = match app.tool {
        Tool::Rect => {
            if w.abs() < 0.2 || h.abs() < 0.2 {
                return;
            }
            Geometry::Rect { w: w.abs(), h: h.abs(), radius: 0.0 }
        }
        Tool::Ellipse => {
            if w.abs() < 0.2 || h.abs() < 0.2 {
                return;
            }
            Geometry::Ellipse { rx: w.abs() / 2.0, ry: h.abs() / 2.0 }
        }
        Tool::Star | Tool::DualStar => {
            if w.abs() < 0.2 || h.abs() < 0.2 {
                return;
            }
            let middle = (app.tool == Tool::DualStar).then_some(DUAL_STAR_MIDDLE);
            Geometry::Star { points: STAR_POINTS, rx: w.abs() / 2.0, ry: h.abs() / 2.0, inner: STAR_INNER, middle }
        }
        t if t.is_ngon() => {
            if w.abs() < 0.2 || h.abs() < 0.2 {
                return;
            }
            let sides = t.fixed_sides().unwrap_or(app.polygon_sides);
            Geometry::Polygon { sides, r: w.abs() / 2.0, ry: Some(h.abs() / 2.0) }
        }
        _ => return,
    };
    let s = Shape::new(0, geom, app.current_layer, c[0], c[1]);
    app.checkpoint_now();
    let id = app.doc.add_shape(s);
    app.selection = vec![id];
}

fn draw_polyline(painter: &egui::Painter, xf: &Xf, pl: &Polyline, stroke: Stroke, dashed: bool) {
    let mut pts: Vec<Pos2> = pl.pts.iter().map(|p| xf.to_screen(*p)).collect();
    // like the original (EditWindow::drawPathShape 0x100a0216c): a point is skipped when it is less
    // than half a pixel from the last drawn one (|dx| + |dy| < 0.5), and a shape smaller than one
    // pixel each way is drawn as a single dot at its centre
    let near = |a: Pos2, b: Pos2| (a.x - b.x).abs() + (a.y - b.y).abs() < 0.5;
    if let Some(r) = (pts.len() > 1).then(|| Rect::from_points(&pts)) {
        if r.width() <= 1.0 && r.height() <= 1.0 {
            painter.circle_filled(r.center(), stroke.width.max(1.0) * 0.5, stroke.color);
            return;
        }
    }
    pts.dedup_by(|b, a| near(*a, *b));
    if pl.closed {
        while pts.len() > 2 && near(pts[0], *pts.last().unwrap()) {
            pts.pop();
        }
    }
    if pts.len() < 2 {
        return;
    }
    if pl.closed {
        pts.push(pts[0]);
    }
    if dashed {
        painter.extend(egui::Shape::dashed_line(&pts, stroke, 4.0, 3.0));
    } else {
        painter.extend(segments(&pts, stroke));
    }
}

/// A polyline as separate straight pieces. Drawn as one continuous path, egui's line joining
/// dropped some short pieces at sharp corners (lines missing until the shape was selected and
/// drawn dashed); separate pieces are always drawn.
pub fn segments(pts: &[Pos2], stroke: Stroke) -> Vec<egui::Shape> {
    pts.windows(2).map(|w| egui::Shape::line_segment([w[0], w[1]], stroke)).collect()
}

/// What a picture's texture was made from: its pixels' size and its size on the page, so a
/// picture changed in place gets a new texture. The workspace shows the picture as it is (the
/// person's choice); the Anteprima shows it as it will be engraved.
fn picture_key(s: &Shape) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Geometry::Image { w, h: hh, px_w, px_h, .. } = &s.geom {
        (w.to_bits(), hh.to_bits(), px_w, px_h).hash(&mut h);
    }
    h.finish()
}

fn draw_image(app: &mut BeamForge, ctx: &egui::Context, painter: &egui::Painter, xf: &Xf, index: usize, alpha: u8) {
    let s = &app.doc.shapes[index];
    let Geometry::Image { w, h, px_w, px_h, gray } = &s.geom else { return };
    let key = picture_key(s);
    if app.texture_keys.get(&s.id) != Some(&key) {
        app.textures.remove(&s.id);
    }
    let tex = app.textures.entry(s.id).or_insert_with(|| {
        app.texture_keys.insert(s.id, key);
        let img = egui::ColorImage::from_gray([*px_w as usize, *px_h as usize], gray);
        ctx.load_texture(format!("img{}", s.id), img, egui::TextureOptions::LINEAR)
    });
    let hw = w / 2.0;
    let hh = h / 2.0;
    let corners = [[-hw, hh], [hw, hh], [hw, -hh], [-hw, -hh]];
    let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
    let mut mesh = egui::Mesh::with_texture(tex.id());
    for (c, uv) in corners.iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex { pos: xf.to_screen(s.to_world(*c)), uv, color: Color32::from_white_alpha(alpha) });
    }
    mesh.indices.extend([0u32, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

pub fn canvas(app: &mut BeamForge, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let (full, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let area = Rect::from_min_max(full.min + vec2(RULER, RULER), full.max);
    if app.view.fit_pending || (app.view.auto_fit && area.size() != app.view.last_area) {
        app.view.last_area = area.size();
        fit_view(app, area);
        app.view.fit_pending = false;
    }
    let painter = ui.painter_at(full);
    let xf = Xf { origin: area.min + app.view.offset, zoom: app.view.zoom };
    let frame = ctx.cumulative_frame_nr();

    // ---------- input: zoom & pan ----------
    let hover = resp.hover_pos();
    if let Some(hp) = hover {
        let (scroll, zoom_d) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        let mut factor = zoom_d;
        if scroll.y.abs() > 0.0 {
            // "Inverti il senso dello zoom della rotellina del mouse" (on by default, as before)
            let dir = if app.prefs.invert_wheel_zoom { 1.0 } else { -1.0 };
            factor *= (dir * scroll.y * 0.0025).exp();
        }
        if (factor - 1.0).abs() > 1e-4 {
            let before = xf.to_world(hp);
            let nz = (app.view.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
            app.view.zoom = nz;
            app.view.auto_fit = false;
            // keep the point under the cursor fixed
            app.view.offset = vec2(hp.x - area.min.x - before[0] as f32 * nz, hp.y - area.min.y + before[1] as f32 * nz);
        }
        if scroll.x.abs() > 0.0 && zoom_d == 1.0 {
            app.view.offset.x += scroll.x;
        }
    }
    if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
        app.view.offset += resp.drag_delta();
        app.view.auto_fit = false;
    }
    let xf = Xf { origin: area.min + app.view.offset, zoom: app.view.zoom };
    let cursor = hover.map(|h| xf.to_world(h));
    app.cursor_world = cursor;
    let tol = 5.0 / app.view.zoom as f64;
    let mods = ctx.input(|i| i.modifiers);

    // ---------- tool interaction ----------
    let press = ctx.input(|i| i.pointer.press_origin()).map(|p| xf.to_world(p));
    let sel_box = app.doc.bbox_of(&app.selection);
    match app.tool {
        Tool::Select => {
            if resp.drag_started_by(egui::PointerButton::Primary) {
                if let Some(p0) = press {
                    let mut started = false;
                    // Shift turns the corner handles into rotate arrows (as the person asked)
                    if sel_box.is_valid() && mods.shift {
                        let pivot = sel_box.center();
                        let mid = xf.to_screen(pivot);
                        for hp in handle_points(&sel_box).iter().step_by(2) {
                            if (rotate_handle_pos(xf.to_screen(*hp), mid) - xf.to_screen(p0)).length() < ROTATE_GRAB_PX {
                                app.checkpoint(frame);
                                let start = (p0[1] - pivot[1]).atan2(p0[0] - pivot[0]);
                                app.drag = Drag::Rotate { orig: app.selected_shapes(), pivot, start, deg: 0.0 };
                                started = true;
                                break;
                            }
                        }
                    }
                    // while Shift shows the rotate arrows, corners don't resize
                    if sel_box.is_valid() && !started && !mods.shift {
                        for (i, hp) in handle_points(&sel_box).iter().enumerate() {
                            let sp = xf.to_screen(*hp);
                            let pp = xf.to_screen(p0);
                            if (sp - pp).length() < 7.0 {
                                app.checkpoint(frame);
                                app.drag = Drag::Scale { handle: i, orig: app.selected_shapes(), bbox: sel_box };
                                started = true;
                                break;
                            }
                        }
                    }
                    if !started {
                        let inside_sel = sel_box.is_valid() && p0[0] >= sel_box.min[0] && p0[0] <= sel_box.max[0] && p0[1] >= sel_box.min[1] && p0[1] <= sel_box.max[1];
                        match hit_test(app, p0, tol) {
                            Some(id) => {
                                if !app.selection.contains(&id) {
                                    let mut sel = if mods.shift { app.selection.clone() } else { Vec::new() };
                                    app.expand_into(&[id], &mut sel);
                                    app.selection = sel;
                                }
                                app.checkpoint(frame);
                                app.drag = Drag::Move { start: p0, orig: app.selected_shapes() };
                            }
                            None if inside_sel && !mods.shift => {
                                app.checkpoint(frame);
                                app.drag = Drag::Move { start: p0, orig: app.selected_shapes() };
                            }
                            None => app.drag = Drag::Rubber { start: p0 },
                        }
                    }
                }
            }
            if resp.dragged_by(egui::PointerButton::Primary) {
                if let Some(cur) = resp.interact_pointer_pos().map(|p| xf.to_world(p)) {
                    match &mut app.drag {
                        Drag::Rotate { orig, pivot, start, deg } => {
                            let a = (cur[1] - pivot[1]).atan2(cur[0] - pivot[0]) - *start;
                            // free rotation, as the person asked (the original's steps are in MECHANICS.md)
                            let d = a.to_degrees();
                            *deg = d;
                            let (orig, pivot) = (orig.clone(), *pivot);
                            app.checkpoint(frame);
                            for o in orig {
                                if let Some(s) = app.doc.find_mut(o.id) {
                                    *s = o.clone();
                                    s.rotate_about(d, pivot);
                                }
                            }
                        }
                        Drag::Move { start, orig } => {
                            let dx = cur[0] - start[0];
                            let dy = cur[1] - start[1];
                            let orig = orig.clone();
                            app.checkpoint(frame);
                            for o in orig {
                                if let Some(s) = app.doc.find_mut(o.id) {
                                    s.tx = o.tx + dx;
                                    s.ty = o.ty + dy;
                                }
                            }
                        }
                        Drag::Scale { handle, orig, bbox } => {
                            let (handle, orig, bbox) = (*handle, orig.clone(), *bbox);
                            let hp = handle_points(&bbox);
                            // Cmd scales about the centre (SelectionTool::Update 0x100777cbc,
                            // Ctrl bit 0x1a → anchor = min + size·0.5), else the opposite handle
                            let anchor = if mods.command { bbox.center() } else { hp[(handle + 4) % 8] };
                            // a side thinner than 0.0001 keeps factor 1 (0x100777f2c)
                            let factor = |c: f64, h: f64, a: f64| if (h - a).abs() > MIN_SCALE_SIDE { ((c - a) / (h - a)).max(0.001) } else { 1.0 };
                            let mut fx = if handle == 1 || handle == 5 { 1.0 } else { factor(cur[0], hp[handle][0], anchor[0]) };
                            let mut fy = if handle == 3 || handle == 7 { 1.0 } else { factor(cur[1], hp[handle][1], anchor[1]) };
                            let corner = handle % 2 == 0;
                            // proportional only while Option (Alt) is held, as the person asked
                            if corner && mods.alt {
                                let f = fx.max(fy);
                                fx = f;
                                fy = f;
                            }
                            app.checkpoint(frame);
                            for o in orig {
                                if let Some(s) = app.doc.find_mut(o.id) {
                                    *s = o.clone();
                                    s.scale_about(fx, fy, anchor);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            if resp.drag_stopped_by(egui::PointerButton::Primary) {
                if let Drag::Rubber { start } = app.drag {
                    if let Some(end) = resp.interact_pointer_pos().map(|p| xf.to_world(p)).or(cursor) {
                        let mut r = BBox::empty();
                        r.add(start);
                        r.add(end);
                        let crossing = end[0] < start[0];
                        let ids: Vec<u64> = app
                            .doc
                            .shapes
                            .iter()
                            .filter(|s| app.doc.layers[s.layer].show)
                            .filter(|s| if crossing { r.intersects(&s.bbox()) } else { r.contains(&s.bbox()) })
                            .map(|s| s.id)
                            .collect();
                        let mut sel = if mods.shift { app.selection.clone() } else { Vec::new() };
                        app.expand_into(&ids, &mut sel);
                        app.selection = sel;
                    }
                }
                app.drag = Drag::None;
            }
            if resp.clicked() {
                if let Some(p) = cursor {
                    match hit_test(app, p, tol) {
                        Some(id) => {
                            let members = app.group_members(id);
                            if mods.shift {
                                if app.selection.contains(&id) {
                                    app.selection.retain(|x| !members.contains(x));
                                } else {
                                    let mut sel = app.selection.clone();
                                    app.expand_into(&[id], &mut sel);
                                    app.selection = sel;
                                }
                            } else {
                                app.selection = members;
                            }
                        }
                        None => {
                            if !mods.shift {
                                app.selection.clear();
                            }
                        }
                    }
                }
            }
            if resp.double_clicked() {
                if app.selection.len() == 1 {
                    app.right_tab = crate::app::RightTab::Shape;
                }
            }
        }
        Tool::Rect | Tool::Ellipse | Tool::Polygon | Tool::Triangle | Tool::Pentagon | Tool::Octagon | Tool::Star | Tool::DualStar => {
            if resp.drag_started_by(egui::PointerButton::Primary) {
                if let Some(p0) = press {
                    app.drag = Drag::Create { start: p0 };
                }
            }
            if resp.drag_stopped_by(egui::PointerButton::Primary) {
                if let Drag::Create { start } = app.drag {
                    if let Some(end) = resp.interact_pointer_pos().map(|p| xf.to_world(p)).or(cursor) {
                        create_shape(app, start, end, square_held(&mods), mods.command);
                    }
                }
                app.drag = Drag::None;
            }
        }
        Tool::Line => {
            if resp.clicked() {
                if let Some(p) = line_cursor(app, cursor, mods.shift) {
                    if app.line_pts.len() >= 3 && xf.to_screen(app.line_pts[0]).distance(xf.to_screen(p)) < 8.0 {
                        finish_line(app, true);
                    } else {
                        app.line_pts.push(p);
                    }
                }
            }
            if resp.double_clicked() && app.line_pts.len() > 1 {
                app.line_pts.pop();
                finish_line(app, false);
            }
            if resp.secondary_clicked() && app.line_pts.len() > 1 {
                finish_line(app, false);
            }
        }
        Tool::Text => {
            if resp.clicked() {
                if let Some(p) = cursor {
                    let id = app.make_text_shape(p);
                    app.selection = vec![id];
                    app.tool = Tool::Select;
                    app.right_tab = crate::app::RightTab::Shape;
                }
            }
        }
        Tool::PositionLaser => {
            if resp.clicked() {
                if let Some(p) = cursor {
                    let mut lines = vec![crate::gcode::jog_line(true, p[0], p[1], app.jog_speed)];
                    lines.extend(crate::gcode::position_pulse_lines(app.doc.device.s_max));
                    app.send_lines(lines);
                    app.status = crate::i18n::trf("Sposto il laser a {}, {}", &[&format!("{:.1}", p[0]), &format!("{:.1}", p[1])]);
                }
            }
        }
        Tool::Tabs => {
            // Manual tab tool (TabTool BeginDrag 0x1007584ec / EndDrag 0x10075a0d8 / double-click 0x100758c1c):
            // click on an outline adds a tab, Ctrl/Cmd-click or double-click on a tab removes it,
            // dragging a tab slides it along the outline. Tab hit radius 5 px, outline search 25 px
            // (FindTabPoint 0x10075866c passes 25.0 to GetPrimPointAt).
            let find_tab = |app: &BeamForge, p: P2| -> Option<(usize, usize)> {
                for (si, s) in app.doc.shapes.iter().enumerate() {
                    for (ti, q) in s.tab_points().iter().enumerate() {
                        if (q[0] - p[0]).hypot(q[1] - p[1]) <= tol {
                            return Some((si, ti));
                        }
                    }
                }
                None
            };
            let nearest = |app: &BeamForge, p: P2| -> Option<(usize, usize, f64)> {
                let mut best: Option<(usize, usize, f64, f64)> = None;
                for (si, s) in app.doc.shapes.iter().enumerate() {
                    if let Some((pi, t, d)) = s.nearest_on_outline(p) {
                        if d <= 25.0 / app.view.zoom as f64 && best.is_none_or(|b| d < b.3) {
                            best = Some((si, pi, t, d));
                        }
                    }
                }
                best.map(|b| (b.0, b.1, b.2))
            };
            if resp.drag_started_by(egui::PointerButton::Primary) {
                if let Some(p0) = press {
                    if let Some(hit) = find_tab(app, p0) {
                        app.checkpoint_now();
                        app.tab_drag = Some(hit);
                    }
                }
            }
            if let Some((si, ti)) = app.tab_drag {
                if resp.dragged_by(egui::PointerButton::Primary) {
                    if let Some(p) = resp.interact_pointer_pos().map(|p| xf.to_world(p)) {
                        if let Some(s) = app.doc.shapes.get_mut(si) {
                            if let Some((pi, t, _)) = s.nearest_on_outline(p) {
                                if let Some(tab) = s.tabs.get_mut(ti) {
                                    *tab = (pi, t);
                                    app.dirty = true;
                                }
                            }
                        }
                    }
                }
                if resp.drag_stopped() {
                    app.tab_drag = None;
                }
            } else if resp.double_clicked() || (resp.clicked() && mods.command) {
                if let Some(p) = cursor {
                    if let Some((si, ti)) = find_tab(app, p) {
                        app.checkpoint_now();
                        app.doc.shapes[si].tabs.remove(ti);
                        app.status = crate::i18n::tr("Linguetta rimossa").to_string();
                    }
                }
            } else if resp.clicked() {
                if let Some(p) = cursor {
                    if find_tab(app, p).is_none() {
                        if let Some((si, pi, t)) = nearest(app, p) {
                            app.checkpoint_now();
                            let layer = app.doc.shapes[si].layer;
                            app.doc.shapes[si].tabs.push((pi, t));
                            if let Some(cs) = app.doc.layers.get_mut(layer) {
                                cs.tabs_enabled = true;
                                cs.manual_tabs = true;
                            }
                            app.status = crate::i18n::tr("Linguetta aggiunta").to_string();
                        }
                    }
                }
            }
        }
        Tool::Measure => {
            // hover: the piece under the pointer within the click tolerance; drag: a free line
            // (MeasureTool::Update 0x10076a788), Shift keeps it on an axis
            app.measure.open = true;
            if resp.dragged_by(egui::PointerButton::Primary) {
                if let (Some(a), Some(b)) = (press, resp.interact_pointer_pos().map(|p| xf.to_world(p))) {
                    let b = if mods.shift { crate::measure::axis_snap(a, b) } else { b };
                    app.measure.line = Some((a, b));
                    app.measure.shape = None;
                    app.measure.prim = None;
                }
            } else if let Some(p) = cursor {
                if let Some((id, i)) = crate::measure::hit_prim(app, p, tol) {
                    app.measure.shape = Some(id);
                    app.measure.prim = Some(i);
                    app.measure.line = None;
                }
            }
        }
    }

    // ---------- drawing ----------
    painter.rect_filled(area, 0.0, crate::theme::p().canvas_out);
    let d = &app.doc.device;
    let ws = Rect::from_two_pos(xf.to_screen([0.0, 0.0]), xf.to_screen([d.width, d.height]));
    painter.rect_filled(ws, 0.0, crate::theme::p().canvas_ws);
    // grid
    // "Visualizza spaziatura griglia" (Impostazioni > Unità e Griglie) is the finest step shown
    let step = nice_step(app.view.zoom).max(app.prefs.grid_spacing.max(0.1));
    let major = step * 10.0;
    let minor_color = minor_grid_color((major as f32) * app.view.zoom);
    let mut x = 0.0;
    while x <= d.width + 1e-6 {
        let is_major = (x / major).fract().abs() < 1e-9;
        let c = if is_major { Some(crate::theme::p().grid_major) } else { minor_color };
        if let Some(c) = c {
            painter.line_segment([xf.to_screen([x, 0.0]), xf.to_screen([x, d.height])], Stroke::new(1.0, c));
        }
        x += step;
    }
    let mut y = 0.0;
    while y <= d.height + 1e-6 {
        let is_major = (y / major).fract().abs() < 1e-9;
        let c = if is_major { Some(crate::theme::p().grid_major) } else { minor_color };
        if let Some(c) = c {
            painter.line_segment([xf.to_screen([0.0, y]), xf.to_screen([d.width, y])], Stroke::new(1.0, c));
        }
        y += step;
    }
    painter.rect_stroke(ws, 0.0, Stroke::new(1.0, Color32::from_rgb(90, 96, 108)), StrokeKind::Outside);
    // "Mostra la croce al centro dell'area di lavoro"
    if app.prefs.show_center_cross {
        let c = ws.center();
        let st = Stroke::new(1.0, Color32::from_rgb(220, 60, 60));
        painter.line_segment([pos2(ws.left(), c.y), pos2(ws.right(), c.y)], st);
        painter.line_segment([pos2(c.x, ws.top()), pos2(c.x, ws.bottom())], st);
    }

    // shapes
    // pictures are drawn in place: copying the shapes would copy every picture's pixels each frame
    for i in 0..app.doc.shapes.len() {
        let s = &app.doc.shapes[i];
        if app.doc.layers[s.layer].show && matches!(s.geom, Geometry::Image { .. }) {
            // a picture's outline only shows while it is selected
            let outline = app.selection.contains(&s.id).then(|| (s.world_paths(), display_color(s.layer)));
            draw_image(app, &ctx, &painter, &xf, i, 255);
            if let Some((paths, col)) = outline {
                for pl in &paths {
                    draw_polyline(&painter, &xf, pl, Stroke::new(1.5, col), true);
                }
            }
        }
    }
    let shapes: Vec<Shape> = app.doc.shapes.iter().filter(|s| !matches!(s.geom, Geometry::Image { .. })).cloned().collect();
    for s in &shapes {
        let cs = &app.doc.layers[s.layer];
        if !cs.show {
            continue;
        }
        let selected = app.selection.contains(&s.id);
        let col = display_color(s.layer);
        let stroke = Stroke::new(if selected { 1.5 } else { 1.0 }, col);
        let paths = s.world_paths();
        if matches!(s.geom, Geometry::Image { .. }) && !selected {
            continue;
        }
        for pl in &paths {
            draw_polyline(&painter, &xf, pl, stroke, selected);
        }
    }

    // selected objects: groups get their own box, the alignment reference (last picked) is highlighted
    let items = app.selection_items();
    if app.tool == Tool::Select && (items.len() > 1 || items.first().is_some_and(|i| i.len() > 1)) {
        for (k, it) in items.iter().enumerate() {
            let b = app.doc.bbox_of(it);
            if !b.is_valid() {
                continue;
            }
            let r = Rect::from_two_pos(xf.to_screen(b.min), xf.to_screen(b.max)).expand(1.0);
            let reference = items.len() > 1 && k + 1 == items.len();
            let col = if reference { Color32::from_rgb(90, 220, 110) } else { Color32::from_rgb(120, 130, 150) };
            painter.rect_stroke(r, 0.0, Stroke::new(if reference { 1.5 } else { 1.0 }, col), StrokeKind::Outside);
        }
    }
    // live preview of the Offset dialog, dashed in the accent colour
    for set in &app.offset_preview {
        for pl in set {
            let mut pts: Vec<Pos2> = pl.pts.iter().map(|p| xf.to_screen(*p)).collect();
            if pl.closed && !pts.is_empty() {
                pts.push(pts[0]);
            }
            painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.5, crate::theme::ACCENT), 5.0, 3.0));
        }
    }
    // selection box and handles
    let sel_box = app.doc.bbox_of(&app.selection);
    if sel_box.is_valid() && app.tool == Tool::Select {
        let r = Rect::from_two_pos(xf.to_screen(sel_box.min), xf.to_screen(sel_box.max)).expand(2.0);
        painter.extend(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.0, crate::theme::ACCENT), 3.0, 3.0));
        let rotating = matches!(app.drag, Drag::Rotate { .. }) || (mods.shift && matches!(app.drag, Drag::None));
        let mid = r.center();
        for (i, hp) in handle_points(&sel_box).into_iter().enumerate() {
            let c = xf.to_screen(hp);
            if rotating && i % 2 == 0 {
                draw_rotate_arrow(&painter, c, rotate_handle_pos(c, mid));
                continue;
            }
            let hr = Rect::from_center_size(c, vec2(7.0, 7.0));
            painter.rect_filled(hr, 1.0, crate::theme::ACCENT);
            painter.rect_stroke(hr, 1.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
        }
        let c = xf.to_screen(sel_box.center());
        painter.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.0, Color32::LIGHT_GRAY));
        painter.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.0, Color32::LIGHT_GRAY));
        // reference point chosen in the 9-dot grid: position, size and rotation work from here
        let a = xf.to_screen(sel_box.anchor(app.anchor));
        painter.circle_filled(a, 4.5, crate::theme::ACCENT);
        painter.circle_stroke(a, 6.5, Stroke::new(1.5, Color32::WHITE));
    }

    // the turn so far, beside the pointer ("%1 deg", SelectionTool::Update 0x100777938)
    if let (Drag::Rotate { deg, .. }, Some(p)) = (&app.drag, resp.interact_pointer_pos().or(hover)) {
        painter.text(p + vec2(16.0, 16.0), egui::Align2::LEFT_TOP, format!("{:.1} deg", wrap_degrees(*deg)), egui::FontId::proportional(12.0), crate::theme::p().text);
    }

    // previews of tools in progress
    let accent = display_color(app.current_layer);
    match (&app.drag, cursor.or_else(|| resp.interact_pointer_pos().map(|p| xf.to_world(p)))) {
        (Drag::Rubber { start }, Some(cur)) => {
            let r = Rect::from_two_pos(xf.to_screen(*start), xf.to_screen(cur));
            let crossing = cur[0] < start[0];
            let col = if crossing { Color32::from_rgb(40, 160, 40) } else { Color32::from_rgb(40, 90, 220) };
            painter.rect_filled(r, 0.0, col.gamma_multiply(0.12));
            painter.extend(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.0, col), 4.0, 3.0));
        }
        (Drag::Create { start }, Some(cur)) => {
            // the preview follows the same 1:1 and from-centre rules as the shape that will be made
            let (wa, wb) = drag_box(*start, cur, square_held(&mods), mods.command);
            let (a, b) = (xf.to_screen(wa), xf.to_screen(wb));
            let hint = if square_held(&mods) { crate::i18n::tr("Rapporto 1:1") } else { crate::i18n::tr("Option: rapporto 1:1 · Cmd: dal centro") };
            painter.text(b + vec2(14.0, 14.0), egui::Align2::LEFT_TOP, hint, egui::FontId::proportional(11.0), crate::theme::p().text_dim);
            match app.tool {
                Tool::Rect => {
                    painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, Stroke::new(1.0, accent), StrokeKind::Middle);
                }
                Tool::Ellipse => {
                    let r = Rect::from_two_pos(a, b);
                    let pts: Vec<Pos2> = (0..72)
                        .map(|i| {
                            let t = i as f32 / 72.0 * std::f32::consts::TAU;
                            pos2(r.center().x + r.width() / 2.0 * t.cos(), r.center().y + r.height() / 2.0 * t.sin())
                        })
                        .collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(1.0, accent)));
                }
                Tool::Star | Tool::DualStar => {
                    let r = Rect::from_two_pos(a, b);
                    let middle = (app.tool == Tool::DualStar).then_some(DUAL_STAR_MIDDLE);
                    // world Y is up, the screen's is down
                    let pts: Vec<Pos2> = star_pts(STAR_POINTS, r.width() as f64 / 2.0, r.height() as f64 / 2.0, STAR_INNER, middle)
                        .into_iter()
                        .map(|p| pos2(r.center().x + p[0] as f32, r.center().y - p[1] as f32))
                        .collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(1.0, accent)));
                }
                t if t.is_ngon() => {
                    let r = Rect::from_two_pos(a, b);
                    let n = t.fixed_sides().unwrap_or(app.polygon_sides);
                    // world Y is up, the screen's is down
                    let pts: Vec<Pos2> = polygon_box_pts(n, r.width() as f64 / 2.0, r.height() as f64 / 2.0)
                        .into_iter()
                        .map(|p| pos2(r.center().x + p[0] as f32, r.center().y - p[1] as f32))
                        .collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(1.0, accent)));
                }
                _ => {}
            }
        }
        _ => {}
    }
    if app.tool == Tool::Line && !app.line_pts.is_empty() {
        let mut pts: Vec<Pos2> = app.line_pts.iter().map(|p| xf.to_screen(*p)).collect();
        if let Some(c) = line_cursor(app, cursor, mods.shift) {
            pts.push(xf.to_screen(c));
        }
        painter.add(egui::Shape::line(pts, Stroke::new(1.0, accent)));
        let f = xf.to_screen(app.line_pts[0]);
        painter.circle_stroke(f, 4.0, Stroke::new(1.0, accent));
    }

    // manual tabs: 10 px circles on the outline (TabTool::DrawTab 0x10075abcc)
    for s in &app.doc.shapes {
        let manual = app.doc.layers.get(s.layer).is_some_and(|cs| cs.tabs_enabled && cs.manual_tabs);
        if s.tabs.is_empty() || !(manual || app.tool == Tool::Tabs) {
            continue;
        }
        for q in s.tab_points() {
            let c = xf.to_screen(q);
            painter.circle_filled(c, 5.0, Color32::from_rgba_unmultiplied(255, 140, 0, 160));
            painter.circle_stroke(c, 5.0, Stroke::new(1.0, Color32::from_rgb(255, 200, 0)));
        }
    }

    if app.tool == Tool::Measure {
        crate::measure::draw(app, &painter, &xf);
    }

    // laser head position
    if let Some(hp) = app.head_pos {
        let c = xf.to_screen(hp);
        let g = Color32::from_rgb(90, 220, 110);
        painter.circle_stroke(c, 6.0, Stroke::new(1.5, g));
        painter.line_segment([c - vec2(10.0, 0.0), c + vec2(10.0, 0.0)], Stroke::new(1.0, g));
        painter.line_segment([c - vec2(0.0, 10.0), c + vec2(0.0, 10.0)], Stroke::new(1.0, g));
    }
    // job origin marker when not absolute
    if app.job_opts.start_from != crate::gcode::StartFrom::AbsoluteCoords {
        let jb = if app.job_opts.selected_only { app.doc.bbox_of(&app.selection) } else { app.doc.bbox_all() };
        if jb.is_valid() {
            let c = xf.to_screen(jb.anchor(app.job_opts.job_origin));
            painter.circle_filled(c, 3.5, Color32::from_rgb(0, 150, 0));
        }
    }

    // ---------- rulers ----------
    let top = Rect::from_min_max(full.min, pos2(full.max.x, full.min.y + RULER));
    let left = Rect::from_min_max(full.min, pos2(full.min.x + RULER, full.max.y));
    let rbg = crate::theme::p().ruler_bg;
    painter.rect_filled(top, 0.0, rbg);
    painter.rect_filled(left, 0.0, rbg);
    let tick = crate::theme::p().ruler_tick;
    let font = egui::FontId::proportional(9.0);
    let lstep = nice_step(app.view.zoom * 0.2).max(1.0);
    let w0 = xf.to_world(pos2(area.min.x, area.max.y));
    let w1 = xf.to_world(pos2(area.max.x, area.min.y));
    let mut x = (w0[0] / lstep).floor() * lstep;
    while x <= w1[0] {
        let sx = xf.to_screen([x, 0.0]).x;
        if sx >= area.min.x {
            painter.line_segment([pos2(sx, top.max.y - 7.0), pos2(sx, top.max.y)], Stroke::new(1.0, tick));
            painter.text(pos2(sx + 2.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, format!("{}", x as i64), font.clone(), tick);
        }
        let half = xf.to_screen([x + lstep / 2.0, 0.0]).x;
        if half >= area.min.x {
            painter.line_segment([pos2(half, top.max.y - 3.0), pos2(half, top.max.y)], Stroke::new(1.0, tick));
        }
        x += lstep;
    }
    let mut y = (w0[1] / lstep).floor() * lstep;
    while y <= w1[1] {
        let sy = xf.to_screen([0.0, y]).y;
        if sy >= area.min.y {
            painter.line_segment([pos2(left.max.x - 7.0, sy), pos2(left.max.x, sy)], Stroke::new(1.0, tick));
            painter.text(pos2(left.min.x + 1.0, sy - 1.0), egui::Align2::LEFT_BOTTOM, format!("{}", y as i64), font.clone(), tick);
        }
        y += lstep;
    }
    if let Some(h) = hover {
        let c = crate::theme::ACCENT;
        painter.line_segment([pos2(h.x, top.min.y), pos2(h.x, top.max.y)], Stroke::new(1.0, c));
        painter.line_segment([pos2(left.min.x, h.y), pos2(left.max.x, h.y)], Stroke::new(1.0, c));
    }
    painter.rect_filled(Rect::from_min_size(full.min, Vec2::splat(RULER)), 0.0, crate::theme::p().ruler_corner);
    painter.text(full.min + vec2(3.0, 4.0), egui::Align2::LEFT_TOP, "mm", font, tick);

    // cursor shape
    if hover.is_some() {
        let icon = match app.tool {
            Tool::Select => {
                if matches!(app.drag, Drag::Move { .. }) {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Default
                }
            }
            Tool::Text => egui::CursorIcon::Text,
            _ => egui::CursorIcon::Crosshair,
        };
        ctx.set_cursor_icon(icon);
    }
}

#[cfg(test)]
mod square_tests {
    use super::*;

    #[test]
    fn option_holds_one_to_one() {
        let alt = egui::Modifiers { alt: true, ..Default::default() };
        assert!(square_held(&alt));
        assert!(!square_held(&egui::Modifiers::default()));
        assert!(!square_held(&egui::Modifiers { command: true, ctrl: true, ..Default::default() }));
        assert_eq!(constrain_corner([0.0, 0.0], [30.0, 10.0], true), [30.0, 30.0]);
        assert_eq!(constrain_corner([0.0, 0.0], [-5.0, 20.0], true), [-20.0, 20.0]);
        assert_eq!(constrain_corner([0.0, 0.0], [30.0, 10.0], false), [30.0, 10.0]);
        // Cmd: the press point is the centre
        assert_eq!(drag_box([10.0, 10.0], [20.0, 15.0], false, true), ([0.0, 5.0], [20.0, 15.0]));
        assert_eq!(drag_box([10.0, 10.0], [20.0, 15.0], true, true), ([0.0, 0.0], [20.0, 20.0]));
    }
}
