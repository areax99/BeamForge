//! PCB files: Gerber (RS-274X, GerberTranslator::Import 0x10009e4c8), Excellon drill files
//! (ExcellonTranslator::Import 0x100097664) and Gerber job files (.gbrjob, a list of the board's
//! files). Copper and mask images are merged into filled outlines (the original composites them
//! with Clipper); the board profile is kept as a centre line.

use super::{Piece, circle};
use crate::i18n::{tr, trf};
use crate::model::Polyline;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use std::collections::HashMap;

type P = [f64; 2];
type Contour = Vec<P>;

/// Inches → mm (the 25.4 in ExcellonTranslator::ReadCoord 0x10009a054 and Gerber %MOIN%).
pub const INCH: f64 = 25.4;

/// What a board file holds, from its extension (GerberTranslator::ClassifyFilename 0x1000a852c).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Function {
    Copper,
    Mask,
    Legend,
    Paste,
    Profile,
    Drill,
    Other,
}

/// BeamForge layer for each board function (estimated: the original picks colours per function
/// and side in GerberTranslator::LayerColorFor 0x10009e368).
pub fn layer_of(f: Function) -> usize {
    match f {
        Function::Copper => 1,
        Function::Mask => 2,
        Function::Legend => 3,
        Function::Paste => 4,
        Function::Profile => 5,
        Function::Drill => 6,
        Function::Other => 0,
    }
}

pub fn function_of_ext(ext: &str) -> Function {
    match ext {
        "gtl" | "gbl" | "g1" | "g2" | "g3" | "g4" | "g5" | "g6" | "g7" | "g8" | "g9" | "g1l" | "g2l" | "g3l" | "g4l" | "g5l" | "g6l"
        | "g7l" | "g8l" | "g9l" => Function::Copper,
        "gts" | "gbs" => Function::Mask,
        "gto" | "gbo" => Function::Legend,
        "gtp" | "gbp" => Function::Paste,
        "gko" | "gm1" => Function::Profile,
        "drl" | "xln" | "exc" | "xnc" => Function::Drill,
        _ => Function::Other,
    }
}

pub fn layer_for_name(path: &std::path::Path) -> Function {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let f = function_of_ext(&ext);
    if f != Function::Other {
        return f;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name.contains("edge") || name.contains("profile") || name.contains("outline") {
        Function::Profile
    } else if name.contains("mask") {
        Function::Mask
    } else if name.contains("silk") || name.contains("legend") {
        Function::Legend
    } else if name.contains("paste") {
        Function::Paste
    } else if name.contains("cu") || name.contains("copper") {
        Function::Copper
    } else {
        Function::Other
    }
}

fn function_of_attr(s: &str) -> Function {
    let s = s.to_lowercase();
    if s.starts_with("copper") {
        Function::Copper
    } else if s.starts_with("soldermask") {
        Function::Mask
    } else if s.starts_with("legend") {
        Function::Legend
    } else if s.starts_with("paste") {
        Function::Paste
    } else if s.starts_with("profile") {
        Function::Profile
    } else if s.contains("drill") || s.contains("plated") {
        Function::Drill
    } else {
        Function::Other
    }
}

pub fn looks_like_excellon(b: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&b[..b.len().min(4096)]);
    t.contains("M48") && !t.contains("%FS")
}

// ------------------------------------------------------------------ geometry helpers

fn area(c: &[P]) -> f64 {
    let mut a = 0.0;
    for i in 0..c.len() {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

fn ccw(mut c: Contour) -> Contour {
    if area(&c) < 0.0 {
        c.reverse();
    }
    c
}

fn hull(mut pts: Vec<P>) -> Contour {
    pts.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap().then(a[1].partial_cmp(&b[1]).unwrap()));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: P, a: P, b: P| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut lower: Vec<P> = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<P> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Union of all contours (non-zero), then optional subtraction; result as oriented contours.
fn union(acc: Vec<Contour>, add: Vec<Contour>) -> Vec<Contour> {
    if add.is_empty() {
        return acc;
    }
    let shapes = acc.overlay(&add, OverlayRule::Union, FillRule::NonZero);
    flatten(shapes)
}

fn subtract(acc: Vec<Contour>, clear: Vec<Contour>) -> Vec<Contour> {
    if clear.is_empty() || acc.is_empty() {
        return acc;
    }
    flatten(acc.overlay(&clear, OverlayRule::Difference, FillRule::NonZero))
}

fn flatten(shapes: Vec<Vec<Vec<P>>>) -> Vec<Contour> {
    let mut out = Vec::new();
    for s in shapes {
        for (i, c) in s.into_iter().enumerate() {
            if c.len() < 3 {
                continue;
            }
            let c = ccw(c);
            out.push(if i == 0 { c } else { c.into_iter().rev().collect() });
        }
    }
    out
}

// ------------------------------------------------------------------ Gerber

#[derive(Clone, Debug)]
enum Aperture {
    Circle(f64),
    Rect(f64, f64),
    Obround(f64, f64),
    Polygon(f64, usize, f64),
    /// Already-built outline (macro), relative to the flash point; dark and clear parts.
    Outline(Vec<Contour>, Vec<Contour>),
}

impl Aperture {
    /// Outline of the aperture at the origin (dark contours, clear contours).
    fn shape(&self) -> (Vec<Contour>, Vec<Contour>) {
        match self {
            Aperture::Circle(d) => (vec![circle([0.0, 0.0], d / 2.0).pts], vec![]),
            Aperture::Rect(w, h) => (vec![vec![[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [w / 2.0, h / 2.0], [-w / 2.0, h / 2.0]]], vec![]),
            Aperture::Obround(w, h) => {
                let r = w.min(*h) / 2.0;
                let (dx, dy) = if w > h { (w / 2.0 - r, 0.0) } else { (0.0, h / 2.0 - r) };
                let mut pts = circle([dx, dy], r).pts;
                pts.extend(circle([-dx, -dy], r).pts);
                (vec![hull(pts)], vec![])
            }
            Aperture::Polygon(d, n, rot) => {
                let n = (*n).clamp(3, 12);
                let pts = (0..n)
                    .map(|i| {
                        let a = rot.to_radians() + i as f64 / n as f64 * std::f64::consts::TAU;
                        [d / 2.0 * a.cos(), d / 2.0 * a.sin()]
                    })
                    .collect();
                (vec![pts], vec![])
            }
            Aperture::Outline(d, c) => (d.clone(), c.clone()),
        }
    }
}

/// Evaluate an aperture-macro expression: numbers, $n variables, + - x X / and parentheses.
fn eval(expr: &str, vars: &HashMap<usize, f64>) -> f64 {
    let toks: Vec<char> = expr.chars().filter(|c| !c.is_whitespace()).collect();
    fn parse_sum(t: &[char], i: &mut usize, v: &HashMap<usize, f64>) -> f64 {
        let mut a = parse_prod(t, i, v);
        while *i < t.len() && (t[*i] == '+' || t[*i] == '-') {
            let op = t[*i];
            *i += 1;
            let b = parse_prod(t, i, v);
            a = if op == '+' { a + b } else { a - b };
        }
        a
    }
    fn parse_prod(t: &[char], i: &mut usize, v: &HashMap<usize, f64>) -> f64 {
        let mut a = parse_atom(t, i, v);
        while *i < t.len() && (t[*i] == 'x' || t[*i] == 'X' || t[*i] == '/') {
            let op = t[*i];
            *i += 1;
            let b = parse_atom(t, i, v);
            a = if op == '/' { if b != 0.0 { a / b } else { 0.0 } } else { a * b };
        }
        a
    }
    fn parse_atom(t: &[char], i: &mut usize, v: &HashMap<usize, f64>) -> f64 {
        if *i >= t.len() {
            return 0.0;
        }
        match t[*i] {
            '(' => {
                *i += 1;
                let r = parse_sum(t, i, v);
                if *i < t.len() && t[*i] == ')' {
                    *i += 1;
                }
                r
            }
            '-' => {
                *i += 1;
                -parse_atom(t, i, v)
            }
            '+' => {
                *i += 1;
                parse_atom(t, i, v)
            }
            '$' => {
                *i += 1;
                let s = *i;
                while *i < t.len() && t[*i].is_ascii_digit() {
                    *i += 1;
                }
                let n: usize = t[s..*i].iter().collect::<String>().parse().unwrap_or(0);
                *v.get(&n).unwrap_or(&0.0)
            }
            _ => {
                let s = *i;
                while *i < t.len() && (t[*i].is_ascii_digit() || t[*i] == '.') {
                    *i += 1;
                }
                if s == *i {
                    *i += 1;
                    return 0.0;
                }
                t[s..*i].iter().collect::<String>().parse().unwrap_or(0.0)
            }
        }
    }
    let mut i = 0;
    parse_sum(&toks, &mut i, vars)
}

fn rot(p: P, deg: f64) -> P {
    let (s, c) = deg.to_radians().sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

/// Build a macro aperture from its primitives (circle 1, vector line 20/2, centre line 21,
/// outline 4, polygon 5; thermal 7 approximated as its outer circle).
fn build_macro(body: &[String], params: &[f64], scale: f64) -> Aperture {
    let mut vars: HashMap<usize, f64> = params.iter().enumerate().map(|(i, v)| (i + 1, *v)).collect();
    let (mut dark, mut clear) = (Vec::new(), Vec::new());
    for stmt in body {
        let s = stmt.trim();
        if s.is_empty() || s.starts_with('0') && !s.starts_with("0.") && s.len() > 0 && s.chars().nth(1).map(|c| c == ' ').unwrap_or(true) {
            continue; // comment
        }
        if let Some(rest) = s.strip_prefix('$') {
            if let Some((n, e)) = rest.split_once('=') {
                if let Ok(n) = n.parse::<usize>() {
                    let v = eval(e, &vars);
                    vars.insert(n, v);
                }
            }
            continue;
        }
        let f: Vec<f64> = s.split(',').map(|x| eval(x, &vars)).collect();
        if f.is_empty() {
            continue;
        }
        let code = f[0] as i32;
        let g = |k: usize| f.get(k).copied().unwrap_or(0.0);
        let (exposure, contour): (f64, Option<Contour>) = match code {
            1 => {
                let c = circle([g(3) * scale, g(4) * scale], g(2) * scale / 2.0).pts;
                (g(1), Some(c.into_iter().map(|p| rot(p, g(5))).collect()))
            }
            20 | 2 => {
                let (w, a, b) = (g(2) * scale, [g(3) * scale, g(4) * scale], [g(5) * scale, g(6) * scale]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
                let n = [-d[1] / l * w / 2.0, d[0] / l * w / 2.0];
                let c = vec![[a[0] + n[0], a[1] + n[1]], [b[0] + n[0], b[1] + n[1]], [b[0] - n[0], b[1] - n[1]], [a[0] - n[0], a[1] - n[1]]];
                (g(1), Some(c.into_iter().map(|p| rot(p, g(7))).collect()))
            }
            21 => {
                let (w, h, cx, cy) = (g(2) * scale, g(3) * scale, g(4) * scale, g(5) * scale);
                let c = vec![[cx - w / 2.0, cy - h / 2.0], [cx + w / 2.0, cy - h / 2.0], [cx + w / 2.0, cy + h / 2.0], [cx - w / 2.0, cy + h / 2.0]];
                (g(1), Some(c.into_iter().map(|p| rot(p, g(6))).collect()))
            }
            4 => {
                let n = g(2) as usize;
                let mut c: Contour = (0..=n).map(|i| [g(3 + 2 * i) * scale, g(4 + 2 * i) * scale]).collect();
                c.pop();
                let r = g(5 + 2 * n);
                (g(1), Some(c.into_iter().map(|p| rot(p, r)).collect()))
            }
            5 => {
                let n = (g(2) as usize).clamp(3, 12);
                let (cx, cy, d) = (g(3) * scale, g(4) * scale, g(5) * scale);
                let c = (0..n)
                    .map(|i| {
                        let a = i as f64 / n as f64 * std::f64::consts::TAU;
                        rot([cx + d / 2.0 * a.cos(), cy + d / 2.0 * a.sin()], g(6))
                    })
                    .collect();
                (g(1), Some(c))
            }
            7 => (1.0, Some(circle([g(1) * scale, g(2) * scale], g(3) * scale / 2.0).pts)),
            _ => (1.0, None),
        };
        if let Some(c) = contour {
            if c.len() >= 3 {
                if exposure >= 0.5 { dark.push(ccw(c)) } else { clear.push(ccw(c)) }
            }
        }
    }
    Aperture::Outline(dark, clear)
}

struct Gerber {
    unit: f64,
    int_d: u32,
    dec_d: u32,
    trailing: bool,
    incremental: bool,
    apertures: HashMap<i32, Aperture>,
    macros: HashMap<String, Vec<String>>,
    cur_ap: i32,
    pos: P,
    interp: i32,
    multi_quadrant: bool,
    region: bool,
    region_pts: Contour,
    region_contours: Vec<Contour>,
    dark: bool,
    batch_dark: Vec<Contour>,
    batch_clear: Vec<Contour>,
    acc: Vec<Contour>,
    lines: Vec<Polyline>,
    centerline: bool,
}

impl Gerber {
    fn coord(&self, s: &str) -> f64 {
        if s.contains('.') {
            return s.parse::<f64>().unwrap_or(0.0) * self.unit;
        }
        let neg = s.starts_with('-');
        let digits: String = s.trim_start_matches(['+', '-']).to_string();
        let total = (self.int_d + self.dec_d) as usize;
        let padded = if self.trailing {
            // trailing zeros omitted: pad on the right
            format!("{:0<width$}", digits, width = total.max(digits.len()))
        } else {
            format!("{:0>width$}", digits, width = total.max(digits.len()))
        };
        let v = padded.parse::<f64>().unwrap_or(0.0) / 10f64.powi(self.dec_d as i32);
        (if neg { -v } else { v }) * self.unit
    }

    fn flush_polarity(&mut self) {
        let d = std::mem::take(&mut self.batch_dark);
        let c = std::mem::take(&mut self.batch_clear);
        let acc = std::mem::take(&mut self.acc);
        self.acc = subtract(union(acc, d), c);
    }

    fn add(&mut self, c: Contour) {
        if c.len() < 3 {
            return;
        }
        let c = ccw(c);
        if self.dark { self.batch_dark.push(c) } else { self.batch_clear.push(c) }
    }

    fn set_dark(&mut self, dark: bool) {
        if dark != self.dark {
            if dark {
                // a clear batch ends: apply it
                self.flush_polarity();
            }
            self.dark = dark;
        }
    }

    fn flash(&mut self, at: P) {
        let Some(ap) = self.apertures.get(&self.cur_ap).cloned() else { return };
        let (d, c) = ap.shape();
        let mv = |c: Contour| c.into_iter().map(|p| [p[0] + at[0], p[1] + at[1]]).collect::<Contour>();
        if self.centerline {
            return;
        }
        for k in d {
            self.add(mv(k));
        }
        if !c.is_empty() {
            let was = self.dark;
            self.set_dark(false);
            for k in c {
                self.add(mv(k));
            }
            self.set_dark(was);
        }
    }

    fn stroke(&mut self, pts: &[P]) {
        if self.centerline {
            self.lines.push(Polyline { pts: pts.to_vec(), closed: false });
            return;
        }
        let Some(ap) = self.apertures.get(&self.cur_ap).cloned() else { return };
        let (d, _) = ap.shape();
        let Some(shape) = d.into_iter().next() else { return };
        for w in pts.windows(2) {
            let mut h = Vec::with_capacity(shape.len() * 2);
            for p in &shape {
                h.push([p[0] + w[0][0], p[1] + w[0][1]]);
                h.push([p[0] + w[1][0], p[1] + w[1][1]]);
            }
            self.add(hull(h));
        }
    }

    fn arc(&self, from: P, to: P, ij: P) -> Vec<P> {
        let cw = self.interp == 2;
        let centers: Vec<P> = if self.multi_quadrant {
            vec![[from[0] + ij[0], from[1] + ij[1]]]
        } else {
            // single quadrant: unsigned offsets, pick the centre giving a ≤ 90° arc
            [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]].iter().map(|s| [from[0] + s[0] * ij[0].abs(), from[1] + s[1] * ij[1].abs()]).collect()
        };
        let mut best: Option<(f64, Vec<P>)> = None;
        for c in centers {
            let r = ((from[0] - c[0]).powi(2) + (from[1] - c[1]).powi(2)).sqrt();
            let r2 = ((to[0] - c[0]).powi(2) + (to[1] - c[1]).powi(2)).sqrt();
            let a0 = (from[1] - c[1]).atan2(from[0] - c[0]);
            let mut a1 = (to[1] - c[1]).atan2(to[0] - c[0]);
            if cw {
                while a1 >= a0 - 1e-9 {
                    a1 -= std::f64::consts::TAU;
                }
                if self.multi_quadrant && (from == to) {
                    a1 = a0 - std::f64::consts::TAU;
                }
            } else {
                while a1 <= a0 + 1e-9 {
                    a1 += std::f64::consts::TAU;
                }
                if self.multi_quadrant && (from == to) {
                    a1 = a0 + std::f64::consts::TAU;
                }
            }
            let sweep = (a1 - a0).abs();
            let err = (r - r2).abs() + if !self.multi_quadrant && sweep > std::f64::consts::FRAC_PI_2 + 1e-3 { 1e6 } else { 0.0 };
            let mut pts = super::arc_points(c, r, a0, a1);
            if let Some(l) = pts.last_mut() {
                *l = to;
            }
            if best.as_ref().map(|b| err < b.0).unwrap_or(true) {
                best = Some((err, pts));
            }
        }
        best.map(|b| b.1).unwrap_or_else(|| vec![to])
    }

    fn word(&mut self, w: &str) {
        let w = w.trim();
        if w.is_empty() {
            return;
        }
        // letters with values
        let mut fields: Vec<(char, String)> = Vec::new();
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                fields.push((ch.to_ascii_uppercase(), String::new()));
            } else if let Some(l) = fields.last_mut() {
                l.1.push(ch);
            }
        }
        let (mut x, mut y, mut i, mut j) = (None, None, 0.0, 0.0);
        let mut d: Option<i32> = None;
        for (l, v) in &fields {
            match l {
                'G' => match v.parse::<i32>().unwrap_or(-1) {
                    1 => self.interp = 1,
                    2 => self.interp = 2,
                    3 => self.interp = 3,
                    36 => {
                        self.region = true;
                        self.region_pts.clear();
                    }
                    37 => {
                        self.close_region_contour();
                        let cs = std::mem::take(&mut self.region_contours);
                        for c in cs {
                            self.add(c);
                        }
                        self.region = false;
                    }
                    74 => self.multi_quadrant = false,
                    75 => self.multi_quadrant = true,
                    70 => self.unit = INCH,
                    71 => self.unit = 1.0,
                    90 => self.incremental = false,
                    91 => self.incremental = true,
                    _ => {}
                },
                'X' => x = Some(self.coord(v)),
                'Y' => y = Some(self.coord(v)),
                'I' => i = self.coord(v),
                'J' => j = self.coord(v),
                'D' => d = v.parse().ok(),
                'M' => {}
                _ => {}
            }
        }
        if let Some(n) = d {
            if n >= 10 {
                self.cur_ap = n;
                return;
            }
        }
        let target = if self.incremental { [self.pos[0] + x.unwrap_or(0.0), self.pos[1] + y.unwrap_or(0.0)] } else { [x.unwrap_or(self.pos[0]), y.unwrap_or(self.pos[1])] };
        let op = d.unwrap_or(if x.is_some() || y.is_some() { 1 } else { 0 });
        match op {
            1 => {
                let pts = if self.interp == 1 { vec![self.pos, target] } else {
                    let mut v = vec![self.pos];
                    v.extend(self.arc(self.pos, target, [i, j]));
                    v
                };
                if self.region {
                    if self.region_pts.is_empty() {
                        self.region_pts.push(self.pos);
                    }
                    self.region_pts.extend(pts.into_iter().skip(1));
                } else {
                    self.stroke(&pts);
                }
                self.pos = target;
            }
            2 => {
                if self.region {
                    self.close_region_contour();
                }
                self.pos = target;
            }
            3 => {
                self.pos = target;
                self.flash(target);
            }
            _ => {}
        }
    }

    fn close_region_contour(&mut self) {
        let mut c = std::mem::take(&mut self.region_pts);
        if c.len() > 1 && c[0] == *c.last().unwrap() {
            c.pop();
        }
        if c.len() >= 3 {
            self.region_contours.push(c);
        }
    }

    fn extended(&mut self, block: &str) {
        let b = block.trim().trim_end_matches('*');
        if let Some(fs) = b.strip_prefix("FS") {
            self.trailing = fs.starts_with('T');
            self.incremental = fs.get(1..2) == Some("I");
            if let Some(xp) = fs.find('X') {
                let digs: Vec<u32> = fs[xp + 1..].chars().take(2).filter_map(|c| c.to_digit(10)).collect();
                if digs.len() == 2 {
                    self.int_d = digs[0];
                    self.dec_d = digs[1];
                }
            }
        } else if let Some(mo) = b.strip_prefix("MO") {
            self.unit = if mo.starts_with("IN") { INCH } else { 1.0 };
        } else if let Some(ad) = b.strip_prefix("ADD") {
            let digits: String = ad.chars().take_while(|c| c.is_ascii_digit()).collect();
            let Ok(n) = digits.parse::<i32>() else { return };
            let rest = &ad[digits.len()..];
            let (name, params) = rest.split_once(',').unwrap_or((rest, ""));
            let p: Vec<f64> = params.split(['X', 'x']).filter_map(|v| v.trim().parse().ok()).collect();
            let g = |k: usize| p.get(k).copied().unwrap_or(0.0) * self.unit;
            let ap = match name {
                "C" => Aperture::Circle(g(0)),
                "R" => Aperture::Rect(g(0), g(1)),
                "O" => Aperture::Obround(g(0), g(1)),
                "P" => Aperture::Polygon(g(0), p.get(1).copied().unwrap_or(3.0) as usize, p.get(2).copied().unwrap_or(0.0)),
                m => match self.macros.get(m) {
                    Some(body) => build_macro(body, &p, self.unit),
                    None => Aperture::Circle(0.0),
                },
            };
            self.apertures.insert(n, ap);
        } else if let Some(lp) = b.strip_prefix("LP") {
            self.set_dark(!lp.starts_with('C'));
        } else if let Some(ff) = b.strip_prefix("TF.FileFunction,") {
            if function_of_attr(ff) == Function::Profile {
                self.centerline = true;
            }
        }
    }
}

pub fn gerber(bytes: &[u8], f: Function) -> Result<Vec<Piece>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut g = Gerber {
        unit: 1.0,
        int_d: 2,
        dec_d: 4,
        trailing: false,
        incremental: false,
        apertures: HashMap::new(),
        macros: HashMap::new(),
        cur_ap: 10,
        pos: [0.0, 0.0],
        interp: 1,
        multi_quadrant: false,
        region: false,
        region_pts: Vec::new(),
        region_contours: Vec::new(),
        dark: true,
        batch_dark: Vec::new(),
        batch_clear: Vec::new(),
        acc: Vec::new(),
        lines: Vec::new(),
        centerline: f == Function::Profile,
    };
    // split into %extended% blocks and plain words
    let mut rest: &str = &text;
    let mut saw_any = false;
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('%') {
            let end = stripped.find('%').unwrap_or(stripped.len());
            let block = &stripped[..end];
            rest = stripped.get(end + 1..).unwrap_or("");
            if let Some(am) = block.trim().strip_prefix("AM") {
                let mut parts = am.split('*').map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                if let Some(name) = parts.next() {
                    g.macros.insert(name, parts.collect());
                }
            } else {
                for cmd in block.split('*') {
                    g.extended(cmd);
                }
            }
            saw_any = true;
        } else {
            let end = rest.find(['%', '*']).unwrap_or(rest.len());
            let word: String = rest[..end].chars().filter(|c| !c.is_whitespace()).collect();
            if rest[end..].starts_with('*') {
                rest = &rest[end + 1..];
            } else {
                rest = &rest[end..];
            }
            if word.starts_with("M02") || word.starts_with("M00") {
                break;
            }
            if !word.starts_with("G04") {
                g.word(&word);
            }
            saw_any = true;
        }
    }
    if !saw_any {
        return Err(tr("Il file Gerber è vuoto.").into());
    }
    g.set_dark(true);
    g.flush_polarity();
    let mut paths: Vec<Polyline> = g.acc.into_iter().map(|c| Polyline { pts: c, closed: true }).collect();
    paths.extend(g.lines);
    if paths.is_empty() {
        return Err(tr("Il file Gerber non contiene forme.").into());
    }
    let f = if g.centerline { Function::Profile } else { f };
    Ok(vec![Piece { layer: layer_of(f), paths }])
}

// ------------------------------------------------------------------ Excellon

/// Excellon drill files: tool table (T<n>C<diameter>), METRIC/INCH with LZ/TZ, hits as circles
/// of the tool's diameter, G85 slots and M15/M16 routes as outlines of the tool path.
pub fn excellon(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut metric = false;
    let mut keep_leading = false; // "LZ": leading zeros kept, so trailing ones are padded
    let mut digits: Option<(u32, u32)> = None;
    let mut tools: HashMap<i32, f64> = HashMap::new();
    let mut tool = 0;
    let mut pos: P = [0.0, 0.0];
    let mut incremental = false;
    let mut routing = false;
    let mut route_mode = 0;
    let mut route: Vec<P> = Vec::new();
    let mut paths: Vec<Polyline> = Vec::new();
    let mut slots: Vec<(P, P, f64)> = Vec::new();
    let mut in_header = false;
    let coord = |s: &str, metric: bool, keep_leading: bool, digits: Option<(u32, u32)>| -> f64 {
        // ExcellonTranslator::ReadCoord 0x10009a054
        let unit = if metric { 1.0 } else { INCH };
        if s.contains('.') {
            return s.parse::<f64>().unwrap_or(0.0) * unit;
        }
        let (int_d, dec_d) = digits.unwrap_or(if metric { (3, 3) } else { (2, 4) });
        let neg = s.starts_with('-');
        let d = s.trim_start_matches(['+', '-']);
        let total = (int_d + dec_d) as usize;
        let padded = if keep_leading { format!("{:0<w$}", d, w = total.max(d.len())) } else { format!("{:0>w$}", d, w = total.max(d.len())) };
        let v = padded.parse::<f64>().unwrap_or(0.0) / 10f64.powi(dec_d as i32);
        (if neg { -v } else { v }) * unit
    };
    let end_route = |route: &mut Vec<P>, paths: &mut Vec<Polyline>, slots: &mut Vec<(P, P, f64)>, d: f64| {
        for w in route.windows(2) {
            slots.push((w[0], w[1], d));
        }
        route.clear();
        let _ = paths;
    };
    for raw in text.lines() {
        let l = raw.trim().to_uppercase();
        if l.is_empty() || l.starts_with(';') {
            continue;
        }
        if l.starts_with("M48") {
            in_header = true;
            continue;
        }
        if l == "%" || l.starts_with("M95") {
            in_header = false;
            continue;
        }
        if l.starts_with("METRIC") || l.starts_with("M71") {
            metric = true;
        }
        if l.starts_with("INCH") || l.starts_with("M72") {
            metric = false;
        }
        if l.starts_with("METRIC") || l.starts_with("INCH") {
            if l.contains(",LZ") {
                keep_leading = true;
            } else if l.contains(",TZ") {
                keep_leading = false;
            }
            if let Some(fmt) = l.split(',').find(|p| p.contains('.') && p.chars().all(|c| c == '0' || c == '.')) {
                let (a, b) = fmt.split_once('.').unwrap();
                digits = Some((a.len() as u32, b.len() as u32));
            }
            continue;
        }
        if l.starts_with("G90") {
            incremental = false;
        }
        if l.starts_with("G91") {
            incremental = true;
        }
        if l.starts_with("M15") {
            routing = true;
            route = vec![pos];
            continue;
        }
        if l.starts_with("M16") || l.starts_with("M17") {
            routing = false;
            let d = tools.get(&tool).copied().unwrap_or(0.0);
            end_route(&mut route, &mut paths, &mut slots, d);
            continue;
        }
        if l.starts_with('T') {
            let num: String = l[1..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num.parse::<i32>() {
                if let Some(ci) = l.find('C') {
                    // ParseToolDef 0x100099c50: T<n> … C<diameter>, kept only when > 0
                    let dia: String = l[ci + 1..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                    if let Ok(dv) = dia.parse::<f64>() {
                        if dv > 0.0 {
                            tools.insert(n, dv * if metric { 1.0 } else { INCH });
                        }
                    }
                }
                if !in_header {
                    tool = n;
                }
            }
            continue;
        }
        if in_header {
            continue;
        }
        if l.starts_with("G00") {
            route_mode = 0;
        } else if l.starts_with("G01") {
            route_mode = 1;
        }
        // coordinates (possibly two pairs around G85)
        let parts: Vec<&str> = l.split("G85").collect();
        let mut pts_here: Vec<P> = Vec::new();
        for part in &parts {
            let mut x = None;
            let mut y = None;
            let mut cur_l = ' ';
            let mut buf = String::new();
            let done = |c: char, b: &str, x: &mut Option<f64>, y: &mut Option<f64>| match c {
                'X' => *x = Some(coord(b, metric, keep_leading, digits)),
                'Y' => *y = Some(coord(b, metric, keep_leading, digits)),
                _ => {}
            };
            for ch in part.chars() {
                if ch.is_ascii_alphabetic() {
                    done(cur_l, &buf, &mut x, &mut y);
                    cur_l = ch;
                    buf.clear();
                } else {
                    buf.push(ch);
                }
            }
            done(cur_l, &buf, &mut x, &mut y);
            if x.is_some() || y.is_some() {
                let p = if incremental { [pos[0] + x.unwrap_or(0.0), pos[1] + y.unwrap_or(0.0)] } else { [x.unwrap_or(pos[0]), y.unwrap_or(pos[1])] };
                pos = p;
                pts_here.push(p);
            }
        }
        let d = tools.get(&tool).copied().unwrap_or(0.0);
        if parts.len() == 2 && pts_here.len() == 2 {
            slots.push((pts_here[0], pts_here[1], d));
        } else if let Some(p) = pts_here.last() {
            if routing {
                if route_mode == 1 {
                    route.push(*p);
                } else {
                    end_route(&mut route, &mut paths, &mut slots, d);
                    route.push(*p);
                }
            } else if !(l.starts_with("G00") || l.starts_with("G01")) {
                if d > 0.0 {
                    paths.push(circle(*p, d / 2.0));
                }
            }
        }
    }
    for (a, b, d) in slots {
        if d <= 0.0 {
            continue;
        }
        let mut pts = circle(a, d / 2.0).pts;
        pts.extend(circle(b, d / 2.0).pts);
        paths.push(Polyline { pts: hull(pts), closed: true });
    }
    if paths.is_empty() {
        return Err(tr("Il file di foratura non contiene fori.").into());
    }
    Ok(vec![Piece { layer: layer_of(Function::Drill), paths }])
}

// ------------------------------------------------------------------ Gerber job

/// A .gbrjob lists the board's files (FilesAttributes: Path + FileFunction); each one found
/// next to it is imported on the layer for its function.
pub fn gerber_job(path: &std::path::Path, bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| trf("Il file .gbrjob non è leggibile: {}", &[&e]))?;
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    let mut pieces = Vec::new();
    let mut missing = Vec::new();
    for f in v.get("FilesAttributes").and_then(|a| a.as_array()).cloned().unwrap_or_default() {
        let Some(name) = f.get("Path").and_then(|p| p.as_str()) else { continue };
        let func = f.get("FileFunction").and_then(|p| p.as_str()).map(function_of_attr).unwrap_or(Function::Other);
        let p = dir.join(name);
        let Ok(b) = std::fs::read(&p) else {
            missing.push(name.to_string());
            continue;
        };
        let r = if func == Function::Drill || looks_like_excellon(&b) { excellon(&b) } else { gerber(&b, func) };
        if let Ok(mut ps) = r {
            pieces.append(&mut ps);
        }
    }
    if pieces.is_empty() {
        return Err(if missing.is_empty() {
            tr("Il file .gbrjob non elenca file della scheda.").into()
        } else {
            trf("I file elencati nel .gbrjob non sono nella stessa cartella: {}", &[&missing.join(", ")])
        });
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gerber_two_pads_and_track_merge() {
        let g = "%FSLAX24Y24*%\n%MOMM*%\n%ADD10C,1.0*%\n%ADD11R,2X2*%\nD11*\nX0Y0D03*\nX100000Y0D03*\nD10*\nX0Y0D02*\nX100000Y0D01*\nM02*\n";
        let p = gerber(g.as_bytes(), Function::Copper).unwrap();
        assert_eq!(p[0].layer, 1);
        // pads and track merge into one outline
        assert_eq!(p[0].paths.len(), 1);
        let xs: Vec<f64> = p[0].paths[0].pts.iter().map(|q| q[0]).collect();
        let max = xs.iter().cloned().fold(f64::MIN, f64::max);
        assert!((max - 11.0).abs() < 1e-6, "{max}");
    }

    #[test]
    fn gerber_region_and_inch() {
        let g = "%FSLAX25Y25*%%MOIN*%G36*X0Y0D02*X100000Y0D01*X100000Y100000D01*X0Y0D01*G37*M02*";
        let p = gerber(g.as_bytes(), Function::Copper).unwrap();
        let xs: Vec<f64> = p[0].paths[0].pts.iter().map(|q| q[0]).collect();
        assert!((xs.iter().cloned().fold(f64::MIN, f64::max) - 25.4).abs() < 1e-6);
    }

    #[test]
    fn excellon_hits() {
        let e = "M48\nMETRIC,TZ\nT1C0.800\n%\nT1\nX10.0Y5.0\nX20000Y5000\nM30\n";
        let p = excellon(e.as_bytes()).unwrap();
        assert_eq!(p[0].paths.len(), 2);
        let c: f64 = p[0].paths[1].pts.iter().map(|q| q[0]).sum::<f64>() / p[0].paths[1].pts.len() as f64;
        assert!((c - 20.0).abs() < 1e-6, "{c}");
    }

    #[test]
    fn macro_expression() {
        let v: HashMap<usize, f64> = [(1, 2.0), (2, 3.0)].into_iter().collect();
        assert!((eval("$1x$2+1", &v) - 7.0).abs() < 1e-9);
        assert!((eval("($1+$2)/5", &v) - 1.0).abs() < 1e-9);
    }
}
