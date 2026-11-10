//! Small line icons drawn with the painter, in the same thin white style as the toolbar
//! (no colour emoji from the font).

use crate::theme::TEXT;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

/// Same line width as the toolbar icons (TB_ICON_STROKE in panels.rs).
pub const ICON_STROKE: f32 = 1.4;

#[derive(Clone, Copy, PartialEq)]
pub enum Ic {
    Save,
    Home,
    Monitor,
    Frame,
    FrameOutline,
    Origin,
    Lock(bool),
    Trash,
    Pencil,
    Play,
    Pause,
    Stop,
    Upload,
    Refresh,
    /// Arrow pointing at (dx, dy), screen axes (y down), each -1, 0 or 1.
    Arrow(i8, i8),
}

/// Draw `ic` in a square of side `size` centred on `c`.
pub fn paint(p: &Painter, c: Pos2, size: f32, ic: Ic, color: Color32) {
    let s = size / 16.0;
    let st = Stroke::new(ICON_STROKE, color);
    let q = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    let line = |pts: &[[f32; 2]]| {
        p.add(egui::Shape::line(pts.iter().map(|a| q(a[0], a[1])).collect(), st));
    };
    let closed = |pts: &[[f32; 2]]| {
        p.add(egui::Shape::closed_line(pts.iter().map(|a| q(a[0], a[1])).collect(), st));
    };
    let fill = |pts: &[[f32; 2]]| {
        p.add(egui::Shape::convex_polygon(pts.iter().map(|a| q(a[0], a[1])).collect(), color, Stroke::NONE));
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        p.rect_stroke(Rect::from_min_max(q(x0, y0), q(x1, y1)), r * s, st, StrokeKind::Middle);
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=20).map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / 20.0;
            q(cx + r * a.cos(), cy + r * a.sin())
        }).collect()
    };
    use std::f32::consts::PI;
    match ic {
        Ic::Save => {
            closed(&[[-6.5, -6.5], [4.0, -6.5], [6.5, -4.0], [6.5, 6.5], [-6.5, 6.5]]);
            rect(-3.5, -6.5, 2.5, -2.5, 0.5);
            rect(-4.0, 1.5, 4.0, 6.5, 0.5);
        }
        Ic::Home => {
            line(&[[-7.5, 0.0], [0.0, -7.0], [7.5, 0.0]]);
            line(&[[-5.5, -1.5], [-5.5, 7.0], [5.5, 7.0], [5.5, -1.5]]);
            line(&[[-1.8, 7.0], [-1.8, 2.5], [1.8, 2.5], [1.8, 7.0]]);
        }
        Ic::Monitor => {
            rect(-7.5, -6.0, 7.5, 3.5, 1.0);
            line(&[[0.0, 3.5], [0.0, 6.5]]);
            line(&[[-4.0, 6.5], [4.0, 6.5]]);
        }
        Ic::Frame => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                line(&[[sx * 7.0, sy * 3.0], [sx * 7.0, sy * 7.0], [sx * 3.0, sy * 7.0]]);
            }
            p.circle_filled(c, 1.3 * s, color);
        }
        Ic::FrameOutline => {
            closed(&[[-7.0, 1.0], [-3.0, -7.0], [5.5, -4.5], [7.0, 3.0], [1.0, 7.0], [-5.0, 6.0]]);
            for (x, y) in [(-3.0, 1.0), (2.5, -1.5), (1.0, 3.5)] {
                p.circle_filled(q(x, y), 1.2 * s, color);
            }
        }
        Ic::Origin => {
            p.circle_stroke(c, 5.5 * s, st);
            p.circle_filled(c, 1.8 * s, color);
            line(&[[0.0, -8.0], [0.0, -5.5]]);
            line(&[[0.0, 5.5], [0.0, 8.0]]);
            line(&[[-8.0, 0.0], [-5.5, 0.0]]);
            line(&[[5.5, 0.0], [8.0, 0.0]]);
        }
        Ic::Lock(locked) => {
            rect(-5.5, -1.0, 5.5, 7.0, 1.2);
            p.circle_filled(q(0.0, 3.0), 1.1 * s, color);
            let mut sh = vec![q(-3.5, -1.0), q(-3.5, -3.5)];
            sh.extend(arc(0.0, -3.5, 3.5, PI, 2.0 * PI));
            sh.push(q(3.5, if locked { -1.0 } else { -2.5 }));
            if !locked {
                // open: the shackle swings up on the right
                sh = sh.into_iter().map(|v| v + vec2(0.0, -2.0 * s)).collect();
                sh[0] = q(-3.5, -1.0);
            }
            p.add(egui::Shape::line(sh, st));
        }
        Ic::Trash => {
            line(&[[-7.0, -4.5], [7.0, -4.5]]);
            line(&[[-2.5, -4.5], [-2.5, -7.0], [2.5, -7.0], [2.5, -4.5]]);
            line(&[[-5.5, -4.5], [-4.5, 7.0], [4.5, 7.0], [5.5, -4.5]]);
            line(&[[-1.8, -1.5], [-1.5, 4.5]]);
            line(&[[1.8, -1.5], [1.5, 4.5]]);
        }
        Ic::Pencil => {
            closed(&[[3.5, -7.0], [7.0, -3.5], [-3.5, 7.0], [-7.0, 7.0], [-7.0, 3.5]]);
            line(&[[1.5, -5.0], [5.0, -1.5]]);
            line(&[[-7.0, 3.5], [-3.5, 7.0]]);
        }
        Ic::Play => fill(&[[-4.5, -6.5], [6.5, 0.0], [-4.5, 6.5]]),
        Ic::Pause => {
            fill(&[[-5.0, -6.0], [-1.8, -6.0], [-1.8, 6.0], [-5.0, 6.0]]);
            fill(&[[1.8, -6.0], [5.0, -6.0], [5.0, 6.0], [1.8, 6.0]]);
        }
        Ic::Stop => fill(&[[-5.5, -5.5], [5.5, -5.5], [5.5, 5.5], [-5.5, 5.5]]),
        Ic::Upload => {
            line(&[[0.0, 3.0], [0.0, -7.0]]);
            line(&[[-4.5, -2.5], [0.0, -7.0], [4.5, -2.5]]);
            line(&[[-7.0, 3.5], [-7.0, 7.0], [7.0, 7.0], [7.0, 3.5]]);
        }
        Ic::Refresh => {
            p.add(egui::Shape::line(arc(0.0, 0.0, 6.0, -PI * 0.35, PI * 1.45), st));
            let tip = q(6.0 * (-PI * 0.35).cos(), 6.0 * (-PI * 0.35).sin());
            p.add(egui::Shape::line(vec![tip + vec2(-4.5 * s, -0.5 * s), tip, tip + vec2(0.5 * s, 4.0 * s)], st));
        }
        Ic::Arrow(dx, dy) => {
            let mut d = vec2(dx as f32, dy as f32);
            if d.length() > 0.0 {
                d = d.normalized();
            }
            let n = vec2(-d.y, d.x);
            let tip = c + d * 7.0 * s;
            p.line_segment([c - d * 7.0 * s, tip], st);
            p.add(egui::Shape::line(vec![tip - d * 5.0 * s + n * 5.0 * s, tip, tip - d * 5.0 * s - n * 5.0 * s], st));
        }
    }
}

/// A framed button that shows only an icon.
pub fn button(ui: &mut egui::Ui, ic: Ic, size: egui::Vec2, icon: f32, tip: &str) -> bool {
    text_button(ui, ic, "", size, None, icon, tip)
}

/// A framed button with an icon followed by a label, both centred; long labels are clipped.
pub fn text_button(ui: &mut egui::Ui, ic: Ic, text: &str, size: egui::Vec2, fill: Option<Color32>, icon: f32, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip) };
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact(&resp);
        let bg = match fill {
            Some(f) if resp.hovered() => f.gamma_multiply(1.15),
            Some(f) => f,
            None => v.weak_bg_fill,
        };
        let p = ui.painter();
        p.rect(rect, v.corner_radius, bg, v.bg_stroke, StrokeKind::Inside);
        let color = if resp.is_pointer_button_down_on() { Color32::from_gray(200) } else { TEXT };
        let font = egui::FontId::proportional(14.0);
        let galley = (!text.is_empty()).then(|| ui.painter().layout_no_wrap(text.to_string(), font, color));
        let gap = if galley.is_some() { 6.0 } else { 0.0 };
        let tw = galley.as_ref().map_or(0.0, |g| g.size().x);
        let total = (icon + gap + tw).min(rect.width() - 8.0);
        let x0 = rect.center().x - total / 2.0;
        paint(p, pos2(x0 + icon / 2.0, rect.center().y), icon, ic, color);
        if let Some(g) = galley {
            let pos = pos2(x0 + icon + gap, rect.center().y - g.size().y / 2.0);
            p.with_clip_rect(rect.shrink(4.0)).galley(pos, g, color);
        }
    }
    resp.clicked()
}
