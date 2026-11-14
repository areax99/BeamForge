//! Plans the laser path for a document and writes GRBL G-code.
//! Rules from MECHANICS.md (Protocol_GCode::OutputHeader 0x1005e3958,
//! EmitSpeedValue 0x1005de0e8, Protocol_GRBL::GetLaserOnOffCommand 0x10062afd4).

use crate::model::{BBox, CutMode, CutSetting, Document, Geometry, ImageMode, P2, Polyline, Shape, is_tool_layer, layer_name, point_in_rings};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum StartFrom {
    AbsoluteCoords,
    UserOrigin,
    CurrentPosition,
}

#[derive(Clone, Debug)]
pub struct JobOptions {
    pub start_from: StartFrom,
    /// 0..8, 3x3 grid, 0 = top-left
    pub job_origin: usize,
    pub selected_only: bool,
    pub optimize: bool,
    /// "Usa origine selezione": the job origin is taken from the selection's box, not the job's
    pub selection_origin: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    /// G0, laser off
    Rapid,
    /// G1 with S0 (overscan / scan gaps)
    Travel,
    /// G1 burning
    Cut,
    /// Dot mode: go to `to` with the laser off, stop, then fire at `power` for `us` microseconds
    Dot { us: u32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub kind: MoveKind,
    pub to: P2,
    /// percent
    pub power: f64,
    /// mm/min
    pub speed: f64,
    pub layer: usize,
}

/// The motion model the preview's times use (device > Altre impostazioni).
#[derive(Clone, Debug)]
pub struct Sim {
    /// mm/s
    pub rapid: f64,
    /// top speed per axis, mm/s
    pub max_speed: [f64; 2],
    /// mm/s² per axis
    pub cut_accel: [f64; 2],
    pub engrave_accel: [f64; 2],
    /// mm/s
    pub min_corner: f64,
    /// percent
    pub cut_scale: f64,
    pub global_scale: f64,
    /// per layer: engraves (fill / image) rather than cuts
    pub engrave_layers: Vec<bool>,
}

impl Default for Sim {
    fn default() -> Self {
        Sim {
            rapid: 400.0,
            max_speed: [f64::INFINITY; 2],
            cut_accel: [3000.0; 2],
            engrave_accel: [3000.0; 2],
            min_corner: 0.0,
            cut_scale: 100.0,
            global_scale: 100.0,
            engrave_layers: Vec::new(),
        }
    }
}

impl Sim {
    fn engraving(&self, layer: usize) -> bool {
        self.engrave_layers.get(layer).copied().unwrap_or(false)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Job {
    pub moves: Vec<Move>,
    pub start: P2,
    pub bounds: Option<BBox>,
    /// (layer, pass, passes, first move index)
    pub sections: Vec<(usize, u32, u32, usize)>,
}

impl Job {
    pub fn cut_length(&self) -> f64 {
        let mut last = self.start;
        let mut d = 0.0;
        for m in &self.moves {
            if matches!(m.kind, MoveKind::Cut) {
                d += dist(last, m.to);
            }
            last = m.to;
        }
        d
    }

    /// Job time with acceleration, like the original's MotionPlanner (0x100c0215c): each move
    /// speeds up and slows down at `accel` mm/s²; the speed carried through a corner is scaled
    /// by the cosine of the turn (0 for 90° or more). Rapids run at `rapid_speed`.
    #[cfg(test)]
    pub fn estimate_seconds(&self, rapid_speed: f64, accel: f64) -> f64 {
        self.move_seconds(rapid_speed, accel).iter().sum()
    }

    /// "Impostazioni della simulazione" from the device, and which layers engrave (Fill, Offset
    /// Fill, Image) rather than cut.
    pub fn sim_for(doc: &Document) -> Sim {
        let d = &doc.device;
        Sim {
            rapid: d.rapid_speed,
            max_speed: d.max_speed,
            cut_accel: d.cut_accel,
            engrave_accel: d.engrave_accel,
            min_corner: d.min_corner_speed,
            cut_scale: d.cut_speed_scale,
            global_scale: d.global_speed_scale,
            engrave_layers: (0..doc.layers.len()).map(|l| doc.layer_art(l).clamp(doc.layers[l].mode) != CutMode::Line).collect(),
        }
    }

    /// Length of the moves with the laser off (rapids and overscan), mm.
    pub fn rapid_length(&self) -> f64 {
        let mut last = self.start;
        let mut d = 0.0;
        for m in &self.moves {
            if !matches!(m.kind, MoveKind::Cut) {
                d += dist(last, m.to);
            }
            last = m.to;
        }
        d
    }

    /// Seconds each move takes, with the same motion model as `estimate_seconds`.
    #[cfg(test)]
    pub fn move_seconds(&self, rapid_speed: f64, accel: f64) -> Vec<f64> {
        let sim = Sim { rapid: rapid_speed, cut_accel: [accel; 2], engrave_accel: [accel; 2], ..Sim::default() };
        self.move_seconds_sim(&sim)
    }

    /// Job time with the device's "Impostazioni della simulazione".
    pub fn estimate_seconds_sim(&self, sim: &Sim) -> f64 {
        self.move_seconds_sim(sim).iter().sum()
    }

    /// Seconds each move takes with the device's simulation settings: per-axis top speeds and
    /// accelerations (cutting or engraving by the move's layer), the speed scales and the
    /// lowest corner speed. They only change the preview's times, never the G-code.
    pub fn move_seconds_sim(&self, sim: &Sim) -> Vec<f64> {
        let mut times = Vec::with_capacity(self.moves.len());
        let n = self.moves.len();
        let mut pts = Vec::with_capacity(n + 1);
        pts.push(self.start);
        pts.extend(self.moves.iter().map(|m| m.to));
        let dir = |i: usize| {
            let d = dist(pts[i], pts[i + 1]);
            if d > 1e-9 { [(pts[i + 1][0] - pts[i][0]) / d, (pts[i + 1][1] - pts[i][1]) / d] } else { [0.0, 0.0] }
        };
        // along direction (ux, uy), the axes allow v so that |ux|·v ≤ x limit and |uy|·v ≤ y limit
        let along = |lim: [f64; 2], u: [f64; 2]| {
            let mut v = f64::INFINITY;
            if u[0].abs() > 1e-9 {
                v = v.min(lim[0] / u[0].abs());
            }
            if u[1].abs() > 1e-9 {
                v = v.min(lim[1] / u[1].abs());
            }
            if v.is_finite() { v } else { lim[0].min(lim[1]) }
        };
        let global = (sim.global_scale / 100.0).max(0.01);
        let speed_of = |i: usize| {
            let m = &self.moves[i];
            let base = match m.kind {
                MoveKind::Rapid | MoveKind::Dot { .. } => sim.rapid,
                _ => m.speed / 60.0 * if sim.engraving(m.layer) { 1.0 } else { (sim.cut_scale / 100.0).max(0.01) },
            };
            (base * global).min(along(sim.max_speed, dir(i)))
        };
        let accel_of = |i: usize| {
            let m = &self.moves[i];
            let lim = if !matches!(m.kind, MoveKind::Rapid) && sim.engraving(m.layer) { sim.engrave_accel } else { sim.cut_accel };
            along(lim, dir(i)).max(1.0)
        };
        let dwell = |m: &Move| match m.kind {
            MoveKind::Dot { us } => us as f64 / 1e6,
            _ => 0.0,
        };
        let mut v_in = 0.0f64;
        for i in 0..n {
            let d = dist(pts[i], pts[i + 1]);
            let vmax = speed_of(i).max(0.1);
            let a = accel_of(i);
            if d < 1e-9 {
                times.push(dwell(&self.moves[i]));
                continue;
            }
            // speed allowed at the end of this move (corner into the next one); a dot stops
            let stops = matches!(self.moves[i].kind, MoveKind::Dot { .. });
            let v_out_target = if !stops && i + 1 < n && std::mem::discriminant(&self.moves[i].kind) == std::mem::discriminant(&self.moves[i + 1].kind) {
                let (p, q) = (dir(i), dir(i + 1));
                let c = (p[0] * q[0] + p[1] * q[1]).max(0.0);
                // "Velocità d'angolo minima": a real turn is never taken slower than this
                let turn = vmax.min(speed_of(i + 1)) * c;
                if c < 1.0 - 1e-9 { turn.max(sim.min_corner.min(vmax)) } else { turn }
            } else {
                0.0
            };
            let v0 = v_in.min(vmax);
            let v1 = v_out_target.min((v0 * v0 + 2.0 * a * d).sqrt());
            let vpeak = vmax.min(((2.0 * a * d + v0 * v0 + v1 * v1) / 2.0).sqrt()).max(v0.max(v1));
            let d_acc = ((vpeak * vpeak - v0 * v0) / (2.0 * a)).max(0.0);
            let d_dec = ((vpeak * vpeak - v1 * v1) / (2.0 * a)).max(0.0);
            let cruise = (d - d_acc - d_dec).max(0.0);
            times.push((vpeak - v0) / a + (vpeak - v1) / a + cruise / vpeak + dwell(&self.moves[i]));
            v_in = v1;
        }
        times
    }

    /// The part of the job from move `from` on, for "Comincia qui" in the preview
    /// (PreviewDialog::on_actionStartFromHere_triggered 0x100da3d48): it starts where the head
    /// is at that point and keeps the layer/pass of the section it falls in.
    pub fn from_move(&self, from: usize) -> Job {
        let from = from.min(self.moves.len());
        let start = if from == 0 { self.start } else { self.moves[from - 1].to };
        let mut sections: Vec<(usize, u32, u32, usize)> = Vec::new();
        if let Some(cur) = self.sections.iter().filter(|s| s.3 <= from).next_back() {
            sections.push((cur.0, cur.1, cur.2, 0));
        }
        sections.extend(self.sections.iter().filter(|s| s.3 > from).map(|s| (s.0, s.1, s.2, s.3 - from)));
        sections.dedup_by(|b, a| a.3 == b.3);
        Job { moves: self.moves[from..].to_vec(), start, bounds: self.bounds, sections }
    }
}

pub fn dist(a: P2, b: P2) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn ring_area(pts: &[P2]) -> f64 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a.abs() / 2.0
}

fn bbox_of(pl: &Polyline) -> BBox {
    let mut b = BBox::empty();
    for p in &pl.pts {
        b.add(*p);
    }
    b
}

/// Order polylines: nearest next start, but never cut a shape before the shapes inside it.
fn order_paths(mut paths: Vec<Polyline>, mut pos: P2, optimize: bool) -> Vec<Polyline> {
    if !optimize {
        return paths;
    }
    let boxes: Vec<BBox> = paths.iter().map(bbox_of).collect();
    let areas: Vec<f64> = paths.iter().map(|p| if p.closed { ring_area(&p.pts) } else { 0.0 }).collect();
    let n = paths.len();
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    for _ in 0..n {
        let mut best: Option<(usize, usize, bool, f64)> = None;
        for i in 0..n {
            if done[i] {
                continue;
            }
            // blocked if an unfinished smaller path lies inside this closed one
            if paths[i].closed {
                let blocked = (0..n).any(|j| j != i && !done[j] && areas[j] < areas[i] && boxes[i].contains(&boxes[j]));
                if blocked {
                    continue;
                }
            }
            let pl = &paths[i];
            if pl.closed {
                for (k, p) in pl.pts.iter().enumerate() {
                    let d = dist(pos, *p);
                    if best.is_none_or(|b| d < b.3) {
                        best = Some((i, k, false, d));
                    }
                }
            } else {
                let d0 = dist(pos, pl.pts[0]);
                let d1 = dist(pos, *pl.pts.last().unwrap());
                if best.is_none_or(|b| d0 < b.3) {
                    best = Some((i, 0, false, d0));
                }
                if best.is_none_or(|b| d1 < b.3) {
                    best = Some((i, 0, true, d1));
                }
            }
        }
        let Some((i, k, rev, _)) = best else { break };
        done[i] = true;
        let mut pl = std::mem::replace(&mut paths[i], Polyline { pts: vec![], closed: false });
        if pl.closed && k > 0 {
            pl.pts.rotate_left(k);
        }
        if rev {
            pl.pts.reverse();
        }
        pos = if pl.closed { pl.pts[0] } else { *pl.pts.last().unwrap() };
        order.push(pl);
    }
    order
}

fn rotate(p: P2, a: f64) -> P2 {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

/// The lowest power of a graded burn: GRBL has no separate minimum power (the editor greys the
/// field out, UpdatePowerControls 0x100b86830), so ramps and greyscale start from 0 %.
fn low_power(cs: &CutSetting) -> f64 {
    if cs.min_power < cs.max_power { cs.min_power } else { 0.0 }
}

/// Number of power steps a ramp is cut into (estimated: the original samples each scan segment
/// at a step it keeps in the scanner, RasterScanner::GenerateLineRamp 0x100276548).
pub const RAMP_STEPS: f64 = 8.0;
/// Shortest ramp step in mm (estimated).
pub const RAMP_MIN_STEP_MM: f64 = 0.05;

/// The scan lines of a set of rings (even-odd), in the rotated frame: each line's y and its
/// burning segments left to right.
fn scan_segments(rot: &[Vec<P2>], interval: f64) -> Vec<(f64, Vec<(f64, f64)>)> {
    let mut miny = f64::INFINITY;
    let mut maxy = f64::NEG_INFINITY;
    for r in rot {
        for p in r {
            miny = miny.min(p[1]);
            maxy = maxy.max(p[1]);
        }
    }
    let mut out = Vec::new();
    let mut y = miny + interval / 2.0;
    while y < maxy {
        let mut xs: Vec<f64> = Vec::new();
        for r in rot {
            let n = r.len();
            for i in 0..n {
                let p = r[i];
                let q = r[(i + 1) % n];
                if (p[1] > y) != (q[1] > y) {
                    xs.push(p[0] + (y - p[1]) / (q[1] - p[1]) * (q[0] - p[0]));
                }
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let segs: Vec<(f64, f64)> = xs.chunks(2).filter(|c| c.len() == 2 && c[1] - c[0] > 1e-6).map(|c| (c[0], c[1])).collect();
        if !segs.is_empty() {
            out.push((y, segs));
        }
        y += interval;
    }
    out
}

/// Flood fill (CUT_PARAMS floodFill +0x92, RasterScanner::GenerateFloodFill 0x100274f88): the
/// scan segments split into connected regions (a segment joins the region of an overlapping
/// segment on the line before), each region filled on its own so the head doesn't cross gaps.
pub fn flood_regions(lines: &[(f64, Vec<(f64, f64)>)]) -> Vec<Vec<(f64, Vec<(f64, f64)>)>> {
    let mut regions: Vec<Vec<(f64, Vec<(f64, f64)>)>> = Vec::new();
    // for each region, its segment on the previous line and that line's index
    let mut last: Vec<(usize, (f64, f64))> = Vec::new();
    for (li, (y, segs)) in lines.iter().enumerate() {
        let mut taken: Vec<usize> = Vec::new();
        for seg in segs {
            let found = (0..regions.len()).find(|&r| !taken.contains(&r) && last[r].0 + 1 == li && last[r].1 .0 < seg.1 && seg.0 < last[r].1 .1);
            let r = match found {
                Some(r) => r,
                None => {
                    regions.push(Vec::new());
                    last.push((li, *seg));
                    regions.len() - 1
                }
            };
            taken.push(r);
            last[r] = (li, *seg);
            regions[r].push((*y, vec![*seg]));
        }
    }
    regions
}

/// The rings a ramp measures from: all of them, or with "rampOuter" only the outer ones
/// (rings not inside an odd number of others).
fn ramp_edges(rot: &[Vec<P2>], outer_only: bool) -> Vec<Vec<P2>> {
    if !outer_only {
        return rot.to_vec();
    }
    rot.iter()
        .enumerate()
        .filter(|(i, r)| {
            let p = r[0];
            let inside = rot.iter().enumerate().filter(|(j, o)| j != i && point_in_ring(p, o)).count();
            inside % 2 == 0
        })
        .map(|(_, r)| r.clone())
        .collect()
}

fn point_in_ring(p: P2, r: &[P2]) -> bool {
    let n = r.len();
    let mut inside = false;
    for i in 0..n {
        let a = r[i];
        let b = r[(i + 1) % n];
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            inside = !inside;
        }
    }
    inside
}

fn edge_distance(p: P2, rings: &[Vec<P2>]) -> f64 {
    let mut d = f64::INFINITY;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            d = d.min(crate::model::dist_seg(p, r[i], r[(i + 1) % n]));
        }
    }
    d
}

/// Ramp power (rampLength +0x7c, GenerateLineRamp 0x100276548): the distance to the nearest
/// edge, capped at the ramp length, scales the power from min (at the edge) to max.
pub fn ramp_power(min_p: f64, max_p: f64, edge_dist: f64, ramp: f64) -> f64 {
    if ramp <= 0.0 {
        return max_p;
    }
    min_p + (max_p - min_p) * (edge_dist / ramp).clamp(0.0, 1.0)
}

/// Write scan lines as moves (bidirectional, overscan, ramp).
#[allow(clippy::too_many_arguments)]
fn emit_scan(lines: &[(f64, Vec<(f64, f64)>)], ramp_rings: &[Vec<P2>], cs: &CutSetting, a: f64, moves: &mut Vec<Move>, layer: usize, pos: &mut P2) {
    let over = if cs.overscan { cs.overscan_percent / 100.0 * cs.speed / 60.0 } else { 0.0 };
    let ramp = cs.ramp_length.max(0.0);
    let mut forward = true;
    for (y, segs) in lines {
        let y = *y;
        let mut segs = segs.clone();
        if !forward {
            segs.reverse();
            for s in segs.iter_mut() {
                *s = (s.1, s.0);
            }
        }
        let dir = if forward { 1.0 } else { -1.0 };
        let first = segs[0].0 - dir * over;
        moves.push(Move { kind: MoveKind::Rapid, to: rotate([first, y], a), power: 0.0, speed: cs.speed, layer });
        for (x0, x1) in &segs {
            moves.push(Move { kind: MoveKind::Travel, to: rotate([*x0, y], a), power: 0.0, speed: cs.speed, layer });
            if ramp > 0.0 && !ramp_rings.is_empty() {
                let len = (x1 - x0).abs();
                let step = (ramp / RAMP_STEPS).max(RAMP_MIN_STEP_MM);
                let n = ((len / step).ceil() as usize).max(1);
                for k in 0..n {
                    let t0 = k as f64 / n as f64;
                    let t1 = (k + 1) as f64 / n as f64;
                    let mid = x0 + (x1 - x0) * (t0 + t1) / 2.0;
                    let p = ramp_power(low_power(cs), cs.max_power, edge_distance([mid, y], ramp_rings), ramp);
                    let to = x0 + (x1 - x0) * t1;
                    moves.push(Move { kind: MoveKind::Cut, to: rotate([to, y], a), power: p, speed: cs.speed, layer });
                }
            } else {
                moves.push(Move { kind: MoveKind::Cut, to: rotate([*x1, y], a), power: cs.max_power, speed: cs.speed, layer });
            }
        }
        let last = segs.last().unwrap().1 + dir * over;
        let pe = rotate([last, y], a);
        moves.push(Move { kind: MoveKind::Travel, to: pe, power: 0.0, speed: cs.speed, layer });
        *pos = pe;
        if cs.bidir {
            forward = !forward;
        }
    }
}

/// Scan-line fill of closed rings (even-odd), the rings given filled together.
fn fill_lines(rings: &[Polyline], cs: &CutSetting, angle_deg: f64, moves: &mut Vec<Move>, layer: usize, pos: &mut P2) {
    let a = angle_deg.to_radians();
    let rot: Vec<Vec<P2>> = rings.iter().filter(|r| r.closed && r.pts.len() > 2).map(|r| r.pts.iter().map(|p| rotate(*p, -a)).collect()).collect();
    if rot.is_empty() {
        return;
    }
    let lines = scan_segments(&rot, cs.interval.max(0.01));
    let ramp_rings = if cs.ramp_length > 0.0 { ramp_edges(&rot, cs.ramp_outer) } else { Vec::new() };
    if cs.flood_fill {
        for region in flood_regions(&lines) {
            emit_scan(&region, &ramp_rings, cs, a, moves, layer, pos);
        }
    } else {
        emit_scan(&lines, &ramp_rings, cs, a, moves, layer, pos);
    }
}

/// Most offset-fill loops per set of shapes (a guard; estimated).
const OFFSET_FILL_MAX_LOOPS: usize = 10_000;

/// Offset fill (type 2, "Generating offset fills...", CutBuilder::RasterizeShapes 0x100bc828c;
/// the path itself comes from the external acclplanner library): outlines shrunk by the line
/// interval, the first half an interval inside the edge (estimated), cut from the innermost
/// loop outwards like the editor's spiral picture (estimated).
pub fn offset_fill_loops(rings: &[Polyline], interval: f64) -> Vec<Polyline> {
    let interval = interval.max(0.01);
    let mut loops: Vec<Vec<Polyline>> = Vec::new();
    // each loop is the one before shrunk by one interval
    let mut cur: Vec<Polyline> = rings.iter().filter(|p| p.closed && p.pts.len() >= 3).cloned().collect();
    let mut d = interval / 2.0;
    for _ in 0..OFFSET_FILL_MAX_LOOPS {
        let ring: Vec<Polyline> = crate::offset::offset_paths(&cur, -d, crate::offset::Corner::Round, false).into_iter().filter(|p| p.pts.len() >= 3).collect();
        if ring.is_empty() {
            break;
        }
        loops.push(ring.clone());
        cur = ring;
        d = interval;
    }
    loops.into_iter().rev().flatten().collect()
}

fn offset_fill(rings: &[Polyline], cs: &CutSetting, moves: &mut Vec<Move>, layer: usize, pos: &mut P2) {
    for pl in offset_fill_loops(rings, cs.interval) {
        let start = pl.pts[0];
        moves.push(Move { kind: MoveKind::Rapid, to: start, power: 0.0, speed: cs.speed, layer });
        for p in pl.pts.iter().skip(1).chain(std::iter::once(&start)) {
            moves.push(Move { kind: MoveKind::Cut, to: *p, power: cs.max_power, speed: cs.speed, layer });
        }
        *pos = start;
    }
}

/// A Fill layer's shapes split by "scanOpt": all together, one set per outermost group
/// (a shape outside any group is its own set), or one per shape.
fn fill_sets(shapes: &[&Shape], grouping: crate::model::FillGrouping) -> Vec<Vec<Polyline>> {
    use crate::model::FillGrouping::*;
    let rings_of = |s: &Shape| -> Vec<Polyline> { s.world_paths().into_iter().filter(|p| p.closed).collect() };
    let vector: Vec<&&Shape> = shapes.iter().filter(|s| !matches!(s.geom, Geometry::Image { .. })).collect();
    match grouping {
        MergeAll => vec![vector.iter().flat_map(|s| rings_of(s)).collect()],
        Individual => vector.iter().map(|s| rings_of(s)).collect(),
        ByGroup => {
            let mut keys: Vec<u64> = Vec::new();
            let mut sets: Vec<Vec<Polyline>> = Vec::new();
            for s in vector {
                let k = s.groups.first().copied().unwrap_or(s.id);
                match keys.iter().position(|x| *x == k) {
                    Some(i) => sets[i].extend(rings_of(s)),
                    None => {
                        keys.push(k);
                        sets.push(rings_of(s));
                    }
                }
            }
            sets
        }
    }
}

/// Grey value 0..1 of one image at a world point, if the point lands on a pixel.
fn sample_image(shape: &Shape, x: f64, y: f64) -> Option<f64> {
    let Geometry::Image { w, h, px_w, px_h, gray } = &shape.geom else { return None };
    if *w <= 0.0 || *h <= 0.0 || *px_w == 0 || *px_h == 0 {
        return None;
    }
    let l = shape.to_local([x, y]);
    let u = (l[0] / w + 0.5) * *px_w as f64;
    let v = (0.5 - l[1] / h) * *px_h as f64;
    if u >= 0.0 && v >= 0.0 && (u as u32) < *px_w && (v as u32) < *px_h {
        Some(gray[(v as u32 * px_w + u as u32) as usize] as f64 / 255.0)
    } else {
        None
    }
}

/// Raster one or more image shapes on shared scan lines.
/// "Fill all at once" passes every picture; "one by one" passes each on its own.
fn image_lines(shapes: &[&Shape], cs: &CutSetting, pass: u32, passes: u32, moves: &mut Vec<Move>, layer: usize, pos: &mut P2) {
    let shapes: Vec<&Shape> = shapes.iter().copied().filter(|s| matches!(s.geom, Geometry::Image { .. })).collect();
    if shapes.is_empty() {
        return;
    }
    let mut b = BBox::empty();
    for s in &shapes {
        b.union(&s.bbox());
    }
    if !b.is_valid() {
        return;
    }
    let interval = cs.interval.max(0.02);
    let cols = ((b.w() / interval).ceil() as usize).max(1);
    let rows = ((b.h() / interval).ceil() as usize).max(1);
    // levels 0..1 (1 = burn fully); -1 is off the pictures
    let mut grid = vec![-1.0f64; cols * rows];
    for r in 0..rows {
        let y = b.max[1] - (r as f64 + 0.5) * interval;
        for c in 0..cols {
            let x = b.min[0] + (c as f64 + 0.5) * interval;
            if let Some(g) = shapes.iter().find_map(|s| sample_image(s, x, y)) {
                let mut level = 1.0 - g;
                if cs.negative {
                    level = 1.0 - level;
                }
                grid[r * cols + c] = level;
            }
        }
    }
    crate::imageproc::apply(cs, &mut grid, cols, rows, b.min[0], b.max[1], interval);
    if !cs.pass_through && cs.image_mode == ImageMode::Dot {
        dot_lines(&grid, cols, rows, &b, interval, cs, moves, layer, pos);
        return;
    }
    if !cs.pass_through && cs.image_mode == ImageMode::Slice3D {
        // 3D Slice: pass k of n burns, at full power, every point darker than k/n, so the darkest
        // grey is burned on every pass and goes deepest (one depth level per pass)
        // `pass` counts from 0 here
        let cut = pass as f64 / passes.max(1) as f64;
        for v in grid.iter_mut() {
            if *v >= 0.0 {
                *v = if *v > cut + 0.004 { 1.0 } else { 0.0 };
            }
        }
    }
    let continuous = cs.pass_through || matches!(cs.image_mode, ImageMode::Grayscale);
    let over = if cs.overscan { cs.overscan_percent / 100.0 * cs.speed / 60.0 } else { 0.0 };
    let ramp = cs.ramp_length.max(0.0);
    let mut forward = true;
    for r in 0..rows {
        let y = b.max[1] - (r as f64 + 0.5) * interval;
        let pw: Vec<f64> = (0..cols)
            .map(|c| {
                let v = grid[r * cols + c];
                if v <= 0.004 {
                    return 0.0;
                }
                let mut p = if continuous {
                    low_power(cs) + (cs.max_power - low_power(cs)) * v.clamp(0.0, 1.0)
                } else {
                    cs.max_power
                };
                if ramp > 0.0 && p > 0.0 {
                    let x = b.min[0] + (c as f64 + 0.5) * interval;
                    let dx = (x - b.min[0]).min(b.max[0] - x).max(0.0);
                    let dy = (y - b.min[1]).min(b.max[1] - y).max(0.0);
                    let t = (dx.min(dy) / ramp).clamp(0.0, 1.0);
                    let low = low_power(cs);
                    p = low + (p - low) * t;
                }
                p
            })
            .collect();
        let Some(first) = pw.iter().position(|p| *p > 0.0) else { continue };
        let last = pw.iter().rposition(|p| *p > 0.0).unwrap();
        let x_at = |c: usize| b.min[0] + c as f64 * interval;
        let cols_iter: Vec<usize> = if forward { (first..=last).collect() } else { (first..=last).rev().collect() };
        let dir = if forward { 1.0 } else { -1.0 };
        let start_x = if forward { x_at(first) } else { x_at(last + 1) };
        moves.push(Move { kind: MoveKind::Rapid, to: [start_x - dir * over, y], power: 0.0, speed: cs.speed, layer });
        moves.push(Move { kind: MoveKind::Travel, to: [start_x, y], power: 0.0, speed: cs.speed, layer });
        let mut cursor = start_x;
        let mut i = 0;
        while i < cols_iter.len() {
            let p = pw[cols_iter[i]];
            let mut j = i;
            while j + 1 < cols_iter.len() && (pw[cols_iter[j + 1]] - p).abs() < 0.5 {
                j += 1;
            }
            let end_c = cols_iter[j];
            let end_x = if forward { x_at(end_c + 1) } else { x_at(end_c) };
            let burn = p > 0.0;
            if burn && cs.dot_width_on && cs.dot_width > 0.0 {
                // the spot is wider than the drawn dot, so each burned run is shortened by half the spot on both ends
                let sign = if forward { 1.0 } else { -1.0 };
                let len = (end_x - cursor) * sign;
                let inset = cs.dot_width * 0.5;
                if len > cs.dot_width {
                    let burn_from = cursor + sign * inset;
                    let burn_to = end_x - sign * inset;
                    moves.push(Move { kind: MoveKind::Travel, to: [burn_from, y], power: 0.0, speed: cs.speed, layer });
                    moves.push(Move { kind: MoveKind::Cut, to: [burn_to, y], power: p, speed: cs.speed, layer });
                    moves.push(Move { kind: MoveKind::Travel, to: [end_x, y], power: 0.0, speed: cs.speed, layer });
                } else {
                    moves.push(Move { kind: MoveKind::Travel, to: [end_x, y], power: 0.0, speed: cs.speed, layer });
                }
            } else {
                let kind = if burn { MoveKind::Cut } else { MoveKind::Travel };
                moves.push(Move { kind, to: [end_x, y], power: p, speed: cs.speed, layer });
            }
            cursor = end_x;
            i = j + 1;
        }
        let end = if forward { x_at(last + 1) } else { x_at(first) };
        moves.push(Move { kind: MoveKind::Travel, to: [end + dir * over, y], power: 0.0, speed: cs.speed, layer });
        *pos = [end + dir * over, y];
        if cs.bidir {
            forward = !forward;
        }
    }
}

/// Dot mode: along each scan line, one dot every `dot_spacing` mm where the picture is not white.
/// The beam fires standing still at the maximum power; the stop is `dot_time_ms` for black and
/// shorter for lighter greys (constant power, time varying: the original's tooltip for Dot Mode).
#[allow(clippy::too_many_arguments)]
fn dot_lines(grid: &[f64], cols: usize, rows: usize, b: &BBox, interval: f64, cs: &CutSetting, moves: &mut Vec<Move>, layer: usize, pos: &mut P2) {
    let spacing = cs.dot_spacing.max(0.01);
    let full_us = (cs.dot_time_ms.max(0.0) * 1000.0).round();
    let width = b.max[0] - b.min[0];
    let dots = ((width / spacing).floor() as usize).max(1);
    let mut forward = true;
    for r in 0..rows {
        let y = b.max[1] - (r as f64 + 0.5) * interval;
        let mut line: Vec<(f64, u32)> = Vec::new();
        for k in 0..dots {
            let x = b.min[0] + (k as f64 + 0.5) * spacing;
            let c = (((x - b.min[0]) / interval) as usize).min(cols - 1);
            let v = grid[r * cols + c];
            if v > 0.004 {
                let us = (full_us * v.clamp(0.0, 1.0)).round() as u32;
                if us > 0 {
                    line.push((x, us));
                }
            }
        }
        if line.is_empty() {
            continue;
        }
        if !forward {
            line.reverse();
        }
        for (x, us) in line {
            moves.push(Move { kind: MoveKind::Dot { us }, to: [x, y], power: cs.max_power, speed: cs.speed, layer });
            *pos = [x, y];
        }
        if cs.bidir {
            forward = !forward;
        }
    }
}

/// Split shapes the way the Image window's three radios do.
fn shape_groups<'a>(shapes: &[&'a Shape], grouping: crate::model::FillGrouping) -> Vec<Vec<&'a Shape>> {
    use crate::model::FillGrouping::*;
    match grouping {
        MergeAll => vec![shapes.to_vec()],
        Individual => shapes.iter().copied().map(|s| vec![s]).collect(),
        ByGroup => {
            let mut keys: Vec<u64> = Vec::new();
            let mut sets: Vec<Vec<&Shape>> = Vec::new();
            for s in shapes {
                let k = s.groups.first().copied().unwrap_or(s.id);
                match keys.iter().position(|x| *x == k) {
                    Some(i) => sets[i].push(s),
                    None => {
                        keys.push(k);
                        sets.push(vec![s]);
                    }
                }
            }
            sets
        }
    }
}

/// Drop the points of an outline that bend it by less than `tol` mm (Douglas-Peucker). A closed
/// outline keeps at least a triangle, an open one its two ends.
fn simplify(p: Polyline, tol: f64) -> Polyline {
    if tol <= 0.0 || p.pts.len() < 4 {
        return p;
    }
    fn dp(pts: &[P2], tol: f64, keep: &mut [bool], a: usize, b: usize) {
        if b <= a + 1 {
            return;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
        let l = (dx * dx + dy * dy).sqrt();
        let mut far = (0.0, a);
        for i in a + 1..b {
            let d = if l > 1e-12 { ((pts[i][0] - pa[0]) * dy - (pts[i][1] - pa[1]) * dx).abs() / l } else { dist(pts[i], pa) };
            if d > far.0 {
                far = (d, i);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            dp(pts, tol, keep, a, far.1);
            dp(pts, tol, keep, far.1, b);
        }
    }
    let mut pts = p.pts.clone();
    if p.closed {
        pts.push(pts[0]);
    }
    let n = pts.len();
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    // a closed outline is split at its farthest point too, so it can't collapse to a line
    if p.closed {
        let far = (1..n - 1).max_by(|&i, &j| dist(pts[i], pts[0]).total_cmp(&dist(pts[j], pts[0]))).unwrap_or(1);
        keep[far] = true;
        dp(&pts, tol, &mut keep, 0, far);
        dp(&pts, tol, &mut keep, far, n - 1);
    } else {
        dp(&pts, tol, &mut keep, 0, n - 1);
    }
    let mut out: Vec<P2> = pts.iter().zip(&keep).filter(|(_, k)| **k).map(|(q, _)| *q).collect();
    if p.closed {
        out.pop();
        if out.len() < 3 {
            return p;
        }
    }
    Polyline { pts: out, closed: p.closed }
}

/// "Regolazione offset di scansione" (device settings): the ends of every scan line run backwards
/// (against the scan direction at `angle_deg`) are moved `shift` mm back along it, so on a
/// machine whose belts lag the two directions line up. Dots and connecting moves stay put.
fn scan_shift(moves: &mut [Move], angle_deg: f64, shift: f64) {
    if shift.abs() < 1e-9 || moves.len() < 2 {
        return;
    }
    let a = angle_deg.to_radians();
    let u = [a.cos(), a.sin()];
    let mut marked = vec![false; moves.len()];
    for i in 1..moves.len() {
        if matches!(moves[i].kind, MoveKind::Rapid | MoveKind::Dot { .. }) {
            continue;
        }
        let (p, q) = (moves[i - 1].to, moves[i].to);
        let d = [q[0] - p[0], q[1] - p[1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        // along the scan line, going backwards
        if l > 1e-9 && (d[0] * u[0] + d[1] * u[1]) / l < -0.999 {
            marked[i - 1] = true;
            marked[i] = true;
        }
    }
    for (m, k) in moves.iter_mut().zip(marked) {
        if k {
            m.to = [m.to[0] - shift * u[0], m.to[1] - shift * u[1]];
        }
    }
}

/// Whether any part of a shape is on the work area.
fn overlaps_work_area(doc: &Document, s: &Shape) -> bool {
    let b = s.bbox();
    !b.is_valid() || (b.max[0] >= 0.0 && b.max[1] >= 0.0 && b.min[0] <= doc.device.width && b.min[1] <= doc.device.height)
}

/// Build the job in world coordinates (mm).
pub fn plan(doc: &Document, selection: &[u64], opts: &JobOptions) -> Job {
    let mut job = Job::default();
    let shapes: Vec<&Shape> = doc
        .shapes
        .iter()
        .filter(|s| !opts.selected_only || selection.contains(&s.id))
        .filter(|s| !is_tool_layer(s.layer))
        .filter(|s| doc.layers[s.layer].output)
        // "Ignora le forme fuori limite se possibile": a shape wholly outside the work area is left out
        .filter(|s| !crate::prefs::with(|p| p.ignore_out_of_bounds) || overlaps_work_area(doc, s))
        .collect();
    let mut bounds = BBox::empty();
    for s in &shapes {
        bounds.union(&s.bbox());
    }
    if !bounds.is_valid() {
        return job;
    }
    job.bounds = Some(bounds);
    // "Usa origine selezione": the origin comes from the selected shapes' box
    let mut origin_box = bounds;
    if opts.selection_origin && !selection.is_empty() {
        let mut sb = BBox::empty();
        for s in doc.shapes.iter().filter(|s| selection.contains(&s.id)) {
            sb.union(&s.bbox());
        }
        if sb.is_valid() {
            origin_box = sb;
        }
    }
    let mut pos = match opts.start_from {
        StartFrom::AbsoluteCoords => [0.0, 0.0],
        _ => origin_box.anchor(opts.job_origin),
    };
    job.start = pos;
    for layer in doc.used_layers() {
        if is_tool_layer(layer) {
            continue;
        }
        let cs = &doc.layers[layer];
        if !cs.output {
            continue;
        }
        let layer_shapes: Vec<&Shape> = shapes.iter().copied().filter(|s| s.layer == layer).collect();
        if layer_shapes.is_empty() {
            continue;
        }
        let passes = cs.passes.max(1);
        // A picture is engraved as an image. A vector layer cannot be sent as one.
        let mode = doc.layer_art(layer).clamp(cs.mode);
        for pass in 0..passes {
            job.sections.push((layer, pass + 1, passes, job.moves.len()));
            match mode {
                CutMode::Line => {
                    let paths: Vec<Polyline> = layer_shapes
                        .iter()
                        .filter(|s| !matches!(s.geom, Geometry::Image { .. }))
                        .flat_map(|s| s.world_paths())
                        .filter(|p| p.pts.len() > 1)
                        // "Tolleranza curva (mm)" (Impostazioni > Importa/Esporta): points that bend the
                        // line by less than this are dropped
                        .map(|p| simplify(p, crate::prefs::with(|q| q.curve_tolerance)))
                        .collect();
                    let rings: Vec<Polyline> = paths.iter().filter(|p| p.closed).cloned().collect();
                    let manual: Vec<P2> = if cs.tabs_enabled && cs.manual_tabs {
                        layer_shapes.iter().flat_map(|s| s.tab_points()).collect()
                    } else {
                        Vec::new()
                    };
                    for pl in order_paths(paths, pos, opts.optimize) {
                        line_path(&pl, &rings, cs, layer, &manual, &mut job.moves);
                        pos = job.moves.last().unwrap().to;
                    }
                }
                CutMode::Fill => {
                    // flood fill works region by region, so grouping doesn't apply (the editor
                    // disables it, CutSettingEdit::on_cbFloodFill_stateChanged 0x100b86bf4)
                    let grouping = if cs.flood_fill { crate::model::FillGrouping::MergeAll } else { cs.fill_grouping };
                    for rings in fill_sets(&layer_shapes, grouping) {
                        let from = job.moves.len();
                        fill_lines(&rings, cs, cs.angle, &mut job.moves, layer, &mut pos);
                        scan_shift(&mut job.moves[from..], cs.angle, doc.device.scan_shift(cs.speed));
                        if cs.crosshatch {
                            let from = job.moves.len();
                            fill_lines(&rings, cs, cs.angle + 90.0, &mut job.moves, layer, &mut pos);
                            scan_shift(&mut job.moves[from..], cs.angle + 90.0, doc.device.scan_shift(cs.speed));
                        }
                    }
                }
                CutMode::OffsetFill => {
                    let rings: Vec<Polyline> = fill_sets(&layer_shapes, crate::model::FillGrouping::MergeAll).into_iter().flatten().collect();
                    offset_fill(&rings, cs, &mut job.moves, layer, &mut pos);
                }
                CutMode::Image => {
                    let (images, vectors): (Vec<&Shape>, Vec<&Shape>) = layer_shapes.iter().copied().partition(|s| matches!(s.geom, Geometry::Image { .. }));
                    let from = job.moves.len();
                    for rings in fill_sets(&vectors, cs.fill_grouping) {
                        fill_lines(&rings, cs, cs.angle, &mut job.moves, layer, &mut pos);
                    }
                    scan_shift(&mut job.moves[from..], cs.angle, doc.device.scan_shift(cs.speed));
                    let from = job.moves.len();
                    for group in shape_groups(&images, cs.fill_grouping) {
                        image_lines(&group, cs, pass, passes, &mut job.moves, layer, &mut pos);
                    }
                    scan_shift(&mut job.moves[from..], 0.0, doc.device.scan_shift(cs.speed));
                }
            }
        }
    }
    job
}

fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn unit(v: P2) -> P2 {
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if l > 1e-12 { [v[0] / l, v[1] / l] } else { [1.0, 0.0] }
}

fn left_normal(d: P2) -> P2 {
    [-d[1], d[0]]
}

/// Point at distance `s` along a polyline (clamped to its ends).
fn point_at(pts: &[P2], mut s: f64) -> P2 {
    for w in pts.windows(2) {
        let l = dist(w[0], w[1]);
        if s <= l && l > 1e-12 {
            let t = s / l;
            return [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
        }
        s -= l;
    }
    *pts.last().unwrap()
}

fn poly_len(pts: &[P2]) -> f64 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// The side of a closed ring the waste is on: +1 = left of travel, -1 = right. Outer shapes
/// keep the waste outside, holes (inside an odd number of other rings) keep it inside.
fn waste_side(pts: &[P2], inner: bool) -> f64 {
    let ccw = ring_area(pts) > 0.0;
    // left of travel is inside for a counter-clockwise ring
    let left_is_inside = ccw;
    if left_is_inside == inner { 1.0 } else { -1.0 }
}

/// Offset a closed ring by `k` mm towards side `s` (miter joins, limited at sharp corners).
fn offset_ring(pts: &[P2], k: f64, s: f64) -> Vec<P2> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let p = pts[i];
            let a = pts[(i + n - 1) % n];
            let b = pts[(i + 1) % n];
            let n1 = left_normal(unit(sub(p, a)));
            let n2 = left_normal(unit(sub(b, p)));
            let den = (1.0 + n1[0] * n2[0] + n1[1] * n2[1]).max(0.25);
            [p[0] + s * k * (n1[0] + n2[0]) / den, p[1] + s * k * (n1[1] + n2[1]) / den]
        })
        .collect()
}

/// Lead points from `p` along `d` (unit), bending `angle` degrees towards side `s`. For an
/// arc the lead is a circular arc of radius `len` tangent to the path at `p`.
fn lead_points(p: P2, d: P2, len: f64, angle: f64, s: f64, style: crate::model::LeadStyle, lead_in: bool) -> Vec<P2> {
    let n = left_normal(d);
    let n = [n[0] * s, n[1] * s];
    // direction away from the path: backwards for a lead-in, forwards for a lead-out
    let dir = if lead_in { -1.0 } else { 1.0 };
    match style {
        crate::model::LeadStyle::Line => {
            let a = angle.to_radians();
            vec![[p[0] + len * (dir * a.cos() * d[0] + a.sin() * n[0]), p[1] + len * (dir * a.cos() * d[1] + a.sin() * n[1])]]
        }
        crate::model::LeadStyle::Arc => {
            let c = [p[0] + len * n[0], p[1] + len * n[1]];
            let sweep = angle.clamp(1.0, 180.0).to_radians();
            let t_end = (p[1] - c[1]).atan2(p[0] - c[0]);
            // travelling along d at p means θ grows when the centre is on the left (s·side = +1)
            let rot = s;
            let steps = ((sweep * len).abs() / 0.5).ceil().clamp(4.0, 64.0) as usize;
            (1..=steps)
                .map(|i| {
                    let f = i as f64 / steps as f64;
                    let t = t_end + dir * rot * sweep * f;
                    [c[0] + len * t.cos(), c[1] + len * t.sin()]
                })
                .collect()
        }
    }
}

/// One Line-mode path with kerf offset, lead-in, overcut, lead-out, tabs and perforation
/// (CUT_PARAMS fields, MECHANICS.md "Cut settings editor").
fn line_path(pl: &Polyline, rings: &[Polyline], cs: &CutSetting, layer: usize, manual_tabs: &[P2], moves: &mut Vec<Move>) {
    let mut main: Vec<P2> = pl.pts.clone();
    let mut inner = false;
    let mut side = 1.0;
    if pl.closed && main.len() >= 3 {
        let others: Vec<Polyline> = rings.iter().filter(|r| r.pts != pl.pts).cloned().collect();
        inner = point_in_rings(main[0], &others);
        side = waste_side(&main, inner);
        if cs.kerf_enabled && cs.kerf.abs() > 1e-9 {
            main = offset_ring(&main, cs.kerf, side);
        }
        main.push(main[0]);
    } else if pl.closed {
        main.push(main[0]);
    }
    let len = poly_len(&main);
    if len < 1e-9 {
        return;
    }
    // overcut: carry on along the path past the start
    let mut path = main.clone();
    if pl.closed && cs.overcut > 0.0 {
        let mut s = 0.0;
        for w in main.windows(2) {
            let l = dist(w[0], w[1]);
            if s + l >= cs.overcut {
                path.push(point_at(&[w[0], w[1]], cs.overcut - s));
                break;
            }
            path.push(w[1]);
            s += l;
        }
    }
    // tabs: gaps (or reduced power) evenly spread over closed shapes
    let mut tabs: Vec<(f64, f64)> = Vec::new();
    if cs.tabs_enabled && !cs.manual_tabs && pl.closed && !(inner && cs.skip_inner_tabs) && cs.tab_size > 0.0 {
        let mut count = if cs.tabs_use_spacing { (len / cs.tab_spacing.max(0.1)).floor().max(1.0) as u32 } else { cs.tab_count.max(1) };
        if cs.limit_max_tabs {
            count = count.min(cs.tab_count_max.max(1));
        }
        let step = len / count as f64;
        if cs.tab_size < step {
            for i in 0..count {
                let c = (i as f64 + 0.5) * step;
                tabs.push((c - cs.tab_size / 2.0, c + cs.tab_size / 2.0));
            }
        }
    }
    // manual tabs (placed on the canvas): each one sitting on this path becomes a gap centred on it
    if cs.tabs_enabled && cs.manual_tabs && cs.tab_size > 0.0 && !manual_tabs.is_empty() {
        let tol = 1.0 + if cs.kerf_enabled { cs.kerf.abs() } else { 0.0 };
        for &t in manual_tabs {
            let mut best: Option<(f64, f64)> = None;
            let mut acc = 0.0;
            for w in main.windows(2) {
                let l = dist(w[0], w[1]);
                let d = crate::model::dist_seg(t, w[0], w[1]);
                let u = if l > 1e-12 {
                    (((t[0] - w[0][0]) * (w[1][0] - w[0][0]) + (t[1] - w[0][1]) * (w[1][1] - w[0][1])) / (l * l)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, acc + u * l));
                }
                acc += l;
            }
            if let Some((d, s)) = best {
                if d <= tol {
                    tabs.push((s - cs.tab_size / 2.0, s + cs.tab_size / 2.0));
                    if pl.closed {
                        // a tab across the seam also covers the other end
                        if s - cs.tab_size / 2.0 < 0.0 {
                            tabs.push((s - cs.tab_size / 2.0 + len, len + 1.0));
                        }
                        if s + cs.tab_size / 2.0 > len {
                            tabs.push((-1.0, s + cs.tab_size / 2.0 - len));
                        }
                    }
                }
            }
        }
    }
    let tab_power = if cs.tab_power_enabled { cs.max_power * cs.tab_cut_power / 100.0 } else { 0.0 };
    let period = cs.perf_len + cs.perf_skip;
    let perforate = cs.perforate && cs.perf_len > 0.0 && cs.perf_skip > 0.0;
    let power_at = |s: f64| -> f64 {
        if tabs.iter().any(|&(a, b)| s > a && s < b) {
            return tab_power;
        }
        if perforate && s.rem_euclid(period) >= cs.perf_len {
            return 0.0;
        }
        cs.max_power
    };
    let push = |moves: &mut Vec<Move>, to: P2, power: f64| {
        let kind = if power > 0.0 { MoveKind::Cut } else { MoveKind::Travel };
        moves.push(Move { kind, to, power, speed: cs.speed, layer });
    };

    let d0 = unit(sub(path[1.min(path.len() - 1)], path[0]));
    let mut start = path[0];
    let mut lead_in_pts = Vec::new();
    if cs.lead_in && cs.lead_in_len > 0.0 {
        let mut pts = lead_points(path[0], d0, cs.lead_in_len, cs.lead_in_angle, side, cs.lead_style, true);
        pts.reverse();
        start = pts[0];
        lead_in_pts = pts.into_iter().skip(1).chain(std::iter::once(path[0])).collect();
    }
    moves.push(Move { kind: MoveKind::Rapid, to: start, power: 0.0, speed: cs.speed, layer });
    for p in lead_in_pts {
        push(moves, p, cs.max_power);
    }
    let mut s = 0.0;
    for w in path.windows(2) {
        let l = dist(w[0], w[1]);
        if l < 1e-12 {
            continue;
        }
        let mut cuts: Vec<f64> = Vec::new();
        for &(a, b) in &tabs {
            for x in [a, b] {
                if x > s && x < s + l {
                    cuts.push(x);
                }
            }
        }
        if perforate {
            let mut k = (s / period).floor();
            while k * period < s + l {
                for x in [k * period, k * period + cs.perf_len] {
                    if x > s + 1e-9 && x < s + l - 1e-9 {
                        cuts.push(x);
                    }
                }
                k += 1.0;
            }
        }
        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        cuts.push(s + l);
        let mut prev = s;
        for x in cuts {
            if x - prev < 1e-9 {
                continue;
            }
            let p = point_at(&[w[0], w[1]], x - s);
            push(moves, p, power_at((prev + x) / 2.0));
            prev = x;
        }
        s += l;
    }
    if cs.lead_out && cs.lead_out_len > 0.0 && path.len() >= 2 {
        let e = path[path.len() - 1];
        let d1 = unit(sub(e, path[path.len() - 2]));
        for p in lead_points(e, d1, cs.lead_out_len, cs.lead_in_angle, side, cs.lead_style, false) {
            push(moves, p, cs.max_power);
        }
    }
}

/// Coordinates: 3 decimals, S: 1 decimal, F: 2 decimals (constant 0x1018f3e68,
/// Protocol_GCode_Base ctor 0x1005de7f0); trailing zeros dropped (LB_NumPrint 0x1005e8690).
const COORD_DECIMALS: usize = 3;
const S_DECIMALS: usize = 1;
const F_DECIMALS: usize = 2;
/// An axis or S value is written only when it changed by at least this (EmitMoveOrCut 0x1005e4644).
const MOVE_EPS: f64 = 0.0025;
/// F is written only when the speed changed by more than this (EmitMoveOrCut 0x1005e4644).
const F_EPS: f64 = 0.01;

fn num_d(v: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

fn num(v: f64) -> String {
    num_d(v, COORD_DECIMALS)
}

/// GRBL jog line (Protocol_GCode_Base::JogXY_New 0x1005e91d0): "$J=", G90 (absolute) or G91
/// (relative), G21, both axes, and an mm/min feed rate with 2 decimals.
pub fn jog_line(absolute: bool, x: f64, y: f64, speed_mm_min: f64) -> String {
    format!(
        "$J={} G21 X{} Y{} F{}",
        if absolute { "G90" } else { "G91" },
        num(x),
        num(y),
        num_d(speed_mm_min, F_DECIMALS)
    )
}

/// GRBL "set origin": one `G10 L20 P1 <axis>0` per axis (Protocol_GRBL::ZeroAxis 0x10062bd88).
pub fn zero_axes_lines() -> Vec<String> {
    vec!["G10 L20 P1 X0".into(), "G10 L20 P1 Y0".into()]
}

/// GRBL home command (Protocol_GRBL::GetHomeXYCommand 0x10062ae70).
pub const HOME_CMD: &str = "$H";

/// GRBL abort: the single real-time byte Ctrl-X (Protocol_GRBL::GetAbortCommand 0x10062aecc).
pub const ABORT_BYTE: u8 = 0x18;

/// Fire button feed rate, "G1 F100 S…" (Protocol_GRBL::GetFireOnCommand 0x10062b0c8).
pub const FIRE_FEED: u32 = 100;

/// Laser on/off (Protocol_GRBL::GetLaserOnOffCommand 0x10062afd4): off = M5; on = M4 only with
/// dynamic power, otherwise M3.
pub fn laser_cmd(on: bool, dynamic_power: bool) -> &'static str {
    match (on, dynamic_power) {
        (false, _) => "M5",
        (true, true) => "M4",
        (true, false) => "M3",
    }
}

/// Air assist on/off (Protocol_GRBL::GetAirAssistOnOffCommand 0x10062af10): off = M9;
/// on = M7 when the device setting "AirAssistM7" is set, else M8.
pub fn air_cmd(on: bool, air_assist_m7: bool) -> &'static str {
    match (on, air_assist_m7) {
        (false, _) => "M9",
        (true, true) => "M7",
        (true, false) => "M8",
    }
}

/// Fire on (Protocol_GRBL::GetFireOnCommand 0x10062b0c8): laser-on command, then
/// `G1 F100 S<pct × S_Scale × 0.01, 1 decimal>`.
pub fn fire_on_lines(pct: f64, s_max: f64, dynamic_power: bool) -> Vec<String> {
    vec![
        laser_cmd(true, dynamic_power).to_string(),
        format!("G1 F{} S{}", FIRE_FEED, num_d(pct * s_max * 0.01, S_DECIMALS)),
    ]
}

/// Fire off (Protocol_GRBL::GetFireOffCommand 0x10062b174): `S0`, `G0`, `M5`.
pub fn fire_off_lines() -> Vec<String> {
    vec!["S0".into(), "G0".into(), "M5".into()]
}

/// World point to machine coordinates for the chosen start mode.
fn to_machine(doc: &Document, job: &Job, opts: &JobOptions, p: P2) -> P2 {
    let d = &doc.device;
    match opts.start_from {
        StartFrom::AbsoluteCoords => {
            use crate::model::OriginCorner::*;
            let x = match d.origin {
                FrontRight | RearRight => d.width - p[0],
                _ => p[0],
            };
            let y = match d.origin {
                RearLeft | RearRight => d.height - p[1],
                _ => p[1],
            };
            [x, y]
        }
        _ => [p[0] - job.start[0], p[1] - job.start[1]],
    }
}

pub fn write_gcode(doc: &Document, job: &Job, opts: &JobOptions) -> String {
    let d = &doc.device;
    let mut out = String::new();
    let relative = opts.start_from == StartFrom::CurrentPosition;
    out.push_str("; RayEtch 1.0.0 - free laser software\n");
    out.push_str(&format!("; {} device profile, {}\n", d.name, match opts.start_from {
        StartFrom::AbsoluteCoords => "absolute coords",
        StartFrom::UserOrigin => "user origin",
        StartFrom::CurrentPosition => "current position",
    }));
    if let Some(b) = job.bounds {
        let a = to_machine(doc, job, opts, b.min);
        let c = to_machine(doc, job, opts, b.max);
        out.push_str(&format!(
            "; Bounds: X{} Y{} to X{} Y{}\n",
            num(a[0].min(c[0])),
            num(a[1].min(c[1])),
            num(a[0].max(c[0])),
            num(a[1].max(c[1]))
        ));
    }
    out.push_str("G00 G17 G40 G21 G54\n");
    out.push_str(if relative { "G91\n" } else { "G90\n" });
    let mut laser_on = laser_cmd(true, d.dynamic_power);
    out.push_str(laser_on);
    out.push('\n');

    let mut last_machine = to_machine(doc, job, opts, job.start);
    let mut last_f: Option<f64> = None;
    let mut last_s: Option<f64> = None;
    let mut air_on = false;
    let mut section_iter = job.sections.iter().peekable();
    for (i, m) in job.moves.iter().enumerate() {
        while let Some(&&(layer, pass, passes, start)) = section_iter.peek() {
            if start != i {
                break;
            }
            section_iter.next();
            let cs = &doc.layers[layer];
            out.push_str(&format!("; Layer {} Pass {} of {}\n", layer_name(layer), pass, passes));
            // "forceConstantPower" (+0x1c0): this layer runs with M3 even on a dynamic-power device
            let on = laser_cmd(true, d.dynamic_power && !cs.constant_power);
            if on != laser_on {
                out.push_str(on);
                out.push('\n');
                laser_on = on;
            }
            out.push_str(&format!("; {} @ {} mm/min, {}% power\n", cs.mode.label(), num(cs.speed), num(cs.max_power)));
            if cs.air != air_on {
                out.push_str(air_cmd(cs.air, d.air_assist_m7));
                out.push('\n');
                air_on = cs.air;
            }
        }
        // Dot mode: rapid to the dot, then fire standing still. GRBL keeps the beam on during a
        // G4 pause only under M3, so each dot is M3 S… / G4 P… / M5 (the pause is in seconds).
        if let MoveKind::Dot { us } = m.kind {
            let mp = to_machine(doc, job, opts, m.to);
            let dx = mp[0] - last_machine[0];
            let dy = mp[1] - last_machine[1];
            if dx.abs() >= MOVE_EPS || dy.abs() >= MOVE_EPS {
                let (x, y) = if relative { (dx, dy) } else { (mp[0], mp[1]) };
                out.push_str(&format!("G0 X{}Y{}\n", num(x), num(y)));
                last_machine = mp;
            }
            let s = m.power / 100.0 * d.s_max;
            out.push_str(&format!("M3 S{}\nG4 P{}\nM5\n", num_d(s, S_DECIMALS), num_d(us as f64 / 1e6, 6)));
            laser_on = "M5";
            last_s = None;
            continue;
        }
        // Protocol_GCode::EmitMoveOrCut (0x1005e4644): "G0 "/"G1 ", then X, Y, S, F written
        // back to back, each only when it changed.
        let mp = to_machine(doc, job, opts, m.to);
        let dx = mp[0] - last_machine[0];
        let dy = mp[1] - last_machine[1];
        let write_x = dx.abs() >= MOVE_EPS;
        let write_y = dy.abs() >= MOVE_EPS;
        let mut words = String::new();
        let mut emitted = last_machine;
        if write_x {
            words.push_str(&format!("X{}", num(if relative { dx } else { mp[0] })));
            emitted[0] = mp[0];
        }
        if write_y {
            words.push_str(&format!("Y{}", num(if relative { dy } else { mp[1] })));
            emitted[1] = mp[1];
        }
        let gword = match m.kind {
            MoveKind::Rapid | MoveKind::Dot { .. } => "G0 ",
            _ => "G1 ",
        };
        if !write_x && !write_y {
            continue;
        }
        if !matches!(m.kind, MoveKind::Rapid) {
            // S = power% / 100 × S-max (EmitMoveOrCut 0x1005e4644, S-max field +0xd4)
            let s = if matches!(m.kind, MoveKind::Cut) { m.power / 100.0 * d.s_max } else { 0.0 };
            if last_s.is_none_or(|ls| (ls - s).abs() >= MOVE_EPS) {
                words.push_str(&format!("S{}", num_d(s, S_DECIMALS)));
                last_s = Some(s);
            }
            // GRBL feed rates are mm/min; layer speeds are already stored in that unit.
            let f = m.speed;
            if last_f.is_none_or(|lf| (lf - f).abs() > F_EPS) {
                words.push_str(&format!("F{}", num_d(f, F_DECIMALS)));
                last_f = Some(f);
            }
        }
        out.push_str(gword);
        out.push_str(&words);
        out.push('\n');
        last_machine = emitted;
    }
    out.push_str(air_cmd(false, d.air_assist_m7));
    out.push('\n');
    out.push_str("G1 S0\n");
    out.push_str(laser_cmd(false, d.dynamic_power));
    out.push('\n');
    match opts.start_from {
        StartFrom::CurrentPosition => {
            out.push_str("; return to starting pos\n");
            out.push_str(&format!("G0 X{} Y{}\n", num(-last_machine[0]), num(-last_machine[1])));
            out.push_str("G90\n");
        }
        StartFrom::UserOrigin => {
            out.push_str("; return to starting pos\n");
            out.push_str("G0 X0 Y0\n");
        }
        StartFrom::AbsoluteCoords => {
            out.push_str("G90\n");
            out.push_str("; return to user-defined finish pos\n");
            out.push_str("G0 X0 Y0\n");
        }
    }
    out.push_str("M2\n");
    out
}

/// Frame power, % of S-max: the person asked for the beam at 1% while framing.
pub const FRAME_POWER_PCT: f64 = 1.0;
/// Position Laser pulses on arrival (person's request): count and power (%).
pub const POSITION_PULSES: u32 = 3;
pub const POSITION_PULSE_PCT: f64 = 1.0;
/// Pulse length 1 s (person's request); gap between pulses in seconds (estimated).
pub const POSITION_PULSE_ON_S: f64 = 1.0;
pub const POSITION_PULSE_GAP_S: f64 = 0.5;

/// S value for a power percentage, like the Fire button (pct × S_Scale × 0.01).
pub fn s_for_pct(pct: f64, s_max: f64) -> String {
    num_d(pct * s_max * 0.01, S_DECIMALS)
}

/// After the head arrives (G4 waits for motion to finish), blink the beam `POSITION_PULSES`
/// times at `POSITION_PULSE_PCT`. Constant power (M3) so it fires while standing still.
pub fn position_pulse_lines(s_max: f64) -> Vec<String> {
    let mut v = vec!["G4 P0".to_string()];
    for _ in 0..POSITION_PULSES {
        v.push("M3".into());
        v.push(format!("G1 F{} S{}", FIRE_FEED, s_for_pct(POSITION_PULSE_PCT, s_max)));
        v.push(format!("G4 P{}", num(POSITION_PULSE_ON_S)));
        v.push("S0".into());
        v.push("M5".into());
        v.push(format!("G4 P{}", num(POSITION_PULSE_GAP_S)));
    }
    v
}

/// Trace a closed path: reach the first point with the beam off, then beam on at `power`
/// (an S value) around the path, then beam off. Always constant power (M3): with M4 the
/// beam dims or goes out while the head slows at corners, and the person wants it continuous.
/// Frame beam S value: 1% of S-max when LaserFrame_Enable is on, otherwise beam off
/// (LightBurn's default framing, TraceTightFrame 0x100c5b604).
pub fn frame_power_s(laser_frame: bool, s_max: f64) -> String {
    if laser_frame { s_for_pct(FRAME_POWER_PCT, s_max) } else { "0".into() }
}

fn trace_path(doc: &Document, job: &Job, opts: &JobOptions, path: &[P2], speed_mm_min: f64, power: &str) -> Vec<String> {
    let mut v = Vec::new();
    let relative = opts.start_from == StartFrom::CurrentPosition;
    v.push(if relative { "G91".into() } else { "G90".into() });
    let mut last = to_machine(doc, job, opts, job.start);
    // "Abilita offset puntatore": framing without the beam is followed by the red-dot pointer,
    // which sits `pointer_offset` from the beam, so the head goes that much the other way
    let d = &doc.device;
    let shift = if d.pointer_offset_on && power == "0" { d.pointer_offset } else { [0.0, 0.0] };
    for (i, &c) in path.iter().enumerate() {
        let mp = to_machine(doc, job, opts, [c[0] - shift[0], c[1] - shift[1]]);
        let t = if relative { [mp[0] - last[0], mp[1] - last[1]] } else { mp };
        if i == 0 {
            v.push(format!("G1 X{} Y{} S0 F{}", num(t[0]), num(t[1]), num(speed_mm_min)));
            v.push(laser_cmd(true, false).into());
        } else {
            v.push(format!("G1 X{} Y{} S{} F{}", num(t[0]), num(t[1]), power, num(speed_mm_min)));
        }
        last = mp;
    }
    v.push("S0".into());
    v.push("M5".into());
    if relative {
        let back = to_machine(doc, job, opts, job.start);
        v.push(format!("G1 X{} Y{} S0", num(back[0] - last[0]), num(back[1] - last[1])));
        v.push("G90".into());
    }
    v
}

/// G-code lines that trace the job's bounding box with the beam at `power` (an S value).
pub fn frame_gcode(doc: &Document, job: &Job, opts: &JobOptions, speed_mm_min: f64, power: &str) -> Vec<String> {
    let Some(b) = job.bounds else { return vec![] };
    let corners = [b.min, [b.max[0], b.min[1]], b.max, [b.min[0], b.max[1]], b.min];
    trace_path(doc, job, opts, &corners, speed_mm_min, power)
}

/// Convex hull (counter-clockwise, no repeated first point) of a point set.
pub fn convex_hull(mut pts: Vec<P2>) -> Vec<P2> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: P2, a: P2, b: P2| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut h: Vec<P2> = Vec::with_capacity(pts.len() * 2);
    for pass in 0..2 {
        let start = h.len();
        let iter: Box<dyn Iterator<Item = &P2>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in iter {
            while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
    }
    h
}

/// Points the burn passes through: both ends of every cutting move.
pub fn burn_points(job: &Job) -> Vec<P2> {
    let mut pts = Vec::new();
    let mut prev = job.start;
    for m in &job.moves {
        if matches!(m.kind, MoveKind::Cut) {
            pts.push(prev);
            pts.push(m.to);
        }
        prev = m.to;
    }
    pts
}

/// "Inquadra forme": trace the rubber-band (convex hull) boundary around the burnt shapes with
/// the beam at frame power (on_actionRubber_band_outline_from_selection_triggered 0x100c5dd90 builds a
/// ConvexHull of the shapes; the tooltip at 0x102245766 says the speed is the Move panel's).
/// Rubber-band frame speed when the Move panel speed is 0 or less: 50 mm/s
/// (TraceTightFrame 0x100c5b604, constant 0x42480000).
pub const OUTLINE_FRAME_FALLBACK_MM_S: f64 = 50.0;

pub fn outline_frame_gcode(doc: &Document, job: &Job, opts: &JobOptions, speed_mm_min: f64, power: &str) -> Vec<String> {
    let speed_mm_min = if speed_mm_min > 0.0 { speed_mm_min } else { OUTLINE_FRAME_FALLBACK_MM_S * 60.0 };
    let hull = convex_hull(burn_points(job));
    if hull.is_empty() {
        return vec![];
    }
    let mut path = hull.clone();
    path.push(hull[0]);
    trace_path(doc, job, opts, &path, speed_mm_min, power)
}

#[allow(dead_code)]
pub fn inside(p: P2, rings: &[Polyline]) -> bool {
    point_in_rings(p, rings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Geometry, Shape};

    #[test]
    fn grbl_fire_and_air_commands() {
        // GetFireOnCommand 0x10062b0c8: 2% of S-max 1000 → S20, feed 100
        assert_eq!(fire_on_lines(2.0, 1000.0, true), vec!["M4", "G1 F100 S20"]);
        assert_eq!(fire_on_lines(2.5, 255.0, false), vec!["M3", "G1 F100 S6.4"]);
        assert_eq!(fire_off_lines(), vec!["S0", "G0", "M5"]);
        assert_eq!(air_cmd(true, false), "M8");
        assert_eq!(air_cmd(true, true), "M7");
        assert_eq!(air_cmd(false, true), "M9");
        assert_eq!(ABORT_BYTE, 0x18);
    }

    #[test]
    fn rect_line_and_fill() {
        let mut doc = Document::default();
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 20.0, h: 10.0, radius: 0.0 }, 0, 50.0, 50.0));
        doc.add_shape(Shape::new(0, Geometry::Ellipse { rx: 5.0, ry: 5.0 }, 1, 100.0, 100.0));
        doc.layers[1].mode = CutMode::Fill;
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: true, selection_origin: false };
        let job = plan(&doc, &[], &opts);
        let g = write_gcode(&doc, &job, &opts);
        println!("{}", g.lines().take(14).collect::<Vec<_>>().join("\n"));
        assert!(g.contains("G00 G17 G40 G21 G54\nG90\nM4\n"));
        // default speed 100 mm/s = F6000 (CUT_PARAMS ctor 0x1009f4744)
        assert!(g.contains("G1 X60S200F6000"));
        assert!(g.contains("; Layer C01 Pass 1 of 1"));
        assert!(g.trim_end().ends_with("M2"));
        assert!(job.estimate_seconds(400.0, 3000.0) > job.cut_length() / (6000.0 / 60.0));
    }

    #[test]
    fn convex_hull_drops_inner_points() {
        let h = convex_hull(vec![[0.0, 0.0], [10.0, 0.0], [5.0, 2.0], [10.0, 10.0], [0.0, 10.0], [5.0, 5.0], [0.0, 0.0]]);
        assert_eq!(h.len(), 4);
        assert!(!h.contains(&[5.0, 5.0]) && !h.contains(&[5.0, 2.0]));
    }

    #[test]
    fn outline_frame_follows_the_shapes_not_the_box() {
        let mut doc = Document::default();
        // a triangle-like spread: two small squares far apart diagonally
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 20.0, 20.0));
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 80.0, 80.0));
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: true, selection_origin: false };
        let job = plan(&doc, &[], &opts);
        let o = outline_frame_gcode(&doc, &job, &opts, 3000.0, &s_for_pct(FRAME_POWER_PCT, 1000.0));
        // hull of two diagonal squares has 6 corners (+ closing move), the box frame 5 moves
        let g1: Vec<_> = o.iter().filter(|l| l.starts_with("G1")).collect();
        assert_eq!(g1.len(), 7);
        // reach the start beam off, then trace at 1% (S10 of 1000), then beam off
        assert!(g1[0].contains("S0"));
        assert!(g1[1..].iter().all(|l| l.contains("S10 ")));
        assert_eq!(o[2], "M3");
        assert_eq!(&o[o.len() - 2..], &["S0", "M5"]);
    }

    #[test]
    fn laser_frame_setting_turns_the_frame_beam_on_or_off() {
        assert_eq!(frame_power_s(true, 1000.0), "10");
        assert_eq!(frame_power_s(false, 1000.0), "0");
    }

    #[test]
    fn position_pulses_three_times_at_one_percent_after_arrival() {
        let v = position_pulse_lines(1000.0);
        assert_eq!(v[0], "G4 P0");
        assert_eq!(v.iter().filter(|l| *l == "G1 F100 S10").count(), 3);
        assert_eq!(v.iter().filter(|l| *l == "G4 P1").count(), 3);
        assert_eq!(v.last().unwrap(), "G4 P0.5");
        assert!(v.contains(&"M5".to_string()));
    }

    #[test]
    fn cutting_feed_rate_is_emitted_as_mm_per_minute() {
        let mut doc = Document::default();
        doc.layers[0].speed = 2500.0;
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 20.0, 20.0));
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: true, selection_origin: false };
        let job = plan(&doc, &[], &opts);
        let g = write_gcode(&doc, &job, &opts);
        assert!(g.contains("F2500"));
        assert!(!g.contains("F150000"));
    }

    #[test]
    fn jog_feed_rate_is_emitted_as_mm_per_minute() {
        assert_eq!(jog_line(false, 1.0, -2.0, 900.0), "$J=G91 G21 X1 Y-2 F900");
    }

    fn square_job(setup: impl Fn(&mut CutSetting)) -> Job {
        let mut doc = Document::default();
        setup(&mut doc.layers[0]);
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 100.0, h: 100.0, radius: 0.0 }, 0, 100.0, 100.0));
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: false, selection_origin: false };
        plan(&doc, &[], &opts)
    }

    fn cut_len(job: &Job, kind: MoveKind) -> f64 {
        let mut prev = job.start;
        let mut total = 0.0;
        for m in &job.moves {
            if std::mem::discriminant(&m.kind) == std::mem::discriminant(&kind) {
                total += dist(prev, m.to);
            }
            prev = m.to;
        }
        total
    }

    #[test]
    fn tabs_leave_uncut_gaps_by_spacing() {
        let job = square_job(|cs| {
            cs.tabs_enabled = true;
            cs.tab_size = 2.0;
            cs.tab_spacing = 50.0;
        });
        // 400 mm perimeter / 50 mm spacing = 8 tabs of 2 mm
        assert!((cut_len(&job, MoveKind::Travel) - 16.0).abs() < 1e-6);
        assert!((cut_len(&job, MoveKind::Cut) - 384.0).abs() < 1e-6);
    }

    #[test]
    fn manual_tabs_leave_gaps_only_where_placed() {
        let mut doc = Document::default();
        doc.layers[0].tabs_enabled = true;
        doc.layers[0].manual_tabs = true;
        doc.layers[0].tab_size = 3.0;
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 100.0, h: 100.0, radius: 0.0 }, 0, 100.0, 100.0));
        doc.shapes[0].tabs = vec![(0, 0.1), (0, 0.6)];
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: false, selection_origin: false };
        let job = plan(&doc, &[], &opts);
        assert!((cut_len(&job, MoveKind::Travel) - 6.0).abs() < 1e-6);
        assert!((cut_len(&job, MoveKind::Cut) - 394.0).abs() < 1e-6);
    }

    #[test]
    fn perforation_alternates_cut_and_skip() {
        let job = square_job(|cs| {
            cs.perforate = true;
            cs.perf_len = 3.0;
            cs.perf_skip = 1.0;
        });
        assert!((cut_len(&job, MoveKind::Cut) - 300.0).abs() < 1e-6);
    }

    #[test]
    fn kerf_grows_outer_shape_and_overcut_adds_length() {
        let job = square_job(|cs| {
            cs.kerf_enabled = true;
            cs.kerf = 1.0;
            cs.overcut = 5.0;
        });
        assert!((cut_len(&job, MoveKind::Cut) - (408.0 + 5.0)).abs() < 1e-6);
    }

    #[test]
    fn line_lead_in_starts_outside_the_shape() {
        let job = square_job(|cs| {
            cs.lead_in = true;
            cs.lead_in_len = 5.0;
            cs.lead_in_angle = 90.0;
            cs.lead_style = crate::model::LeadStyle::Line;
        });
        let start = job.moves[0].to;
        let corner = job.moves[1].to;
        assert!((dist(start, corner) - 5.0).abs() < 1e-6);
        // the lead lies outside the square (the waste side of an outer shape)
        let ring = vec![Polyline { pts: job.moves[1..5].iter().map(|m| m.to).collect(), closed: true }];
        assert!(!point_in_rings(start, &ring));
    }

    #[test]
    fn fill_options() {
        // ramp: min at the edge, max once the ramp length in
        assert_eq!(ramp_power(10.0, 50.0, 0.0, 2.0), 10.0);
        assert_eq!(ramp_power(10.0, 50.0, 1.0, 2.0), 30.0);
        assert_eq!(ramp_power(10.0, 50.0, 5.0, 2.0), 50.0);
        assert_eq!(ramp_power(10.0, 50.0, 0.0, 0.0), 50.0);
        // flood fill: two separate columns become two regions
        let lines = vec![(0.0, vec![(0.0, 1.0), (5.0, 6.0)]), (1.0, vec![(0.0, 1.0), (5.0, 6.0)])];
        assert_eq!(flood_regions(&lines).len(), 2);
        // offset fill: a 10 mm square at 1 mm interval gives 5 loops, innermost first
        let sq = Polyline { pts: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], closed: true };
        let loops = offset_fill_loops(&[sq], 1.0);
        assert_eq!(loops.len(), 5);
        let w = |p: &Polyline| p.pts.iter().map(|q| q[0]).fold(f64::MIN, f64::max) - p.pts.iter().map(|q| q[0]).fold(f64::MAX, f64::min);
        assert!(w(&loops[0]) < w(&loops[4]));
        // grouping: two shapes filled one by one give two sets
        let mut doc = Document::default();
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 10.0, 10.0));
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 40.0, 10.0));
        let shapes: Vec<&Shape> = doc.shapes.iter().collect();
        assert_eq!(fill_sets(&shapes, crate::model::FillGrouping::Individual).len(), 2);
        assert_eq!(fill_sets(&shapes, crate::model::FillGrouping::MergeAll).len(), 1);
        assert_eq!(fill_sets(&shapes, crate::model::FillGrouping::ByGroup).len(), 2);
        // constant power switches the layer to M3
        doc.layers[0].mode = CutMode::OffsetFill;
        doc.layers[0].constant_power = true;
        doc.device.dynamic_power = true;
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: true, selection_origin: false };
        let job = plan(&doc, &[], &opts);
        assert!(!job.moves.is_empty());
        let g = write_gcode(&doc, &job, &opts);
        assert!(g.contains("; Layer C00 Pass 1 of 1\nM3\n"));
    }

    fn image_job(gray: Vec<u8>, tweak: impl FnOnce(&mut CutSetting)) -> Job {
        let mut doc = Document::default();
        let px = gray.len() as u32;
        doc.layers[0].mode = CutMode::Image;
        doc.layers[0].interval = 1.0;
        doc.layers[0].overscan = false;
        doc.layers[0].image_mode = ImageMode::Threshold;
        doc.layers[0].bidir = false;
        tweak(&mut doc.layers[0]);
        let w = px as f64;
        doc.add_shape(Shape::new(0, Geometry::Image { w, h: 1.0, px_w: px, px_h: 1, gray }, 0, w / 2.0, 0.5));
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: false, selection_origin: false };
        plan(&doc, &[], &opts)
    }

    #[test]
    fn image_modes_change_what_the_laser_burns() {
        // a white picture does not burn; Negative turns it on
        let plain = image_job(vec![255, 255, 255, 255], |_| {});
        assert!(cut_len(&plain, MoveKind::Cut) < 1e-6);
        let neg = image_job(vec![255, 255, 255, 255], |cs| cs.negative = true);
        assert!(cut_len(&neg, MoveKind::Cut) > 1.0);
        // dot-width correction shortens the burned run by the spot size
        let full = image_job(vec![0, 0, 0, 0], |_| {});
        let corrected = image_job(vec![0, 0, 0, 0], |cs| {
            cs.dot_width_on = true;
            cs.dot_width = 1.0;
        });
        assert!((cut_len(&full, MoveKind::Cut) - cut_len(&corrected, MoveKind::Cut) - 1.0).abs() < 1e-6);
        // pass-through keeps the grey instead of a hard on/off
        let pass = image_job(vec![0, 128, 255, 0], |cs| {
            cs.pass_through = true;
            cs.min_power = 0.0;
            cs.max_power = 100.0;
        });
        let powers: Vec<f64> = pass.moves.iter().filter(|m| m.kind == MoveKind::Cut).map(|m| m.power).collect();
        assert!(powers.iter().any(|p| *p > 10.0 && *p < 90.0), "powers {powers:?}");
    }

    #[test]
    fn dot_mode_stops_longer_on_darker_dots() {
        let job = image_job(vec![0, 128, 255, 0], |cs| {
            cs.image_mode = ImageMode::Dot;
            cs.dot_spacing = 1.0;
            cs.dot_time_ms = 2.0;
        });
        let dots: Vec<u32> = job.moves.iter().filter_map(|m| if let MoveKind::Dot { us } = m.kind { Some(us) } else { None }).collect();
        // black = the full 2 ms, mid grey about half, white no dot
        assert_eq!(dots.len(), 3, "dots {dots:?}");
        assert_eq!(dots[0], 2000);
        assert!(dots[1] > 800 && dots[1] < 1200);
        let doc = Document::default();
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: false, selection_origin: false };
        let g = write_gcode(&doc, &job, &opts);
        assert!(g.contains("M3 S"), "{g}");
        assert!(g.contains("G4 P0.002\nM5"), "{g}");
    }

    #[test]
    fn scan_offset_moves_only_backward_lines() {
        let mut d = crate::model::Device::default();
        d.scan_offset_on = true;
        d.scan_offsets = vec![crate::model::ScanOffset { speed: 100.0, shift: 0.2, start: 0.0, laser2: false }];
        // 6000 mm/min = 100 mm/s → 0.2 mm
        assert!((d.scan_shift(6000.0) - 0.2).abs() < 1e-9);
        let mv = |kind, x: f64, y: f64| Move { kind, to: [x, y], power: 50.0, speed: 6000.0, layer: 0 };
        let mut moves = vec![
            mv(MoveKind::Rapid, 0.0, 0.0),
            mv(MoveKind::Cut, 10.0, 0.0),
            mv(MoveKind::Travel, 10.0, 0.1),
            mv(MoveKind::Cut, 0.0, 0.1),
        ];
        scan_shift(&mut moves, 0.0, d.scan_shift(6000.0));
        // the forward line stays, the backward line (both its ends) moves 0.2 mm left
        assert_eq!(moves[1].to, [10.0, 0.0]);
        assert!((moves[2].to[0] - 9.8).abs() < 1e-9 && (moves[3].to[0] + 0.2).abs() < 1e-9);
    }

    #[test]
    fn slice_3d_burns_less_on_each_pass() {
        // levels 1.0, 0.69, 0.41, 0.18 against the pass cuts 0, 1/4, 2/4, 3/4
        let job = image_job(vec![0, 80, 150, 210], |cs| {
            cs.image_mode = ImageMode::Slice3D;
            cs.passes = 4;
        });
        // the burned length shrinks from pass to pass: darker greys go deeper
        let mut lens = Vec::new();
        for (i, s) in job.sections.iter().enumerate() {
            let end = job.sections.get(i + 1).map(|n| n.3).unwrap_or(job.moves.len());
            let mut last = if s.3 > 0 { job.moves[s.3 - 1].to } else { job.start };
            let mut d = 0.0;
            for m in &job.moves[s.3..end] {
                if m.kind == MoveKind::Cut {
                    d += dist(last, m.to);
                }
                last = m.to;
            }
            lens.push(d);
        }
        assert_eq!(lens.len(), 4);
        assert!(lens[0] > lens[1] && lens[1] > lens[2] && lens[2] > lens[3] && lens[3] > 0.5, "lens {lens:?}");
    }
}
