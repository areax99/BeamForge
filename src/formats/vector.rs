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

/// The first bytes of a binary DXF ("AutoCAD Binary DXF\r\n" + SUB + NUL).
const BINARY_DXF_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

/// A binary DXF as the same (group code, value) pairs the text form gives, so one reader serves both.
/// Group codes are 2 bytes little-endian (R13 and later; R12's 1-byte codes with 255 = "2 more bytes
/// follow" are read too); the value's type comes from the code's range, as in AutoCAD's DXF reference.
fn read_binary_pairs(bytes: &[u8]) -> Option<Vec<(i32, String)>> {
    let mut i = BINARY_DXF_SENTINEL.len();
    // R12 writes 1-byte codes: the first one is 0 followed directly by "SECTION"
    let one_byte = bytes.get(i + 1).is_some_and(|b| *b == b'S');
    let mut out = Vec::new();
    let take = |i: &mut usize, n: usize| -> Option<&[u8]> {
        let s = bytes.get(*i..*i + n)?;
        *i += n;
        Some(s)
    };
    while i < bytes.len() {
        let code = if one_byte {
            let c = *take(&mut i, 1)?.first()? as i32;
            if c == 255 { i16::from_le_bytes(take(&mut i, 2)?.try_into().ok()?) as i32 } else { c }
        } else {
            i16::from_le_bytes(take(&mut i, 2)?.try_into().ok()?) as i32
        };
        let string = |i: &mut usize| -> Option<String> {
            let end = bytes.get(*i..)?.iter().position(|b| *b == 0)?;
            let s = String::from_utf8_lossy(&bytes[*i..*i + end]).into_owned();
            *i += end + 1;
            Some(s)
        };
        let f64v = |i: &mut usize| -> Option<String> { Some(f64::from_le_bytes(take(i, 8)?.try_into().ok()?).to_string()) };
        let i16v = |i: &mut usize| -> Option<String> { Some(i16::from_le_bytes(take(i, 2)?.try_into().ok()?).to_string()) };
        let i32v = |i: &mut usize| -> Option<String> { Some(i32::from_le_bytes(take(i, 4)?.try_into().ok()?).to_string()) };
        let i64v = |i: &mut usize| -> Option<String> { Some(i64::from_le_bytes(take(i, 8)?.try_into().ok()?).to_string()) };
        let chunk = |i: &mut usize| -> Option<String> {
            let n = *take(i, 1)?.first()? as usize;
            take(i, n)?;
            Some(String::new())
        };
        let value = match code {
            0..=9 | 100..=109 | 300..=309 | 320..=369 | 390..=399 | 410..=419 | 430..=439 | 470..=481 | 999 | 1000..=1003 | 1005..=1009 => string(&mut i)?,
            310..=319 | 1004 => chunk(&mut i)?,
            10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => f64v(&mut i)?,
            60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => i16v(&mut i)?,
            90..=99 | 420..=429 | 440..=459 | 1071 => i32v(&mut i)?,
            160..=169 => i64v(&mut i)?,
            290..=299 => take(&mut i, 1)?.first()?.to_string(),
            _ => return if out.is_empty() { None } else { Some(out) },
        };
        let end = code == 0 && value == "EOF";
        out.push((code, value));
        if end {
            break;
        }
    }
    Some(out)
}

pub fn dxf(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let pairs = if bytes.starts_with(BINARY_DXF_SENTINEL) {
        read_binary_pairs(bytes).ok_or_else(|| tr("Il file DXF non è valido (forse è un DXF binario o non è un DXF)."))?
    } else {
        let text = String::from_utf8_lossy(bytes);
        if !text.contains("SECTION") {
            return Err(tr("Il file DXF non è valido (forse è un DXF binario o non è un DXF).").into());
        }
        read_pairs(&text)
    };
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
    let mut insunits: Option<i32> = None;
    if let Some(h) = sections.get("HEADER") {
        for w in h.windows(2) {
            if w[0].0 == 9 && w[0].1.trim() == "$INSUNITS" && w[1].0 == 70 {
                insunits = w[1].1.trim().parse().ok();
            }
        }
    }
    // Impostazioni > Importa/Esporta: "Rileva automaticamente le unità" reads $INSUNITS; a file
    // without one, or with detection off, uses the unit picked there (Millimetri by default)
    let (auto, fallback) = crate::prefs::with(|p| (p.dxf_auto_units, p.dxf_unit.mm()));
    let unit = match insunits {
        Some(u) if auto && u != 0 => dxf_unit(u),
        _ => fallback,
    };
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
                // text is drawn with the default font unless "Ignora oggetti di testo" is on
                "TEXT" | "MTEXT" if !crate::prefs::with(|p| p.dxf_ignore_text) => {
                    let raw = e.s(1).unwrap_or("");
                    let text = if e.kind == "MTEXT" { mtext_plain(raw) } else { raw.to_string() };
                    let h = e.fd(40, 2.5).abs().max(0.01);
                    let rot = e.fd(50, 0.0).to_radians();
                    let at = [e.fd(10, 0.0), e.fd(20, 0.0)];
                    paths.extend(dxf_text(&text, h, rot, at).into_iter().map(|p| Polyline { pts: p.pts.into_iter().map(t).collect(), closed: p.closed }));
                }
                _ => {}
            }
            // "Chiudi automaticamente tolleranza (mm)": an outline whose ends meet this closely is closed
            let close_tol = crate::prefs::with(|p| p.dxf_close_tolerance);
            for p in paths.iter_mut() {
                if !p.closed && p.pts.len() > 2 {
                    let (a, b) = (p.pts[0], p.pts[p.pts.len() - 1]);
                    if ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() <= close_tol {
                        p.pts.pop();
                        p.closed = true;
                    }
                }
            }
            if !paths.is_empty() {
                let layer = self.order.layer(aci);
                self.pieces.push(Piece { layer, paths });
            }
            k += 1;
        }
    }
}

/// MTEXT without its formatting: \P breaks the line, \~ is a space, other \codes and braces go.
fn mtext_plain(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' | '}' => {}
            '\\' => match it.next() {
                Some('P') => out.push('\n'),
                Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                // codes with an argument run to the next ';'
                Some('f' | 'F' | 'H' | 'W' | 'Q' | 'T' | 'A' | 'C' | 'c' | 'p') => {
                    for d in it.by_ref() {
                        if d == ';' {
                            break;
                        }
                    }
                }
                _ => {}
            },
            _ => out.push(c),
        }
    }
    out
}

/// A DXF text's outlines in drawing units: the default font at height `h`, its lower-left corner
/// at `at`, turned by `rot` radians.
fn dxf_text(text: &str, h: f64, rot: f64, at: P) -> Vec<Polyline> {
    let font = crate::prefs::with(|p| p.default_font.clone());
    let font = if font.is_empty() { crate::text::fonts()[0].name.clone() } else { font };
    let paths = crate::text::text_outline(text, h, &font);
    let (mut mx, mut my) = (f64::INFINITY, f64::INFINITY);
    for q in paths.iter().flat_map(|p| p.pts.iter()) {
        mx = mx.min(q[0]);
        my = my.min(q[1]);
    }
    let (c, s) = (rot.cos(), rot.sin());
    paths
        .into_iter()
        .map(|p| Polyline {
            pts: p.pts.iter().map(|q| {
                let (x, y) = (q[0] - mx, q[1] - my);
                [at[0] + c * x - s * y, at[1] + s * x + c * y]
            }).collect(),
            closed: p.closed,
        })
        .collect()
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

/// Plotter units for PU/PD/PA/PR (HPGLTranslator::Import 0x1000ac1f4, constant 0x3cd013aa): the
/// 1"/1000 choice. The units now follow Impostazioni > Importa/Esporta (1"/1016 by default).
#[allow(dead_code)]
pub const HPGL_UNIT_MM: f64 = 0.0254;
/// AA arc centres are read as 1016 per inch (÷1016 × 25.4, constants 0x447e0000 / 0x41cb3333).
#[allow(dead_code)]
pub const HPGL_ARC_UNIT_MM: f64 = 25.4 / 1016.0;
/// A finished outline closes when its ends are this close (PathShape::Close(0.001)).
pub const HPGL_CLOSE_TOL: f64 = 0.001;

pub fn hpgl(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    // "Precisione importazione da formato HPGL/PLT": plotter units of 1/1016" (default) or 1/1000"
    let unit = crate::prefs::with(|p| 25.4 / p.hpgl_units.max(1) as f64);
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
                let p = [xy[0] * unit, xy[1] * unit];
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
                pieces.push(Piece { layer, paths: vec![circle(pos, r * unit)] });
            }
        } else if op == "AA" || op == "AR" {
            if nums.len() >= 3 {
                let c = if op == "AA" { [nums[0] * unit, nums[1] * unit] } else { [pos[0] + nums[0] * unit, pos[1] + nums[1] * unit] };
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
        // 1000 plotter units at the default 1"/1016 (Impostazioni > Importa/Esporta)
        assert!((pl.pts[1][0] - 1000.0 * 25.4 / 1016.0).abs() < 1e-9);
    }

    #[test]
    fn gcode_only_burning_moves() {
        let p = gcode(b"G21 G90\nG0 X0 Y0\nM4 S500\nG1 X10 Y0 F600\nG1 X10 Y10\nM5\nG0 X50 Y50\n").unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].paths[0].pts.len(), 3);
    }
}

#[cfg(test)]
mod dxf_samples {
    /// Reads every file listed (one path per line) in RAYETCH_DXF_LIST and reports what each gives.
    /// `RAYETCH_DXF_LIST=list.txt cargo test dxf_samples -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dxf_samples() {
        let Ok(list) = std::env::var("RAYETCH_DXF_LIST") else { return };
        for path in std::fs::read_to_string(list).unwrap().lines().filter(|l| !l.is_empty()) {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    println!("READ FAIL {path}: {e}");
                    continue;
                }
            };
            match super::dxf(&bytes) {
                Ok(p) => println!("OK {} pieces, {} paths  {path}", p.len(), p.iter().map(|x| x.paths.len()).sum::<usize>()),
                Err(e) => println!("ERR {e}  {path}"),
            }
        }
    }
}
