//! Combining two objects: join, cut one out of the other, keep the overlap
//! (MainWindow::BooleanOperation 0x100c56cf4 → BooleanOperator::Compute 0x100179f88), and the
//! assistant window that tries each result (BooleanHelper 0x10081c240).

use crate::app::BeamForge;
use crate::i18n::tr;
use crate::model::*;
use crate::theme::{self, ACCENT};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use std::collections::HashMap;

/// Result points closer than this (mm, on both axes) are merged into their midpoint
/// (`0x3c4ccccd` with ×0.5, BooleanOperator::Compute 0x10017a2xx).
pub const MERGE_DIST: f64 = 0.0125;
/// Clipper's integer units per mm: results are scaled back by 2⁻¹² (`0x3f30000000000000`,
/// BooleanOperator::Compute 0x100179f88).
pub const COORD_SCALE: f64 = 4096.0;
/// The assistant previews a result on hover only while both objects together have at most this
/// many outline pieces (`cmp w8,#0x5db`, BooleanHelper ctor 0x10081c240).
pub const HOVER_PREVIEW_MAX_PRIMS: usize = 1499;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CombineOp {
    /// BoolOpType 2 → Clipper ctUnion
    Union,
    /// BoolOpType 0 → ctDifference: first selected minus second
    Subtract,
    /// BoolOpType 1 → ctIntersection
    Intersect,
    /// The assistant's second difference (doDifference2 0x10081db8c): second minus first
    SubtractReverse,
}

impl CombineOp {
    pub fn label(self) -> &'static str {
        match self {
            CombineOp::Union => tr("Unisci forme"),
            CombineOp::Subtract => tr("Sottrai forme"),
            CombineOp::Intersect => tr("Interseca forme"),
            CombineOp::SubtractReverse => tr("Sottrai al contrario"),
        }
    }
}

/// The closed outlines of `a` and `b` combined with the even-odd rule (Clipper pftEvenOdd = 0).
/// Each result is one piece: its outer outline followed by its holes. Open lines are left out.
pub fn combine(a: &[Polyline], b: &[Polyline], op: CombineOp) -> Vec<Vec<Polyline>> {
    // points on the original's integer grid (Clipper works in 1/COORD_SCALE mm)
    let snap = |p: &P2| [(p[0] * COORD_SCALE).round() / COORD_SCALE, (p[1] * COORD_SCALE).round() / COORD_SCALE];
    let contours = |ps: &[Polyline]| -> Vec<Vec<[f64; 2]>> { ps.iter().filter(|p| p.closed && p.pts.len() >= 3).map(|p| p.pts.iter().map(snap).collect()).collect() };
    let (mut subj, mut clip) = (contours(a), contours(b));
    if op == CombineOp::SubtractReverse {
        std::mem::swap(&mut subj, &mut clip);
    }
    let rule = match op {
        CombineOp::Union => OverlayRule::Union,
        CombineOp::Intersect => OverlayRule::Intersect,
        CombineOp::Subtract | CombineOp::SubtractReverse => OverlayRule::Difference,
    };
    let shapes = subj.overlay(&clip, rule, FillRule::EvenOdd);
    shapes
        .into_iter()
        .map(|shape| shape.into_iter().map(|c| Polyline { pts: merge_close(c), closed: true }).filter(|p| p.pts.len() >= 3).collect::<Vec<_>>())
        .filter(|s: &Vec<Polyline>| !s.is_empty())
        .collect()
}

/// Merge neighbouring points closer than MERGE_DIST on both axes into their midpoint.
fn merge_close(pts: Vec<P2>) -> Vec<P2> {
    let mut out: Vec<P2> = Vec::with_capacity(pts.len());
    for p in pts {
        if let Some(last) = out.last_mut() {
            if (p[0] - last[0]).abs() < MERGE_DIST && (p[1] - last[1]).abs() < MERGE_DIST {
                *last = [(p[0] + last[0]) * 0.5, (p[1] + last[1]) * 0.5];
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// The assistant window while it is open: the document before it, the two objects in the
/// order they were picked, and which result is shown.
pub struct CombineAssist {
    pub before: Document,
    pub selection_before: Vec<u64>,
    pub items: [Vec<u64>; 2],
    pub applied: Option<CombineOp>,
    pub hover_preview: bool,
    /// The result the drawing shows right now (None = the originals).
    pub shown: Option<CombineOp>,
}

impl BeamForge {
    /// The two picked objects, or a message when there aren't exactly two.
    fn two_objects(&mut self) -> Option<[Vec<u64>; 2]> {
        let items = self.selection_items();
        if items.len() != 2 {
            self.status = tr("Seleziona esattamente 2 oggetti (il primo scelto è quello da cui si sottrae).").into();
            return None;
        }
        Some([items[0].clone(), items[1].clone()])
    }

    fn paths_of(&self, ids: &[u64]) -> Vec<Polyline> {
        self.doc.shapes.iter().filter(|s| ids.contains(&s.id)).flat_map(|s| s.world_paths()).collect()
    }

    /// Replace the two objects with the result, on the layer most used by them; the result is
    /// selected (UndoImportShape + UndoDeleteShapes + UndoChangeSelection in one undo group).
    /// Returns false when the result is empty (nothing changes).
    fn apply_combine(&mut self, items: &[Vec<u64>; 2], op: CombineOp) -> bool {
        let result = combine(&self.paths_of(&items[0]), &self.paths_of(&items[1]), op);
        if result.is_empty() {
            return false;
        }
        // BooleanOperator::GetMostUsedCutIndex 0x10017aa14
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for s in self.doc.shapes.iter().filter(|s| items[0].contains(&s.id) || items[1].contains(&s.id)) {
            *counts.entry(s.layer).or_default() += 1;
        }
        let layer = counts.into_iter().max_by_key(|(l, n)| (*n, std::cmp::Reverse(*l))).map(|(l, _)| l).unwrap_or(self.current_layer);
        let gone: Vec<u64> = items.iter().flatten().copied().collect();
        self.doc.shapes.retain(|s| !gone.contains(&s.id));
        let gid = (result.len() > 1).then(|| self.doc.alloc_id());
        let mut ids = Vec::new();
        for subpaths in result {
            let mut s = Shape::new(0, Geometry::Path { subpaths }, layer, 0.0, 0.0);
            if let Some(g) = gid {
                s.groups.push(g);
            }
            ids.push(self.doc.add_shape(s));
        }
        self.selection = ids;
        true
    }

    /// Unisci / Sottrai / Interseca from the tool bar or the menu.
    pub fn combine_selection(&mut self, op: CombineOp) {
        let Some(items) = self.two_objects() else { return };
        let before = self.doc.clone();
        if self.apply_combine(&items, op) {
            let after = std::mem::replace(&mut self.doc, before);
            self.checkpoint_now();
            self.doc = after;
            self.status = op.label().into();
        } else {
            self.status = tr("Il risultato è vuoto: le forme non cambiano.").into();
        }
    }

    pub fn open_combine_assistant(&mut self) {
        let Some(items) = self.two_objects() else { return };
        let prims: usize = self.paths_of(&items[0]).iter().chain(self.paths_of(&items[1]).iter()).map(|p| p.pts.len()).sum();
        self.combine_assist = Some(CombineAssist {
            before: self.doc.clone(),
            selection_before: self.selection.clone(),
            items,
            applied: None,
            hover_preview: prims <= HOVER_PREVIEW_MAX_PRIMS,
            shown: None,
        });
    }

    /// Show `op` (or the originals for None) in the document, starting from the saved state.
    fn assistant_show(&mut self, op: Option<CombineOp>) {
        let Some(a) = self.combine_assist.as_ref() else { return };
        let (doc, sel, items) = (a.before.clone(), a.selection_before.clone(), a.items.clone());
        self.doc = doc;
        self.selection = sel;
        if let Some(op) = op {
            if !self.apply_combine(&items, op) {
                self.status = tr("Il risultato è vuoto: le forme non cambiano.").into();
            }
        }
    }
}

/// Two overlapping squares, drawn the way each operation keeps them (own artwork).
pub fn paint_icon(p: &egui::Painter, c: Pos2, op: Option<CombineOp>, col: Color32, k: f32) {
    let a = Rect::from_center_size(c + vec2(-2.5, -2.5) * k, vec2(11.0, 11.0) * k);
    let b = Rect::from_center_size(c + vec2(2.5, 2.5) * k, vec2(11.0, 11.0) * k);
    let st = Stroke::new(1.5 * k.max(0.8), col);
    let dash = |r: Rect| egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.0, col), 1.5 * k, 1.5 * k);
    let ov = a.intersect(b);
    match op {
        Some(CombineOp::Union) => {
            p.add(egui::Shape::closed_line(vec![a.left_top(), a.right_top(), pos2(a.right(), b.top()), b.right_top(), b.right_bottom(), b.left_bottom(), pos2(b.left(), a.bottom()), a.left_bottom()], st));
            p.extend(dash(ov));
        }
        Some(CombineOp::Subtract) | Some(CombineOp::SubtractReverse) => {
            let (keep, cut) = if op == Some(CombineOp::Subtract) { (a, b) } else { (b, a) };
            let o = keep.intersect(cut);
            let corner = |x: f32, y: f32| pos2(x, y);
            if op == Some(CombineOp::Subtract) {
                p.add(egui::Shape::closed_line(vec![keep.left_top(), keep.right_top(), corner(keep.right(), o.top()), o.left_top(), corner(o.left(), keep.bottom()), keep.left_bottom()], st));
            } else {
                p.add(egui::Shape::closed_line(vec![o.right_bottom(), corner(o.right(), keep.top()), keep.right_top(), keep.right_bottom(), keep.left_bottom(), corner(keep.left(), o.bottom())], st));
            }
            p.extend(dash(cut));
        }
        Some(CombineOp::Intersect) => {
            p.extend(dash(a));
            p.extend(dash(b));
            p.rect_stroke(ov, 0.0, st, StrokeKind::Middle);
        }
        None => {
            // the assistant: both outlines whole
            p.rect_stroke(a, 0.0, st, StrokeKind::Middle);
            p.rect_stroke(b, 0.0, st, StrokeKind::Middle);
        }
    }
}

/// The assistant window (BooleanHelper): pick a result, see it on the drawing, then OK keeps it
/// and Annulla puts the objects back (on_BooleanHelper_rejected 0x10081d77c undoes).
pub fn assistant_window(app: &mut BeamForge, ctx: &egui::Context) {
    let Some(a) = app.combine_assist.as_ref() else { return };
    let (applied, hover_ok) = (a.applied, a.hover_preview);
    let mut clicked: Option<Option<CombineOp>> = None;
    let mut hovered: Option<Option<CombineOp>> = None;
    let mut ok = false;
    let mut cancel = false;
    let mut open = true;
    theme::centered(ctx, "combine_assist", egui::Window::new(tr("Assistente combinazione")).open(&mut open).resizable(false).collapsible(false).fixed_size(vec2(640.0, 190.0))).show(ctx, |ui| {
        ui.label(tr("Scegli come combinare i 2 oggetti selezionati: il risultato compare subito sul disegno."));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ops: [(Option<CombineOp>, &str); 5] = [
                (Some(CombineOp::Union), tr("Unisci")),
                (Some(CombineOp::Intersect), tr("Interseca")),
                (Some(CombineOp::Subtract), tr("Primo − secondo")),
                (Some(CombineOp::SubtractReverse), tr("Secondo − primo")),
                (None, tr("Originali")),
            ];
            for (op, name) in ops {
                let (rect, resp) = ui.allocate_exact_size(vec2(118.0, 84.0), Sense::click());
                let p = ui.painter();
                let on = applied == op;
                p.rect(rect, 6.0, if on { Color32::from_rgb(120, 52, 18) } else if resp.hovered() { crate::theme::p().btn_hover } else { crate::theme::p().field }, Stroke::new(1.0, if on { ACCENT } else { crate::theme::p().line }), StrokeKind::Inside);
                // white on the dark orange of the picked operation, theme text otherwise
                let ink = if on { Color32::WHITE } else { crate::theme::p().text };
                paint_icon(p, rect.center() - vec2(0.0, 10.0), op, ink, 1.8);
                p.text(pos2(rect.center().x, rect.bottom() - 14.0), egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(13.0), ink);
                if resp.clicked() {
                    clicked = Some(op);
                }
                if resp.hovered() {
                    hovered = Some(op);
                }
            }
        });
        if !hover_ok {
            ui.label(egui::RichText::new(tr("Forme molto complesse: il risultato si vede al clic, non al passaggio del mouse.")).color(crate::theme::p().text_dim));
        }
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(tr("OK")).fill(Color32::from_rgb(170, 70, 20)).min_size(vec2(90.0, 26.0))).clicked() {
                ok = true;
            }
            if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                cancel = true;
            }
        });
    });
    // what the drawing should show: the clicked result, or while hovering (doUnion_hover …
    // 0x10081d960) the hovered one, back to the chosen one after
    let want = match clicked {
        Some(op) => {
            if let Some(a) = app.combine_assist.as_mut() {
                a.applied = op;
            }
            op
        }
        None if hover_ok => hovered.unwrap_or(applied),
        None => applied,
    };
    if app.combine_assist.as_ref().is_some_and(|a| a.shown != want) {
        app.assistant_show(want);
        if let Some(a) = app.combine_assist.as_mut() {
            a.shown = want;
        }
    }
    if ok {
        let a = app.combine_assist.take().unwrap();
        app.assistant_finish(a, true);
    } else if cancel || !open {
        let a = app.combine_assist.take().unwrap();
        app.assistant_finish(a, false);
    }
}

impl BeamForge {
    /// OK keeps the chosen result as one undo step; Annulla / ✕ puts the objects back.
    fn assistant_finish(&mut self, a: CombineAssist, keep: bool) {
        self.doc = a.before;
        self.selection = a.selection_before;
        if let (true, Some(op)) = (keep, a.applied) {
            self.checkpoint_now();
            if self.apply_combine(&a.items, op) {
                self.status = op.label().into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: f64, y: f64, s: f64) -> Vec<Polyline> {
        vec![Polyline { pts: vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]], closed: true }]
    }

    fn area(ps: &[Polyline]) -> f64 {
        ps.iter()
            .map(|p| {
                let n = p.pts.len();
                (0..n).map(|i| p.pts[i][0] * p.pts[(i + 1) % n][1] - p.pts[(i + 1) % n][0] * p.pts[i][1]).sum::<f64>() / 2.0
            })
            .sum::<f64>()
            .abs()
    }

    #[test]
    fn two_squares() {
        let (a, b) = (sq(0.0, 0.0, 10.0), sq(5.0, 5.0, 10.0));
        let u = combine(&a, &b, CombineOp::Union);
        assert_eq!(u.len(), 1);
        assert!((area(&u[0]) - 175.0).abs() < 1e-6);
        let i = combine(&a, &b, CombineOp::Intersect);
        assert!((area(&i[0]) - 25.0).abs() < 1e-6);
        let d = combine(&a, &b, CombineOp::Subtract);
        assert!((area(&d[0]) - 75.0).abs() < 1e-6);
        assert!(d[0][0].pts.iter().any(|p| p[0] < 1.0 && p[1] < 1.0));
        let r = combine(&a, &b, CombineOp::SubtractReverse);
        assert!(r[0][0].pts.iter().any(|p| p[0] > 14.0 && p[1] > 14.0));
        // apart: intersection is empty, union keeps both pieces
        let far = sq(50.0, 50.0, 5.0);
        assert!(combine(&a, &far, CombineOp::Intersect).is_empty());
        assert_eq!(combine(&a, &far, CombineOp::Union).len(), 2);
    }
}
