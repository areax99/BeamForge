//! DXF, HPGL / PLT and G-code drawings.

use super::{Piece, arc_points, circle, finish};
use crate::i18n::tr;
use crate::model::Polyline;
use std::collections::HashMap;

type P = [f64; 2];

// ---------------------------------------------------------------- DXF

/// mm per drawing unit for $INSUNITS (ParseHeader 0x100088454 → table 0x1018f487c →
/// DXFTranslator::SetUnits 0x1000878ec, factors at 0x1018f4864). Unitless and miles stay 1 (mm).
fn dxf_unit(insunits: i32) -> f64 {
    match insunits {
        1 => 25.4,
        2 => 304.8,
        4 => 1.0,
        5 => 10.0,
        6 => 1000.0,
        7 => 1.0e6,
        8 => 0.0254,
        _ => 1.0,
    }
}

/// DXF colours become layers in order of first appearance (DXFTranslator::GetColorIndex
/// 0x100097348: the colour's position in the list of colours seen, new ones appended up to 24).
struct ColorOrder(Vec<i32>);
impl ColorOrder {
    fn layer(&mut self, aci: i32) -> usize {
        if let Some(i) = self.0.iter().position(|c| *c == aci) {
            return i;
        }
        if self.0.len() <= 0x17 {
            self.0.push(aci);
            self.0.len() - 1
        } else {
            self.0.len() - 1
        }
    }
}

#[derive(Clone, Default)]
struct Ent {
    kind: String,
    codes: Vec<(i32, String)>,
}
impl Ent {
    fn f(&self, code: i32) -> Option<f64> {
        self.codes.iter().find(|c| c.0 == code).and_then(|c| c.1.trim().parse().ok())
    }
    fn fd(&self, code: i32, d: f64) -> f64 {
        self.f(code).unwrap_or(d)
    }
    fn s(&self, code: i32) -> Option<&str> {
        self.codes.iter().find(|c| c.0 == code).map(|c| c.1.trim())
    }
    fn all(&self, code: i32) -> Vec<f64> {
        self.codes.iter().filter(|c| c.0 == code).filter_map(|c| c.1.trim().parse().ok()).collect()
    }
}

struct Block {
    base: P,
    ents: Vec<Ent>,
}

/// An affine map applied to INSERTed block contents.
#[derive(Clone, Copy)]
struct Xf {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}
impl Xf {
    fn ap(&self, p: P) -> P {
        [self.a * p[0] + self.c * p[1] + self.e, self.b * p[0] + self.d * p[1] + self.f]
    }
    fn then(&self, o: &Xf) -> Xf {
        // o ∘ self
        Xf {
            a: o.a * self.a + o.c * self.b,
            b: o.b * self.a + o.d * self.b,
            c: o.a * self.c + o.c * self.d,
            d: o.b * self.c + o.d * self.d,
            e: o.a * self.e + o.c * self.f + o.e,
            f: o.b * self.e + o.d * self.f + o.f,
        }
    }
}

fn bulge_to(out: &mut Vec<P>, a: P, b: P, bulge: f64) {
    if bulge.abs() < 1e-9 {
        out.push(b);
        return;
    }
    let theta = 4.0 * bulge.atan();
    let chord = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    if chord < 1e-12 {
        return;
    }
    let r = chord / (2.0 * (theta / 2.0).sin());
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let h = r * (theta / 2.0).cos();
    let n = [-(b[1] - a[1]) / chord, (b[0] - a[0]) / chord];
    let c = [mid[0] + n[0] * h, mid[1] + n[1] * h];
    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    let mut pts = arc_points(c, r.abs(), a0, a0 + theta);
    if let Some(l) = pts.last_mut() {
        *l = b;
    }
    out.extend(pts);
}

fn bspline(ctrl: &[P], degree: usize, knots: &[f64]) -> Vec<P> {
    let n = ctrl.len();
    if n < 2 {
        return ctrl.to_vec();
    }
    let p = degree.clamp(1, n - 1);
    let knots: Vec<f64> = if knots.len() == n + p + 1 {
        knots.to_vec()
    } else {
        // clamped uniform knots
        (0..n + p + 1).map(|i| if i <= p { 0.0 } else if i >= n { (n - p) as f64 } else { (i - p) as f64 }).collect()
    };
    let (t0, t1) = (knots[p], knots[n]);
    let steps = (n * 12).clamp(16, 2000);
    (0..=steps)
        .map(|s| {
            let t = (t0 + (t1 - t0) * s as f64 / steps as f64).min(t1 - 1e-12).max(t0);
            let mut k = p;
            while k < n - 1 && t >= knots[k + 1] {
                k += 1;
            }
            // de Boor
            let mut d: Vec<P> = (0..=p).map(|j| ctrl[j + k - p]).collect();
            for r in 1..=p {
                for j in (r..=p).rev() {
                    let i = j + k - p;
                    let den = knots[i + p + 1 - r] - knots[i];
                    let al = if den.abs() < 1e-12 { 0.0 } else { (t - knots[i]) / den };
                    d[j] = [(1.0 - al) * d[j - 1][0] + al * d[j][0], (1.0 - al) * d[j - 1][1] + al * d[j][1]];
                }
            }
            d[p]
        })
        .collect()
}

fn read_pairs(text: &str) -> Vec<(i32, String)> {
    let mut lines = text.lines();
    let mut out = Vec::new();
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        if let Ok(code) = c.trim().parse::<i32>() {
            out.push((code, v.to_string()));
        }
    }
    out
}

fn entities_of(pairs: &[(i32, String)]) -> Vec<Ent> {
    let mut v: Vec<Ent> = Vec::new();
    for (code, val) in pairs {
        if *code == 0 {
            v.push(Ent { kind: val.trim().to_uppercase(), codes: Vec::new() });
        } else if let Some(e) = v.last_mut() {
            e.codes.push((*code, val.clone()));
        }
    }
    v
}

pub fn dxf(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let text = String::from_utf8_lossy(bytes);
    if !text.contains("SECTION") {
        return Err(tr("Il file DXF non è valido (forse è un DXF binario o non è un DXF).").into());
    }
    let pairs = read_pairs(&text);
    // split sections
    let mut sections: HashMap<String, Vec<(i32, String)>> = HashMap::new();
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].0 == 0 && pairs[i].1.trim() == "SECTION" && i + 1 < pairs.len() && pairs[i + 1].0 == 2 {
            let name = pairs[i + 1].1.trim().to_uppercase();
            let mut j = i + 2;
            while j < pairs.len() && !(pairs[j].0 == 0 && pairs[j].1.trim() == "ENDSEC") {
                j += 1;
            }
            sections.insert(name, pairs[i + 2..j.min(pairs.len())].to_vec());
            i = j;
        }
        i += 1;
    }
    let mut unit = 1.0;
    if let Some(h) = sections.get("HEADER") {
        for w in h.windows(2) {
            if w[0].0 == 9 && w[0].1.trim() == "$INSUNITS" && w[1].0 == 70 {
                unit = dxf_unit(w[1].1.trim().parse().unwrap_or(0));
            }
        }
    }
    // layer colours (TABLES / LAYER, code 62)
    let mut layer_colors: HashMap<String, i32> = HashMap::new();
    if let Some(t) = sections.get("TABLES") {
        for e in entities_of(t) {
            if e.kind == "LAYER" {
                if let (Some(n), Some(c)) = (e.s(2), e.f(62)) {
                    layer_colors.insert(n.to_string(), (c as i32).abs());
                }
            }
        }
    }
    let mut blocks: HashMap<String, Block> = HashMap::new();
    if let Some(b) = sections.get("BLOCKS") {
        let mut cur: Option<(String, Block)> = None;
        for e in entities_of(b) {
            match e.kind.as_str() {
                "BLOCK" => cur = Some((e.s(2).unwrap_or("").to_string(), Block { base: [e.fd(10, 0.0), e.fd(20, 0.0)], ents: Vec::new() })),
                "ENDBLK" => {
                    if let Some((n, bl)) = cur.take() {
                        blocks.insert(n, bl);
                    }
                }
                _ => {
                    if let Some((_, bl)) = cur.as_mut() {
                        bl.ents.push(e);
                    }
                }
            }
        }
    }
    let ents = entities_of(sections.get("ENTITIES").map(|v| v.as_slice()).unwrap_or(&[]));
    let mut ctx = DxfCtx { order: ColorOrder(Vec::new()), layer_colors, blocks: &blocks, pieces: Vec::new() };
    let base = Xf { a: unit, b: 0.0, c: 0.0, d: unit, e: 0.0, f: 0.0 };
    ctx.emit(&ents, &base, 7, 0);
    if ctx.pieces.is_empty() {
        return Err(tr("Il file DXF non contiene forme utilizzabili.").into());
    }
    Ok(ctx.pieces)
}

struct DxfCtx<'a> {
    order: ColorOrder,
    layer_colors: HashMap<String, i32>,
    blocks: &'a HashMap<String, Block>,
    pieces: Vec<Piece>,
}

impl DxfCtx<'_> {
    fn color(&self, e: &Ent, inherited: i32) -> i32 {
        match e.f(62).map(|c| c as i32) {
            Some(0) => inherited,           // BYBLOCK
            Some(c) if c != 256 => c.abs(), // own colour
            _ => e.s(8).and_then(|l| self.layer_colors.get(l).copied()).unwrap_or(7),
        }
    }

    fn emit(&mut self, ents: &[Ent], xf: &Xf, inherited: i32, depth: usize) {
        let mut k = 0;
        while k < ents.len() {
            let e = &ents[k];
            let aci = self.color(e, inherited);
            let mut paths: Vec<Polyline> = Vec::new();
            let t = |p: P| xf.ap(p);
            match e.kind.as_str() {
                "LINE" => paths.push(Polyline { pts: vec![t([e.fd(10, 0.0), e.fd(20, 0.0)]), t([e.fd(11, 0.0), e.fd(21, 0.0)])], closed: false }),
                "CIRCLE" => {
                    let c = circle([e.fd(10, 0.0), e.fd(20, 0.0)], e.fd(40, 0.0));
                    paths.push(Polyline { pts: c.pts.into_iter().map(t).collect(), closed: true });
                }
                "ARC" => {
                    let c = [e.fd(10, 0.0), e.fd(20, 0.0)];
                    let r = e.fd(40, 0.0);
                    let a0 = e.fd(50, 0.0).to_radians();
                    let mut a1 = e.fd(51, 360.0).to_radians();
                    while a1 <= a0 {
                        a1 += std::f64::consts::TAU;
                    }
                    let mut pts = vec![[c[0] + r * a0.cos(), c[1] + r * a0.sin()]];
                    pts.extend(arc_points(c, r, a0, a1));
                    paths.push(Polyline { pts: pts.into_iter().map(t).collect(), closed: false });
                }
                "ELLIPSE" => {
                    let c = [e.fd(10, 0.0), e.fd(20, 0.0)];
                    let mj = [e.fd(11, 1.0), e.fd(21, 0.0)];
                    let ratio = e.fd(40, 1.0);
                    let (s0, mut s1) = (e.fd(41, 0.0), e.fd(42, std::f64::consts::TAU));
                    while s1 <= s0 {
                        s1 += std::f64::consts::TAU;
                    }
                    let mn = [-mj[1] * ratio, mj[0] * ratio];
                    let n = 96;
                    let full = (s1 - s0 - std::f64::consts::TAU).abs() < 1e-6;
                    let pts: Vec<P> = (0..=n)
                        .filter(|i| !(full && *i == n))
                        .map(|i| {
                            let a = s0 + (s1 - s0) * i as f64 / n as f64;
                            t([c[0] + mj[0] * a.cos() + mn[0] * a.sin(), c[1] + mj[1] * a.cos() + mn[1] * a.sin()])
                        })
                        .collect();
                    paths.push(Polyline { pts, closed: full });
                }
                "LWPOLYLINE" => {
                    let closed = (e.fd(70, 0.0) as i32) & 1 == 1;
                    // vertices in order: 10, 20, optional 42 (bulge)
                    let mut verts: Vec<(P, f64)> = Vec::new();
                    for (code, v) in &e.codes {
                        let v: f64 = v.trim().parse().unwrap_or(0.0);
                        match code {
                            10 => verts.push(([v, 0.0], 0.0)),
                            20 => {
                                if let Some(l) = verts.last_mut() {
                                    l.0[1] = v
                                }
                            }
                            42 => {
                                if let Some(l) = verts.last_mut() {
                                    l.1 = v
                                }
                            }
                            _ => {}
                        }
                    }
                    paths.push(poly_with_bulges(&verts, closed, xf));
                }
                "POLYLINE" => {
                    let closed = (e.fd(70, 0.0) as i32) & 1 == 1;
                    let mut verts = Vec::new();
                    let mut j = k + 1;
                    while j < ents.len() && ents[j].kind == "VERTEX" {
                        verts.push(([ents[j].fd(10, 0.0), ents[j].fd(20, 0.0)], ents[j].fd(42, 0.0)));
                        j += 1;
                    }
                    if j < ents.len() && ents[j].kind == "SEQEND" {
                        j += 1;
                    }
                    k = j - 1;
                    paths.push(poly_with_bulges(&verts, closed, xf));
                }
                "SPLINE" => {
                    let degree = e.fd(71, 3.0) as usize;
                    let knots = e.all(40);
                    let xs = e.all(10);
                    let ys = e.all(20);
                    let ctrl: Vec<P> = xs.iter().zip(ys.iter()).map(|(x, y)| [*x, *y]).collect();
                    let pts = if ctrl.len() >= 2 {
                        bspline(&ctrl, degree, &knots)
                    } else {
                        e.all(11).iter().zip(e.all(21).iter()).map(|(x, y)| [*x, *y]).collect()
                    };
                    let closed = (e.fd(70, 0.0) as i32) & 1 == 1;
                    paths.push(finish(pts.into_iter().map(t).collect(), 1e-6).tap_closed(closed));
                }
                "SOLID" | "3DFACE" => {
                    let mut pts: Vec<P> = [(10, 20), (11, 21), (13, 23), (12, 22)].iter().filter_map(|(a, b)| Some([e.f(*a)?, e.f(*b)?])).collect();
                    if e.kind == "3DFACE" && pts.len() == 4 {
                        pts.swap(2, 3);
                    }
                    pts.dedup();
                    paths.push(Polyline { pts: pts.into_iter().map(t).collect(), closed: true });
                }
                "INSERT" if depth < 16 => {
                    if let Some(b) = e.s(2).and_then(|n| self.blocks.get(n)) {
                        let (sx, sy) = (e.fd(41, 1.0), e.fd(42, 1.0));
                        let r = e.fd(50, 0.0).to_radians();
                        let (cols, rows) = (e.fd(70, 1.0).max(1.0) as usize, e.fd(71, 1.0).max(1.0) as usize);
                        let (dc, dr) = (e.fd(44, 0.0), e.fd(45, 0.0));
                        let ents = b.ents.clone();
                        for ri in 0..rows {
                            for ci in 0..cols {
                                let local = Xf { a: sx, b: 0.0, c: 0.0, d: sy, e: -b.base[0] * sx, f: -b.base[1] * sy };
                                let (cs, sn) = (r.cos(), r.sin());
                                let off = [ci as f64 * dc, ri as f64 * dr];
                                let place = Xf {
                                    a: cs,
                                    b: sn,
                                    c: -sn,
                                    d: cs,
                                    e: e.fd(10, 0.0) + cs * off[0] - sn * off[1],
                                    f: e.fd(20, 0.0) + sn * off[0] + cs * off[1],
                                };
                                let m = local.then(&place).then(xf);
                                self.emit(&ents, &m, aci, depth + 1);
                            }
                        }
                    }
                }
                _ => {}
            }
            if !paths.is_empty() {
                let layer = self.order.layer(aci);
                self.pieces.push(Piece { layer, paths });
            }
            k += 1;
        }
    }
}

trait TapClosed {
    fn tap_closed(self, c: bool) -> Self;
}
impl TapClosed for Polyline {
    fn tap_closed(mut self, c: bool) -> Self {
        self.closed |= c;
        self
    }
}

fn poly_with_bulges(verts: &[(P, f64)], closed: bool, xf: &Xf) -> Polyline {
    let mut pts = Vec::new();
    if let Some(first) = verts.first() {
        pts.push(first.0);
    }
    for w in verts.windows(2) {
        bulge_to(&mut pts, w[0].0, w[1].0, w[0].1);
    }
    if closed && verts.len() > 1 {
        let (l, f) = (verts[verts.len() - 1], verts[0]);
        bulge_to(&mut pts, l.0, f.0, l.1);
        pts.pop();
    }
    Polyline { pts: pts.into_iter().map(|p| xf.ap(p)).collect(), closed }
}

// ---------------------------------------------------------------- HPGL

/// Plotter units for PU/PD/PA/PR (HPGLTranslator::Import 0x1000ac1f4, constant 0x3cd013aa).
pub const HPGL_UNIT_MM: f64 = 0.0254;
/// AA arc centres are read as 1016 per inch (÷1016 × 25.4, constants 0x447e0000 / 0x41cb3333).
pub const HPGL_ARC_UNIT_MM: f64 = 25.4 / 1016.0;
/// A finished outline closes when its ends are this close (PathShape::Close(0.001)).
pub const HPGL_CLOSE_TOL: f64 = 0.001;

pub fn hpgl(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut pieces: Vec<Piece> = Vec::new();
    let mut layer = 0usize;
    let mut pos: P = [0.0, 0.0];
    let mut down = false;
    let mut absolute = true;
    let mut cur: Vec<P> = Vec::new();
    let mut dots: Vec<P> = Vec::new();
    let flush = |cur: &mut Vec<P>, layer: usize, pieces: &mut Vec<Piece>, dots: &mut Vec<P>| {
        if cur.len() > 2 || (cur.len() == 2 && cur[0] != cur[1]) {
            pieces.push(Piece { layer, paths: vec![finish(std::mem::take(cur), HPGL_CLOSE_TOL)] });
        } else if cur.len() == 2 {
            // a pen-down on the spot: a dot (EllipseShape in the original)
            dots.push(cur[0]);
        }
        cur.clear();
    };
    for cmd in text.split([';', '\n', '\r']).flat_map(split_hpgl_commands) {
        let (op, args) = cmd;
        let nums: Vec<f64> = args.split([',', ' ']).filter(|s| !s.is_empty()).filter_map(|s| s.trim().parse().ok()).collect();
        match op.as_str() {
            "IN" => {
                absolute = true;
            }
            "SP" => {
                flush(&mut cur, layer, &mut pieces, &mut dots);
                layer = nums.first().map(|p| (*p as usize).saturating_sub(1)).unwrap_or(0).min(29);
            }
            "PA" => absolute = true,
            "PR" => absolute = false,
            "PU" | "PD" | "PA_" => {}
            _ => {}
        }
        if matches!(op.as_str(), "PU" | "PD" | "PA" | "PR") {
            if op == "PU" {
                flush(&mut cur, layer, &mut pieces, &mut dots);
                down = false;
            } else if op == "PD" {
                down = true;
            }
            for xy in nums.chunks(2).filter(|c| c.len() == 2) {
                let p = [xy[0] * HPGL_UNIT_MM, xy[1] * HPGL_UNIT_MM];
                let next = if absolute { p } else { [pos[0] + p[0], pos[1] + p[1]] };
                if down {
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    cur.push(next);
                }
                pos = next;
            }
        } else if op == "CI" {
            if let Some(r) = nums.first() {
                pieces.push(Piece { layer, paths: vec![circle(pos, r * HPGL_UNIT_MM)] });
            }
        } else if op == "AA" || op == "AR" {
            if nums.len() >= 3 {
                let c = if op == "AA" { [nums[0] * HPGL_ARC_UNIT_MM, nums[1] * HPGL_ARC_UNIT_MM] } else { [pos[0] + nums[0] * HPGL_ARC_UNIT_MM, pos[1] + nums[1] * HPGL_ARC_UNIT_MM] };
                let r = ((pos[0] - c[0]).powi(2) + (pos[1] - c[1]).powi(2)).sqrt();
                let a0 = (pos[1] - c[1]).atan2(pos[0] - c[0]);
                let a1 = a0 + nums[2].to_radians();
                let pts = arc_points(c, r, a0, a1);
                if down {
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    cur.extend(pts.iter().copied());
                }
                pos = *pts.last().unwrap_or(&pos);
            }
        }
    }
    flush(&mut cur, layer, &mut pieces, &mut dots);
    for d in dots {
        pieces.push(Piece { layer, paths: vec![circle(d, 0.05)] });
    }
    if pieces.is_empty() {
        return Err(tr("Il file HPGL non contiene tracciati.").into());
    }
    Ok(pieces)
}

/// "PD100,200PU" → [("PD","100,200"), ("PU","")]: two letters start a command.
fn split_hpgl_commands(s: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() && i + 1 < b.len() && b[i + 1].is_ascii_alphabetic() {
            out.push((format!("{}{}", b[i], b[i + 1]).to_uppercase(), String::new()));
            i += 2;
        } else if b[i].is_ascii_alphabetic() {
            i += 1;
        } else {
            if let Some(l) = out.last_mut() {
                l.1.push(b[i]);
            }
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------- G-code

/// Inches → mm for G20 (GCodeTranslator::Import 0x10009b354, constant 0x4039000000000000).
pub const GCODE_INCH: f64 = 25.4;

/// Reads the moves of a G-code file back into outlines: G0/G1/G2/G3 motion modes, G20/G21 units,
/// G90/G91 positioning, laser on with M3/M4 and off with M5 (M106/M107 count as on/off for fan-
/// driven lasers); only feed moves with the laser on draw (GCodeTranslator::Import 0x10009b354).
pub fn gcode(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut pieces: Vec<Piece> = Vec::new();
    let mut pos: P = [0.0, 0.0];
    let mut mode = 0; // 0 rapid, 1 line, 2 cw, 3 ccw
    let mut inch = false;
    let mut relative = false;
    let mut laser = false;
    let mut power = 0.0f64;
    let mut cur: Vec<P> = Vec::new();
    let flush = |cur: &mut Vec<P>, pieces: &mut Vec<Piece>| {
        if cur.len() > 1 {
            pieces.push(Piece { layer: 0, paths: vec![finish(std::mem::take(cur), 1e-4)] });
        }
        cur.clear();
    };
    for raw in text.lines() {
        // strip ; comments and ( ) comments
        let line = raw.split(';').next().unwrap_or("");
        let mut clean = String::new();
        let mut depth = 0;
        for ch in line.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth = (depth - 1).max(0),
                _ if depth == 0 => clean.push(ch.to_ascii_uppercase()),
                _ => {}
            }
        }
        let words = gcode_words(&clean);
        let mut target: [Option<f64>; 2] = [None, None];
        let (mut ci, mut cj) = (0.0, 0.0);
        let mut radius: Option<f64> = None;
        let mut has_s = false;
        for (letter, v) in &words {
            match letter {
                'G' => match *v as i32 {
                    0 | 1 | 2 | 3 if (v.fract()).abs() < 1e-6 => mode = *v as i32,
                    20 => inch = true,
                    21 => inch = false,
                    90 => relative = false,
                    91 => relative = true,
                    _ => {}
                },
                'M' => match *v as i32 {
                    3 | 4 | 106 => laser = true,
                    5 | 107 => laser = false,
                    _ => {}
                },
                'X' => target[0] = Some(*v),
                'Y' => target[1] = Some(*v),
                'I' => ci = *v,
                'J' => cj = *v,
                'R' => radius = Some(*v),
                'S' => {
                    power = *v;
                    has_s = true;
                }
                _ => {}
            }
        }
        let _ = has_s;
        if target[0].is_none() && target[1].is_none() {
            continue;
        }
        let k = if inch { GCODE_INCH } else { 1.0 };
        let mut next = pos;
        for a in 0..2 {
            if let Some(v) = target[a] {
                next[a] = if relative { pos[a] + v * k } else { v * k };
            }
        }
        let burning = laser && mode != 0 && power > 0.0;
        if !burning {
            flush(&mut cur, &mut pieces);
            pos = next;
            continue;
        }
        if cur.is_empty() {
            cur.push(pos);
        }
        if mode == 1 {
            cur.push(next);
        } else {
            let c = if let Some(r) = radius {
                // centre from radius (positive = short arc)
                let d = [next[0] - pos[0], next[1] - pos[1]];
                let q = (d[0] * d[0] + d[1] * d[1]).sqrt();
                if q < 1e-12 {
                    pos
                } else {
                    let h = ((r * k).powi(2) - (q / 2.0).powi(2)).max(0.0).sqrt();
                    let s = if (mode == 2) == (r > 0.0) { -1.0 } else { 1.0 };
                    [(pos[0] + next[0]) / 2.0 - s * h * d[1] / q, (pos[1] + next[1]) / 2.0 + s * h * d[0] / q]
                }
            } else {
                [pos[0] + ci * k, pos[1] + cj * k]
            };
            let r = ((pos[0] - c[0]).powi(2) + (pos[1] - c[1]).powi(2)).sqrt();
            let a0 = (pos[1] - c[1]).atan2(pos[0] - c[0]);
            let mut a1 = (next[1] - c[1]).atan2(next[0] - c[0]);
            if mode == 3 {
                while a1 <= a0 + 1e-9 {
                    a1 += std::f64::consts::TAU;
                }
            } else {
                while a1 >= a0 - 1e-9 {
                    a1 -= std::f64::consts::TAU;
                }
            }
            let mut pts = arc_points(c, r, a0, a1);
            if let Some(l) = pts.last_mut() {
                *l = next;
            }
            cur.extend(pts);
        }
        pos = next;
    }
    flush(&mut cur, &mut pieces);
    if pieces.is_empty() {
        return Err(tr("Il G-code non contiene tagli con il laser acceso.").into());
    }
    Ok(pieces)
}

fn gcode_words(s: &str) -> Vec<(char, f64)> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_ascii_alphabetic() {
            let l = c[i];
            let mut j = i + 1;
            while j < c.len() && (c[j].is_ascii_digit() || c[j] == '.' || c[j] == '-' || c[j] == '+') {
                j += 1;
            }
            let num: String = c[i + 1..j].iter().collect();
            if let Ok(v) = num.parse::<f64>() {
                out.push((l, v));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxf_inch_line_and_colours() {
        let s = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n62\n1\n10\n0\n20\n0\n11\n1\n21\n0\n0\nCIRCLE\n8\n0\n62\n5\n10\n0\n20\n0\n40\n1\n0\nENDSEC\n0\nEOF\n";
        let p = dxf(s.as_bytes()).unwrap();
        assert_eq!(p.len(), 2);
        assert!((p[0].paths[0].pts[1][0] - 25.4).abs() < 1e-9);
        assert_eq!(p[0].layer, 0);
        assert_eq!(p[1].layer, 1);
        assert!(p[1].paths[0].closed);
    }

    #[test]
    fn hpgl_square_closes() {
        let p = hpgl(b"IN;SP1;PU0,0;PD1000,0,1000,1000,0,1000,0,0;PU;").unwrap();
        assert_eq!(p.len(), 1);
        let pl = &p[0].paths[0];
        assert!(pl.closed);
        assert!((pl.pts[1][0] - 25.4).abs() < 1e-9);
    }

    #[test]
    fn gcode_only_burning_moves() {
        let p = gcode(b"G21 G90\nG0 X0 Y0\nM4 S500\nG1 X10 Y0 F600\nG1 X10 Y10\nM5\nG0 X50 Y50\n").unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].paths[0].pts.len(), 3);
    }
}
