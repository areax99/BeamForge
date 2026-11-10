//! Project files of other programs: LightBurn projects (.lbrn, .lbrn2, .mage, .lbt — LBRNTranslator
//! 0x1000adf30 reads the file as a project and returns its shapes as one group) and SignCut
//! drawings (.sc, .scpro2 — SCProTranslator::ReadShapes 0x1000b5bb0).

use super::{Piece, base64, cubic};
use crate::i18n::tr;
use crate::model::{CUT_LAYER_COUNT, Geometry, Polyline, Shape};
use std::collections::HashMap;

type P = [f64; 2];
type Xf = [f64; 6];

fn ap(m: &Xf, p: P) -> P {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}
fn mul(inner: &Xf, outer: &Xf) -> Xf {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}
const ID: Xf = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn attr_f(n: &roxmltree::Node, k: &str) -> Option<f64> {
    n.attribute(k).and_then(|v| v.trim().parse().ok())
}

fn child<'a, 'i>(n: &roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.has_tag_name(name))
}

/// A LightBurn project's shapes, each keeping its kind, layer (CutIndex) and placement (XForm).
/// Groups come back as `groups` numbers 1, 2, … that the caller turns into real group ids.
pub fn lightburn(bytes: &[u8]) -> Result<Vec<Shape>, String> {
    let text = String::from_utf8_lossy(bytes);
    let doc = roxmltree::Document::parse(&text).map_err(|e| format!("{}: {e}", tr("Il progetto non è leggibile")))?;
    let root = doc.root_element();
    if !root.has_tag_name("LightBurnProject") {
        return Err(tr("Il file non è un progetto LightBurn.").into());
    }
    let mut cx = Lb { shapes: Vec::new(), verts: HashMap::new(), prims: HashMap::new(), next_group: 1 };
    for s in root.children().filter(|c| c.has_tag_name("Shape")) {
        cx.shape(&s, &ID, &[]);
    }
    if cx.shapes.is_empty() {
        return Err(tr("Il progetto non contiene forme.").into());
    }
    Ok(cx.shapes)
}

struct Lb {
    shapes: Vec<Shape>,
    verts: HashMap<String, String>,
    prims: HashMap<String, String>,
    next_group: u64,
}

struct Vert {
    p: P,
    c0: Option<P>,
    c1: Option<P>,
}

/// lbrn2 vertex list "V1 2c0x3c0y4c1x5c1y6V…" (or lbrn <V vx vy c0x …/> children).
fn parse_verts(node: &roxmltree::Node, text: &str) -> Vec<Vert> {
    let mut out = Vec::new();
    let elems: Vec<_> = node.children().filter(|c| c.has_tag_name("V")).collect();
    if !elems.is_empty() {
        for v in elems {
            let p = [attr_f(&v, "vx").unwrap_or(0.0), attr_f(&v, "vy").unwrap_or(0.0)];
            let c0 = attr_f(&v, "c0x").zip(attr_f(&v, "c0y")).map(|(x, y)| [x, y]);
            let c1 = attr_f(&v, "c1x").zip(attr_f(&v, "c1y")).map(|(x, y)| [x, y]);
            out.push(Vert { p, c0, c1 });
        }
        return out;
    }
    for chunk in text.split('V').skip(1) {
        // "x y" then optional c0x…c1y keys
        let mut parts = chunk.split('c');
        let xy: Vec<f64> = parts.next().unwrap_or("").split_whitespace().filter_map(|t| t.parse().ok()).collect();
        if xy.len() < 2 {
            continue;
        }
        let mut v = Vert { p: [xy[0], xy[1]], c0: None, c1: None };
        let mut c = HashMap::new();
        for kv in parts {
            if kv.len() > 2 {
                if let Ok(val) = kv[2..].trim().parse::<f64>() {
                    c.insert(kv[..2].to_string(), val);
                }
            }
        }
        if let (Some(x), Some(y)) = (c.get("0x"), c.get("0y")) {
            v.c0 = Some([*x, *y]);
        }
        if let (Some(x), Some(y)) = (c.get("1x"), c.get("1y")) {
            v.c1 = Some([*x, *y]);
        }
        out.push(v);
    }
    out
}

/// Primitives: lbrn2 "L0 1B1 2…" or "LineClosed"; lbrn <P T="L" p0 p1/> children.
fn parse_prims(node: &roxmltree::Node, text: &str, n_verts: usize) -> Vec<(bool, usize, usize)> {
    let elems: Vec<_> = node.children().filter(|c| c.has_tag_name("P")).collect();
    if !elems.is_empty() {
        return elems
            .iter()
            .filter_map(|p| Some((p.attribute("T") == Some("B"), p.attribute("p0")?.parse().ok()?, p.attribute("p1")?.parse().ok()?)))
            .collect();
    }
    let t = text.trim();
    if t.starts_with("LineClosed") || t.is_empty() && n_verts > 1 {
        let mut v: Vec<_> = (0..n_verts.saturating_sub(1)).map(|i| (false, i, i + 1)).collect();
        if t.starts_with("LineClosed") && n_verts > 2 {
            v.push((false, n_verts - 1, 0));
        }
        return v;
    }
    let mut out = Vec::new();
    let mut kind = ' ';
    let mut nums = String::new();
    let push = |k: char, s: &str, out: &mut Vec<(bool, usize, usize)>| {
        let n: Vec<usize> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if n.len() >= 2 && (k == 'L' || k == 'B') {
            out.push((k == 'B', n[0], n[1]));
        }
    };
    for ch in t.chars() {
        if ch.is_ascii_alphabetic() {
            push(kind, &nums, &mut out);
            kind = ch;
            nums.clear();
        } else {
            nums.push(ch);
        }
    }
    push(kind, &nums, &mut out);
    out
}

/// Chain primitives into outlines; a chain that returns to its first vertex is closed.
fn build_paths(verts: &[Vert], prims: &[(bool, usize, usize)]) -> Vec<Polyline> {
    let mut out: Vec<Polyline> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    let (mut start, mut last) = (usize::MAX, usize::MAX);
    for &(bez, a, b) in prims {
        if a >= verts.len() || b >= verts.len() {
            continue;
        }
        if a != last {
            if cur.len() > 1 {
                out.push(Polyline { pts: std::mem::take(&mut cur), closed: false });
            }
            cur = vec![verts[a].p];
            start = a;
        }
        if bez {
            let p1 = verts[a].c0.unwrap_or(verts[a].p);
            let p2 = verts[b].c1.unwrap_or(verts[b].p);
            cur.extend(cubic(verts[a].p, p1, p2, verts[b].p));
        } else {
            cur.push(verts[b].p);
        }
        last = b;
        if b == start {
            cur.pop();
            out.push(Polyline { pts: std::mem::take(&mut cur), closed: true });
            last = usize::MAX;
        }
    }
    if cur.len() > 1 {
        out.push(Polyline { pts: cur, closed: false });
    }
    out
}

impl Lb {
    fn xform(n: &roxmltree::Node) -> Xf {
        let Some(t) = child(n, "XForm").and_then(|x| x.text()) else { return ID };
        let v: Vec<f64> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() == 6 { [v[0], v[1], v[2], v[3], v[4], v[5]] } else { ID }
    }

    fn place(&mut self, geom: Geometry, layer: usize, m: &Xf, groups: &[u64]) {
        let sx = (m[0] * m[0] + m[1] * m[1]).sqrt();
        let det = m[0] * m[3] - m[1] * m[2];
        let skew = (m[0] * m[2] + m[1] * m[3]).abs();
        let mut s = if sx > 1e-12 && skew < 1e-6 * sx.max(1.0) {
            let mut s = Shape::new(0, geom, layer, m[4], m[5]);
            s.sx = sx;
            s.sy = det / sx;
            s.rot = m[1].atan2(m[0]).to_degrees();
            s
        } else {
            // skewed: bake the transform into the outline
            let paths: Vec<Polyline> = geom.local_paths().into_iter().map(|p| Polyline { pts: p.pts.iter().map(|q| ap(m, *q)).collect(), closed: p.closed }).collect();
            Shape::new(0, Geometry::Path { subpaths: paths }, layer, 0.0, 0.0)
        };
        s.groups = groups.to_vec();
        self.shapes.push(s);
    }

    fn shape(&mut self, n: &roxmltree::Node, parent: &Xf, groups: &[u64]) {
        let m = mul(&Self::xform(n), parent);
        let layer = n.attribute("CutIndex").and_then(|c| c.parse::<usize>().ok()).unwrap_or(0).min(CUT_LAYER_COUNT - 1);
        match n.attribute("Type").unwrap_or("") {
            "Rect" => {
                let g = Geometry::Rect { w: attr_f(n, "W").unwrap_or(0.0), h: attr_f(n, "H").unwrap_or(0.0), radius: attr_f(n, "Cr").unwrap_or(0.0) };
                self.place(g, layer, &m, groups);
            }
            "Ellipse" => {
                let g = Geometry::Ellipse { rx: attr_f(n, "Rx").unwrap_or(0.0), ry: attr_f(n, "Ry").unwrap_or(0.0) };
                self.place(g, layer, &m, groups);
            }
            "Polygon" => {
                let sides = attr_f(n, "N").unwrap_or(6.0) as u32;
                let r = attr_f(n, "Rx").or(attr_f(n, "R")).unwrap_or(10.0);
                let ry = attr_f(n, "Ry").unwrap_or(r);
                self.place(Geometry::Polygon { sides, r, ry: Some(ry) }, layer, &m, groups);
            }
            "Group" => {
                let gid = self.next_group;
                self.next_group += 1;
                let mut gs = groups.to_vec();
                gs.push(gid);
                if let Some(ch) = child(n, "Children") {
                    for c in ch.children().filter(|c| c.has_tag_name("Shape")) {
                        self.shape(&c, &m, &gs);
                    }
                }
            }
            "Bitmap" => {
                let (w, h) = (attr_f(n, "W").unwrap_or(0.0), attr_f(n, "H").unwrap_or(0.0));
                let data = base64(n.attribute("Data").unwrap_or(""));
                if let Ok(img) = image::load_from_memory(&data) {
                    let g = img.to_luma_alpha8();
                    let (px_w, px_h) = g.dimensions();
                    let gray = g.pixels().map(|p| ((p[0] as f64 * p[1] as f64 + 255.0 * (255.0 - p[1] as f64)) / 255.0).round() as u8).collect();
                    self.place(Geometry::Image { w, h, px_w, px_h, gray }, layer, &m, groups);
                }
            }
            _ => {
                // Path, Text (its BackupPath) and anything else carrying a vertex list
                let src = if n.attribute("Type") == Some("Text") { child(n, "BackupPath").unwrap_or(*n) } else { *n };
                let vl = child(&src, "VertList");
                let pl = child(&src, "PrimList");
                let vid = src.attribute("VertID").map(String::from);
                let pid = src.attribute("PrimID").map(String::from);
                let vtext = vl.and_then(|v| v.text()).map(String::from).or_else(|| vid.as_ref().and_then(|i| self.verts.get(i).cloned())).unwrap_or_default();
                let ptext = pl.and_then(|p| p.text()).map(String::from).or_else(|| pid.as_ref().and_then(|i| self.prims.get(i).cloned())).unwrap_or_default();
                if let (Some(i), true) = (vid, vl.is_some()) {
                    self.verts.insert(i, vtext.clone());
                }
                if let (Some(i), true) = (pid, pl.is_some()) {
                    self.prims.insert(i, ptext.clone());
                }
                let verts = match vl {
                    Some(v) => parse_verts(&v, &vtext),
                    None => parse_verts(&src, &vtext),
                };
                if verts.is_empty() {
                    return;
                }
                let prims = match pl {
                    Some(p) => parse_prims(&p, &ptext, verts.len()),
                    None => parse_prims(&src, &ptext, verts.len()),
                };
                let paths = build_paths(&verts, &prims);
                if !paths.is_empty() {
                    let m = if n.attribute("Type") == Some("Text") && src != *n { mul(&Self::xform(&src), &m) } else { m };
                    self.place(Geometry::Path { subpaths: paths }, layer, &m, groups);
                }
            }
        }
    }
}

/// SignCut drawings: <SignCut><Image><Scale/><Paths><Path><Nodes><Node status point bez1 bez2/>.
/// A node whose status has bit 14 (0x4000) starts a new outline (ReadNode 0x1000b62fc); a
/// segment is a curve unless its control points sit on its ends; a path ending on its start
/// is closed (ReadPath 0x1000b5fac). Values are "x;y" pairs.
pub fn signcut(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let text = String::from_utf8_lossy(bytes);
    let doc = roxmltree::Document::parse(&text).map_err(|_| tr("Il file SignCut non è leggibile (formato non riconosciuto)."))?;
    let root = doc.root_element();
    if !root.has_tag_name("SignCut") {
        return Err(tr("Il file non è un disegno SignCut.").into());
    }
    let pair = |s: Option<&str>| -> Option<P> {
        let v: Vec<f64> = s?.split(';').filter_map(|t| t.trim().parse().ok()).collect();
        (v.len() == 2).then(|| [v[0], v[1]])
    };
    let mut scale = [1.0, 1.0];
    let mut pieces = Vec::new();
    for n in root.descendants() {
        if n.has_tag_name("Scale") {
            // ReadScale 0x1000b5e24 reads two one-letter attributes (estimated: x and y)
            let sx = attr_f(&n, "x").or(attr_f(&n, "X")).unwrap_or(1.0);
            let sy = attr_f(&n, "y").or(attr_f(&n, "Y")).unwrap_or(1.0);
            scale = [if sx != 0.0 { sx } else { 1.0 }, if sy != 0.0 { sy } else { 1.0 }];
        }
        if !n.has_tag_name("Path") {
            continue;
        }
        let mut paths: Vec<Polyline> = Vec::new();
        let mut cur: Vec<P> = Vec::new();
        let mut prev: Option<(P, P)> = None; // (point, outgoing control)
        for node in n.descendants().filter(|c| c.has_tag_name("Node")) {
            let status: i64 = node.attribute("status").and_then(|s| s.parse().ok()).unwrap_or(0);
            let Some(p) = pair(node.attribute("point")) else { continue };
            let p = [p[0] * scale[0], p[1] * scale[1]];
            let b1 = pair(node.attribute("bez1")).map(|b| [b[0] * scale[0], b[1] * scale[1]]).unwrap_or(p);
            let b2 = pair(node.attribute("bez2")).map(|b| [b[0] * scale[0], b[1] * scale[1]]).unwrap_or(p);
            if status & 0x4000 != 0 || prev.is_none() {
                if cur.len() > 1 {
                    paths.push(close_if_back(std::mem::take(&mut cur)));
                }
                cur = vec![p];
            } else if let Some((pp, pc)) = prev {
                if pc == pp && b1 == p {
                    cur.push(p);
                } else {
                    cur.extend(cubic(pp, pc, b1, p));
                }
            }
            prev = Some((p, b2));
        }
        if cur.len() > 1 {
            paths.push(close_if_back(cur));
        }
        if !paths.is_empty() {
            pieces.push(Piece { layer: 0, paths });
        }
    }
    if pieces.is_empty() {
        return Err(tr("Il file SignCut non contiene tracciati.").into());
    }
    Ok(pieces)
}

fn close_if_back(mut pts: Vec<P>) -> Polyline {
    let closed = pts.len() > 2 && {
        let (a, b) = (pts[0], *pts.last().unwrap());
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    };
    if closed {
        pts.pop();
    }
    Polyline { pts, closed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lbrn2_rect_path_group() {
        let x = r#"<?xml version="1.0"?>
<LightBurnProject AppVersion="1.7" FormatVersion="1">
  <Shape Type="Rect" CutIndex="2" W="20" H="10" Cr="0"><XForm>1 0 0 1 50 60</XForm></Shape>
  <Shape Type="Group"><XForm>1 0 0 1 0 0</XForm><Children>
    <Shape Type="Path" CutIndex="1" VertID="0" PrimID="0"><XForm>1 0 0 1 0 0</XForm>
      <VertList>V0 0V10 0V10 10c1x5c1y10</VertList><PrimList>L0 1L1 2L2 0</PrimList></Shape>
    <Shape Type="Path" CutIndex="1" VertID="0" PrimID="0"><XForm>1 0 0 1 100 0</XForm></Shape>
  </Children></Shape>
</LightBurnProject>"#;
        let s = lightburn(x.as_bytes()).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].layer, 2);
        assert_eq!((s[0].tx, s[0].ty), (50.0, 60.0));
        assert_eq!(s[1].groups, vec![1]);
        if let Geometry::Path { subpaths } = &s[2].geom {
            assert!(subpaths[0].closed);
        } else {
            panic!()
        }
        assert_eq!(s[2].tx, 100.0);
    }

    #[test]
    fn signcut_square() {
        let x = r#"<SignCut Version="2"><Image><Paths><Path><Nodes>
<Node status="16384" point="0;0"/><Node status="0" point="10;0"/><Node status="0" point="10;10"/><Node status="0" point="0;0"/>
</Nodes></Path></Paths></Image></SignCut>"#;
        let p = signcut(x.as_bytes()).unwrap();
        assert!(p[0].paths[0].closed);
        assert_eq!(p[0].paths[0].pts.len(), 3);
    }
}
