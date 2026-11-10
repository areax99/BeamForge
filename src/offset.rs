//! Offset shapes (Modifica → Offset). Rules from the original's OffsetDialog (0x1008a341c,
//! Preview 0x1008a4b60, CommitSettings 0x1008a505c), MainWindow::FinalizeOffset 0x10067e348 and
//! Offsetter (ctor 0x10026bd8c, ClipperOffset with miter limit 2.0 and arc tolerance 0.25).
//! See MECHANICS.md, "Offset".

use crate::model::{P2, Polyline};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::stroke::offset::StrokeOffset;
use i_overlay::mesh::float::style::{LineCap, LineJoin, OutlineStyle, StrokeStyle};

/// Distance box range in mm (constant 0x101922da0: 0 .. 1000).
pub const OFFSET_MIN_MM: f64 = 0.0;
pub const OFFSET_MAX_MM: f64 = 1000.0;
/// Clipper units per mm (Offsetter+0x198 = 2048.0, set in the ctor 0x10026bd8c; Compute multiplies
/// the distance by it and divides the result points by it).
pub const CLIPPER_SCALE: f64 = 2048.0;
/// Miter limit Offsetter::Compute 0x10026c448 sets before each run (ClipperOffset.MiterLimit = 10.0;
/// the ctor's 2.0 is overwritten).
pub const MITER_LIMIT: f64 = 10.0;
/// Round-corner arc tolerance in Clipper units: 2.0, or 0.8 when the distance is under 0.2 mm
/// (Offsetter::Compute 0x10026c448, constant 0x3e4ccccd).
pub const ARC_TOLERANCE_UNITS: f64 = 2.0;
pub const ARC_TOLERANCE_SMALL_UNITS: f64 = 0.8;
pub const SMALL_DISTANCE_MM: f64 = 0.2;
/// "Ottimizza / Semplifica risultati": the original arc-fits the results with a preference value
/// × 0.25 (PathShape::ArcFit in Compute); BeamForge drops points within this distance of the
/// line instead (estimated tolerance).
pub const SIMPLIFY_TOLERANCE_MM: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Outward = 0,
    Inward = 1,
    Both = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Round = 0,
    Square = 1,
    Miter = 2,
}

#[derive(Clone, Debug)]
pub struct OffsetSettings {
    pub distance: f64,
    pub direction: Direction,
    pub corner: Corner,
    pub outer_only: bool,
    pub select_results: bool,
    pub delete_originals: bool,
    pub optimize: bool,
}

impl Default for OffsetSettings {
    fn default() -> Self {
        // 5 mm, Esterno, Arrotondato, options off: as the person's picture of the original shows
        Self { distance: 5.0, direction: Direction::Outward, corner: Corner::Round, outer_only: false, select_results: false, delete_originals: false, optimize: false }
    }
}

fn join(corner: Corner, dist: f64) -> LineJoin<f64> {
    match corner {
        Corner::Round => {
            // Clipper's step angle: 2·acos(1 − tolerance / |delta|), both in Clipper units
            let tol = if dist.abs() < SMALL_DISTANCE_MM { ARC_TOLERANCE_SMALL_UNITS } else { ARC_TOLERANCE_UNITS };
            let delta = (dist.abs() * CLIPPER_SCALE).max(1e-6);
            let c = (1.0 - tol / delta).clamp(-1.0, 1.0);
            LineJoin::Round(2.0 * c.acos())
        }
        // Clipper jtSquare (table 0x1018f83dc maps Smussatura → 0); i_overlay has no square join,
        // so the corner is cut flat (bevel), a little closer to the corner than Clipper's
        Corner::Square => LineJoin::Bevel,
        // a miter longer than MITER_LIMIT × distance is cut: the sharpest corner kept has
        // sin(angle / 2) = 1 / MITER_LIMIT
        Corner::Miter => LineJoin::Miter(2.0 * (1.0 / MITER_LIMIT).asin()),
    }
}

/// Offset one set of outlines by `dist` mm (positive grows, negative shrinks). Closed outlines are
/// read even-odd, so a letter's hole stays a hole; open lines get an outline all around them.
pub fn offset_paths(paths: &[Polyline], dist: f64, corner: Corner, outer_only: bool) -> Vec<Polyline> {
    if dist.abs() < 1e-9 {
        return Vec::new();
    }
    let closed: Vec<Vec<[f64; 2]>> = paths.iter().filter(|p| p.closed && p.pts.len() >= 3).map(|p| p.pts.clone()).collect();
    let mut out: Vec<Polyline> = Vec::new();
    if !closed.is_empty() {
        let mut shapes = closed.simplify_shape(FillRule::EvenOdd);
        if outer_only {
            // only the outer outline of each piece is offset; inner ones are left out
            for s in shapes.iter_mut() {
                s.truncate(1);
            }
        }
        let style = OutlineStyle::new(dist).line_join(join(corner, dist));
        for shape in shapes.outline(&style) {
            for c in shape {
                out.push(Polyline { pts: c, closed: true });
            }
        }
    }
    // open lines: outlined on both sides; their ends follow the corner style (Compute sets the
    // Clipper end type to 4 − corner: round, square or butt). Clipper skips open lines when the
    // distance is negative, so shrinking one gives nothing.
    if dist > 0.0 {
        for p in paths.iter().filter(|p| !p.closed && p.pts.len() >= 2) {
            let cap = match corner {
                Corner::Round => LineCap::Round(join(corner, dist).angle()),
                Corner::Square => LineCap::Square,
                Corner::Miter => LineCap::Butt,
            };
            let style = StrokeStyle::new(2.0 * dist).line_join(join(corner, dist)).start_cap(cap.clone()).end_cap(cap);
            for shape in p.pts.stroke(style, false) {
                for c in shape {
                    out.push(Polyline { pts: c, closed: true });
                }
            }
        }
    }
    out
}

trait JoinAngle {
    fn angle(&self) -> f64;
}

impl JoinAngle for LineJoin<f64> {
    fn angle(&self) -> f64 {
        match self {
            LineJoin::Round(a) | LineJoin::Miter(a) => *a,
            _ => 0.25,
        }
    }
}

/// Every result for one run of the dialog: outward, inward or both (FinalizeOffset 0x10067e348
/// runs a second Offsetter with the distance negated for Entrambe).
/// Each entry becomes one new shape.
pub fn offset_all(groups: &[Vec<Polyline>], s: &OffsetSettings) -> Vec<Vec<Polyline>> {
    let d = s.distance.clamp(OFFSET_MIN_MM, OFFSET_MAX_MM);
    let signs: &[f64] = match s.direction {
        Direction::Outward => &[1.0],
        Direction::Inward => &[-1.0],
        Direction::Both => &[1.0, -1.0],
    };
    let mut out = Vec::new();
    for sign in signs {
        for g in groups {
            let mut r = offset_paths(g, sign * d, s.corner, s.outer_only);
            if s.optimize {
                r = r.into_iter().map(|p| simplify(p, SIMPLIFY_TOLERANCE_MM)).filter(|p| p.pts.len() >= 3).collect();
            }
            if !r.is_empty() {
                out.push(r);
            }
        }
    }
    out
}

/// Drop points that lie within `tol` mm of the line through their neighbours (Douglas–Peucker).
pub fn simplify(p: Polyline, tol: f64) -> Polyline {
    fn dp(pts: &[P2], tol: f64, keep: &mut Vec<bool>, a: usize, b: usize) {
        if b <= a + 1 {
            return;
        }
        let (mut best, mut idx) = (0.0, a);
        for i in a + 1..b {
            let d = crate::model::dist_seg(pts[i], pts[a], pts[b]);
            if d > best {
                best = d;
                idx = i;
            }
        }
        if best > tol {
            keep[idx] = true;
            dp(pts, tol, keep, a, idx);
            dp(pts, tol, keep, idx, b);
        }
    }
    let n = p.pts.len();
    if n < 4 {
        return p;
    }
    let mut pts = p.pts.clone();
    if p.closed {
        pts.push(pts[0]);
    }
    let m = pts.len();
    let mut keep = vec![false; m];
    keep[0] = true;
    keep[m - 1] = true;
    dp(&pts, tol, &mut keep, 0, m - 1);
    let mut res: Vec<P2> = pts.iter().zip(&keep).filter(|(_, k)| **k).map(|(q, _)| *q).collect();
    if p.closed {
        res.pop();
    }
    Polyline { pts: res, closed: p.closed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(s: f64) -> Polyline {
        Polyline { pts: vec![[0.0, 0.0], [s, 0.0], [s, s], [0.0, s]], closed: true }
    }

    fn bounds(ps: &[Polyline]) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in ps {
            for q in &p.pts {
                b = (b.0.min(q[0]), b.1.min(q[1]), b.2.max(q[0]), b.3.max(q[1]));
            }
        }
        b
    }

    #[test]
    fn outward_and_inward() {
        let o = offset_paths(&[square(10.0)], 2.0, Corner::Miter, false);
        let b = bounds(&o);
        assert!((b.0 + 2.0).abs() < 0.05 && (b.2 - 12.0).abs() < 0.05, "{b:?}");
        let i = offset_paths(&[square(10.0)], -2.0, Corner::Miter, false);
        let b = bounds(&i);
        assert!((b.0 - 2.0).abs() < 0.05 && (b.2 - 8.0).abs() < 0.05, "{b:?}");
    }

    #[test]
    fn both_gives_two_outlines() {
        let s = OffsetSettings { direction: Direction::Both, distance: 1.0, ..Default::default() };
        assert_eq!(offset_all(&[vec![square(10.0)]], &s).len(), 2);
        let s = OffsetSettings { direction: Direction::Inward, distance: 6.0, ..Default::default() };
        // shrinking a 10 mm square by 6 mm leaves nothing
        assert!(offset_all(&[vec![square(10.0)]], &s).is_empty());
    }
}
