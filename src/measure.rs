//! Measure tool (ruler), like the original's MeasureTool / MeasureToolDialog:
//! hover a shape to read its nodes, lines, curves, perimeter, size, closed/open and area, plus the
//! segment under the pointer; drag to measure a free line (Shift keeps it on an axis).
//! Rules: MeasureTool::Update 0x10076a788, MeasureToolDialog::UpdateDisplay 0x10089ebc0,
//! AsUnits 0x1008a1934, on_pbUnits_clicked 0x1008a1988 (see MECHANICS.md).

use crate::app::BeamForge;
use crate::i18n::{tr, trf};
use crate::model::*;
use eframe::egui::{self, Color32, vec2};

/// One drawn piece of a shape: a straight line or a curve (with its arc radius/centre when it is
/// a circular arc).
#[derive(Clone, Debug)]
pub struct Prim {
    pub pts: Vec<P2>,
    pub curve: bool,
    pub arc: Option<(f64, P2)>,
}

#[derive(Default)]
pub struct MeasureState {
    pub open: bool,
    pub shape: Option<u64>,
    pub prim: Option<usize>,
    pub line: Option<(P2, P2)>,
    /// false = millimetres (the original starts in the design units)
    pub inches: bool,
}

fn len(pts: &[P2]) -> f64 {
    pts.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum()
}

/// Length of a piece: circular arcs (all quarter turns here) are exact, r·π/2.
fn prim_len(p: &Prim) -> f64 {
    match p.arc {
        Some((r, _)) => r * std::f64::consts::FRAC_PI_2,
        None => len(&p.pts),
    }
}

fn arc_pts(c: P2, r: f64, a0: f64, a1: f64, n: usize) -> Vec<P2> {
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians();
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}

/// The shape's pieces in world coordinates.
pub fn prims(s: &Shape) -> Vec<Prim> {
    let w = |pts: Vec<P2>| -> Vec<P2> { pts.into_iter().map(|p| s.to_world(p)).collect() };
    let circle_r = |rx: f64, ry: f64| -> Option<f64> {
        let a = (rx * s.sx).abs();
        let b = (ry * s.sy).abs();
        if (a - b).abs() < 1e-6 { Some(a) } else { None }
    };
    let line = |a: P2, b: P2| Prim { pts: w(vec![a, b]), curve: false, arc: None };
    match &s.geom {
        Geometry::Rect { w: rw, h: rh, radius } => {
            let (hw, hh) = (rw / 2.0, rh / 2.0);
            let r = radius.max(0.0).min(hw).min(hh);
            if r <= 1e-6 {
                // top-left, top-right, bottom-right, bottom-left: clockwise, as in reference/picture-2
                let c = [[-hw, hh], [hw, hh], [hw, -hh], [-hw, -hh]];
                (0..4).map(|i| line(c[i], c[(i + 1) % 4])).collect()
            } else {
                let mut v = Vec::new();
                let corners = [([hw - r, -hh + r], -90.0), ([hw - r, hh - r], 0.0), ([-hw + r, hh - r], 90.0), ([-hw + r, -hh + r], 180.0)];
                for (k, (c, a0)) in corners.iter().enumerate() {
                    let pts = arc_pts(*c, r, *a0, a0 + 90.0, 12);
                    let rr = circle_r(r, r).map(|rad| (rad, s.to_world(*c)));
                    let end = *pts.last().unwrap();
                    v.push(Prim { pts: w(pts), curve: true, arc: rr });
                    let next = corners[(k + 1) % 4];
                    let start_next = arc_pts(next.0, r, next.1, next.1 + 90.0, 1)[0];
                    if (end[0] - start_next[0]).abs() > 1e-9 || (end[1] - start_next[1]).abs() > 1e-9 {
                        v.push(line(end, start_next));
                    }
                }
                // put the bottom edge first, like a rectangle's first line
                v.rotate_right(1);
                v
            }
        }
        Geometry::Ellipse { rx, ry } => {
            let arc = circle_r(*rx, *ry).map(|rad| (rad, [s.tx, s.ty]));
            (0..4)
                .map(|q| {
                    let pts: Vec<P2> = (0..=96)
                        .map(|i| {
                            let a = ((q as f64) * 90.0 + i as f64 * 90.0 / 96.0).to_radians();
                            [rx * a.cos(), ry * a.sin()]
                        })
                        .collect();
                    Prim { pts: w(pts), curve: true, arc }
                })
                .collect()
        }
        Geometry::Image { .. } | Geometry::Polygon { .. } | Geometry::Star { .. } | Geometry::Path { .. } | Geometry::Text { .. } => {
            let mut v = Vec::new();
            for pl in s.geom.local_paths() {
                let n = pl.pts.len();
                if n < 2 {
                    continue;
                }
                let count = if pl.closed { n } else { n - 1 };
                for i in 0..count {
                    v.push(line(pl.pts[i], pl.pts[(i + 1) % n]));
                }
            }
            v
        }
    }
}

pub struct ShapeInfo {
    pub nodes: usize,
    pub lines: usize,
    pub curves: usize,
    pub perimeter: f64,
    pub width: f64,
    pub height: f64,
    pub closed: bool,
    pub area: f64,
}

pub fn shape_info(s: &Shape) -> ShapeInfo {
    let ps = prims(s);
    let paths = s.world_paths();
    let closed = !paths.is_empty() && paths.iter().all(|p| p.closed);
    let open_paths = paths.iter().filter(|p| !p.closed).count();
    // signed areas add up, so holes drawn the other way round are taken away (PathShape::ComputeArea)
    let mut signed = 0.0;
    for p in paths.iter().filter(|p| p.closed && p.pts.len() > 2) {
        let n = p.pts.len();
        for i in 0..n {
            let a = p.pts[i];
            let b = p.pts[(i + 1) % n];
            signed += a[0] * b[1] - b[0] * a[1];
        }
    }
    let b = s.bbox();
    ShapeInfo {
        nodes: ps.len() + open_paths,
        lines: ps.iter().filter(|p| !p.curve).count(),
        curves: ps.iter().filter(|p| p.curve).count(),
        perimeter: ps.iter().map(prim_len).sum(),
        width: b.w(),
        height: b.h(),
        closed,
        area: (signed / 2.0).abs(),
    }
}

/// Nearest piece of any visible shape within `tol` (mm).
pub fn hit_prim(app: &BeamForge, p: P2, tol: f64) -> Option<(u64, usize)> {
    let mut best: Option<(u64, usize, f64)> = None;
    for s in app.doc.shapes.iter().rev() {
        if !app.doc.layers[s.layer].show {
            continue;
        }
        let b = s.bbox();
        if p[0] < b.min[0] - tol || p[0] > b.max[0] + tol || p[1] < b.min[1] - tol || p[1] > b.max[1] + tol {
            continue;
        }
        for (i, pr) in prims(s).iter().enumerate() {
            for w in pr.pts.windows(2) {
                let d = dist_seg(p, w[0], w[1]);
                if d <= tol && best.is_none_or(|bb| d < bb.2) {
                    best = Some((s.id, i, d));
                }
            }
        }
    }
    best.map(|b| (b.0, b.1))
}

/// Shift snaps a dragged measuring line to the nearest of horizontal, vertical or 45° diagonal
/// (MeasureTool::AxisSnap 0x10076a6ac: the diagonal uses min(|dx|, |dy|) on both axes, signs kept).
pub fn axis_snap(a: P2, b: P2) -> P2 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let m = dx.abs().min(dy.abs());
    let candidates = [[dx, 0.0], [0.0, dy], [m * dx.signum(), m * dy.signum()]];
    let best = candidates
        .iter()
        .min_by(|p, q| {
            let dp = (p[0] - dx).powi(2) + (p[1] - dy).powi(2);
            let dq = (q[0] - dx).powi(2) + (q[1] - dy).powi(2);
            dp.partial_cmp(&dq).unwrap()
        })
        .unwrap();
    [a[0] + best[0], a[1] + best[1]]
}

fn fmt(v: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v);
    let sep = crate::i18n::decimal_char();
    if sep == '.' { s } else { s.replace('.', &sep.to_string()) }
}

pub fn window(app: &mut BeamForge, ctx: &egui::Context) {
    if !app.measure.open {
        return;
    }
    let st = &app.measure;
    // mm: 2 decimals, inches: 4 (UpdateDisplay 0x10089ecd8); inches = mm / 25.4 (AsUnits 0x1008a1934)
    let (k, dec, unit) = if st.inches { (1.0 / 25.4, 4, "in") } else { (1.0, 2, "mm") };
    let lenu = |v: f64| format!("{} {}", fmt(v * k, dec), unit);
    let pt = |p: P2| trf("{}x, {}y", &[&fmt(p[0] * k, dec), &fmt(p[1] * k, dec)]);
    let shape = st.shape.and_then(|id| app.doc.find(id)).cloned();
    let info = shape.as_ref().map(shape_info);
    // the segment shown on the right: the hovered piece, or the dragged line
    let seg: Option<(P2, P2, f64, Option<(f64, P2)>)> = match (&st.line, &shape, st.prim) {
        (Some((a, b)), _, _) => Some((*a, *b, len(&[*a, *b]), None)),
        (None, Some(s), Some(i)) => prims(s).get(i).map(|p| (p.pts[0], *p.pts.last().unwrap(), prim_len(p), p.arc)),
        _ => None,
    };
    let mut open = true;
    let mut ok = false;
    let mut toggle = false;
    crate::theme::centered(ctx, "measure", egui::Window::new(tr("Misurazione"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false))
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                egui::Grid::new("measure_shape").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, l: &str, v: String| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(egui::RichText::new(l).color(crate::theme::TEXT_DIM)));
                        ui.label(v);
                        ui.end_row();
                    };
                    let i = info.as_ref();
                    row(ui, tr("Nodi:"), i.map(|i| i.nodes.to_string()).unwrap_or_default());
                    row(ui, tr("Linee:"), i.map(|i| i.lines.to_string()).unwrap_or_default());
                    row(ui, tr("Curve:"), i.map(|i| i.curves.to_string()).unwrap_or_default());
                    row(ui, tr("Lunghezza del perimetro:"), i.map(|i| lenu(i.perimeter)).unwrap_or_default());
                    row(ui, tr("Larghezza"), i.map(|i| lenu(i.width)).unwrap_or_default());
                    row(ui, tr("Altezza"), i.map(|i| lenu(i.height)).unwrap_or_default());
                    row(ui, tr("Chiuso/Aperto:"), i.map(|i| if i.closed { tr("Chiuso") } else { tr("Aperto") }.to_string()).unwrap_or_default());
                    row(ui, tr("Area:"), i.map(|i| format!("{} {}^2", fmt(i.area * k * k, dec), unit)).unwrap_or_default());
                });
                ui.add_space(16.0);
                egui::Grid::new("measure_seg").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, l: &str, v: String| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(egui::RichText::new(l).color(crate::theme::TEXT_DIM)));
                        ui.label(v);
                        ui.end_row();
                    };
                    let s = seg.as_ref();
                    row(ui, tr("Lunghezza del segmento:"), s.map(|s| lenu(s.2)).unwrap_or_default());
                    row(ui, tr("Punto iniziale:"), s.map(|s| pt(s.0)).unwrap_or_default());
                    row(ui, tr("Punto finale:"), s.map(|s| pt(s.1)).unwrap_or_default());
                    row(ui, tr("Differenza:"), s.map(|s| pt([s.1[0] - s.0[0], s.1[1] - s.0[1]])).unwrap_or_default());
                    // angle = atan2(dy, dx) in degrees, 1 decimal, only for pieces longer than 0.001
                    let ang = s.filter(|s| s.2 > 0.001).map(|s| format!("{}°", fmt((s.1[1] - s.0[1]).atan2(s.1[0] - s.0[0]).to_degrees(), 1)));
                    row(ui, tr("Angolo:"), ang.unwrap_or_default());
                    row(ui, tr("Raggio dell'arco:"), s.and_then(|s| s.3).map(|a| lenu(a.0)).unwrap_or_default());
                    row(ui, tr("Centro dell'arco:"), s.and_then(|s| s.3).map(|a| pt(a.1)).unwrap_or_default());
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(unit).min_size(vec2(130.0, 24.0))).on_hover_text(tr("Cambia unità (mm / pollici)")).clicked() {
                    toggle = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(tr("OK")).fill(Color32::from_rgb(170, 70, 20)).min_size(vec2(80.0, 24.0))).clicked() {
                        ok = true;
                    }
                });
            });
            if info.is_none() && seg.is_none() {
                ui.label(egui::RichText::new(tr("Passa sopra una forma o trascina per misurare.")).color(crate::theme::TEXT_DIM));
            }
        });
    if toggle {
        app.measure.inches = !app.measure.inches;
    }
    if ok || !open {
        // closing the window ends the tool (MainWindow::action_measureToolClosed)
        app.measure = MeasureState { inches: app.measure.inches, ..Default::default() };
        app.tool = crate::app::Tool::Select;
    }
}

/// Draw the measured shape (dashed green) with its hovered piece in magenta, or the dragged line.
pub fn draw(app: &BeamForge, painter: &egui::Painter, xf: &crate::canvas::Xf) {
    let st = &app.measure;
    // pens from MeasureTool::Draw 0x10076ac64: shape green (0,255,0) alpha 0x60, width 3;
    // hovered piece and dragged line magenta (255,0,255) alpha 0xC0, widths 3 and 2
    // (QColor constants 0x101919a28 / 0x101919a30)
    let green = Color32::from_rgba_unmultiplied(0, 255, 0, 0x60);
    let magenta = Color32::from_rgba_unmultiplied(255, 0, 255, 0xC0);
    if let Some(s) = st.shape.and_then(|id| app.doc.find(id)) {
        for (i, p) in prims(s).iter().enumerate() {
            let pts: Vec<egui::Pos2> = p.pts.iter().map(|q| xf.to_screen(*q)).collect();
            let col = if Some(i) == st.prim { magenta } else { green };
            painter.add(egui::Shape::line(pts, egui::Stroke::new(3.0, col)));
        }
    }
    if let Some((a, b)) = st.line {
        let (pa, pb) = (xf.to_screen(a), xf.to_screen(b));
        painter.line_segment([pa, pb], egui::Stroke::new(2.0, magenta));
        painter.circle_filled(pa, 3.0, crate::theme::ACCENT);
        painter.circle_filled(pb, 3.0, crate::theme::ACCENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_like_the_screenshot() {
        // 80 × 54 mm rectangle: 4 nodes, 4 lines, perimeter 268, area 4320 (reference/picture-2)
        let s = Shape::new(1, Geometry::Rect { w: 80.0, h: 54.0, radius: 0.0 }, 0, 94.0, 152.0);
        let i = shape_info(&s);
        assert_eq!((i.nodes, i.lines, i.curves), (4, 4, 0));
        assert!((i.perimeter - 268.0).abs() < 1e-9);
        assert!((i.area - 4320.0).abs() < 1e-9);
        assert!(i.closed);
        // right edge: from (134,179) down to (134,125), 54 mm, -90° (as in the screenshot)
        let p = &prims(&s)[1];
        assert!((len(&p.pts) - 54.0).abs() < 1e-9);
        let (a, b) = (p.pts[0], p.pts[1]);
        assert_eq!((a, b), ([134.0, 179.0], [134.0, 125.0]));
        assert!(((b[1] - a[1]).atan2(b[0] - a[0]).to_degrees() + 90.0).abs() < 1e-9);
    }

    #[test]
    fn shift_snaps_to_axes_and_diagonals() {
        let a = [0.0, 0.0];
        assert_eq!(axis_snap(a, [10.0, 1.0]), [10.0, 0.0]);
        assert_eq!(axis_snap(a, [1.0, -10.0]), [0.0, -10.0]);
        assert_eq!(axis_snap(a, [10.0, 9.0]), [9.0, 9.0]);
        assert_eq!(axis_snap(a, [-8.0, 9.0]), [-8.0, 8.0]);
    }

    #[test]
    fn circle_has_four_arcs_with_radius() {
        let s = Shape::new(1, Geometry::Ellipse { rx: 10.0, ry: 10.0 }, 0, 0.0, 0.0);
        let i = shape_info(&s);
        assert_eq!((i.nodes, i.lines, i.curves), (4, 0, 4));
        assert!((i.perimeter - std::f64::consts::TAU * 10.0).abs() < 0.01);
        assert_eq!(prims(&s)[0].arc.map(|a| a.0), Some(10.0));
    }
}
