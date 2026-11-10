//! 3D models (STL, OBJ, OFF, PLY, AMF, 3MF — the original's STL/OBJ/OFF/PLY/AMF/ThreeMF
//! translators) and their depth map. The original opens these in Model3DImportWizard, which can
//! insert the model or convert it into a depth-map picture for engraving
//! (BuildDepthMapResult 0x1008e76f4 → DepthMapConvert::GetDepthMapShape). BeamForge makes the
//! depth map straight away: seen from above, the highest point is white and the lowest black.

use crate::i18n::{tr, trf};
use crate::model::{Geometry, Shape};
use std::io::Read;

pub type V3 = [f64; 3];
pub type Tri = [V3; 3];

/// Depth-map resolution: 10 pixels per mm, the default 0.1 mm line interval (estimated).
pub const DEPTH_PX_PER_MM: f64 = 10.0;
/// Largest side of a depth map in pixels (as for imported pictures).
pub const DEPTH_MAX_PX: u32 = 1600;

/// mm per unit for the unit names used by 3MF and AMF (ThreeMFTranslator::unitScale 0x1000d2410,
/// AMFTranslator::unitScale 0x100076200).
pub fn unit_scale(u: &str) -> f64 {
    match u.trim().to_lowercase().as_str() {
        "micron" | "micrometer" => 0.001,
        "centimeter" => 10.0,
        "meter" => 1000.0,
        "inch" => 25.4,
        "foot" | "feet" => 304.8,
        _ => 1.0,
    }
}

pub fn read(ext: &str, b: &[u8]) -> Result<Vec<Tri>, String> {
    let tris = match ext {
        "stl" => stl(b)?,
        "obj" => obj(b),
        "off" => off(b)?,
        "ply" => ply(b)?,
        "amf" => amf(b)?,
        "3mf" => three_mf(b)?,
        _ => Vec::new(),
    };
    if tris.is_empty() {
        return Err(tr("Il modello 3D non contiene triangoli.").into());
    }
    Ok(tris)
}

fn fan(idx: &[usize], v: &[V3], out: &mut Vec<Tri>) {
    for k in 1..idx.len().saturating_sub(1) {
        if let (Some(a), Some(b), Some(c)) = (v.get(idx[0]), v.get(idx[k]), v.get(idx[k + 1])) {
            out.push([*a, *b, *c]);
        }
    }
}

fn stl(b: &[u8]) -> Result<Vec<Tri>, String> {
    // binary when the size matches the triangle count (STLTranslator::Import 0x1000b6a58)
    if b.len() >= 84 {
        let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
        if 84 + n * 50 == b.len() || !b.starts_with(b"solid") {
            let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64;
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let o = 84 + i * 50 + 12;
                if o + 36 > b.len() {
                    break;
                }
                out.push([[f(o), f(o + 4), f(o + 8)], [f(o + 12), f(o + 16), f(o + 20)], [f(o + 24), f(o + 28), f(o + 32)]]);
            }
            return Ok(out);
        }
    }
    let t = String::from_utf8_lossy(b);
    let mut out = Vec::new();
    let mut cur: Vec<V3> = Vec::new();
    for l in t.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        if w.first() == Some(&"vertex") && w.len() >= 4 {
            cur.push([w[1].parse().unwrap_or(0.0), w[2].parse().unwrap_or(0.0), w[3].parse().unwrap_or(0.0)]);
        } else if w.first() == Some(&"endloop") {
            let idx: Vec<usize> = (0..cur.len()).collect();
            fan(&idx, &cur, &mut out);
            cur.clear();
        }
    }
    Ok(out)
}

fn obj(b: &[u8]) -> Vec<Tri> {
    let t = String::from_utf8_lossy(b);
    let mut v: Vec<V3> = Vec::new();
    let mut out = Vec::new();
    for l in t.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.first() {
            Some(&"v") if w.len() >= 4 => v.push([w[1].parse().unwrap_or(0.0), w[2].parse().unwrap_or(0.0), w[3].parse().unwrap_or(0.0)]),
            Some(&"f") => {
                // OBJTranslator::parseFaceIndex: "v", "v/t", "v//n", negative = from the end
                let idx: Vec<usize> = w[1..]
                    .iter()
                    .filter_map(|f| f.split('/').next()?.parse::<i64>().ok())
                    .map(|i| if i < 0 { (v.len() as i64 + i) as usize } else { (i - 1).max(0) as usize })
                    .collect();
                fan(&idx, &v, &mut out);
            }
            _ => {}
        }
    }
    out
}

fn off(b: &[u8]) -> Result<Vec<Tri>, String> {
    let t = String::from_utf8_lossy(b);
    let mut toks = t.lines().map(|l| l.split('#').next().unwrap_or("")).flat_map(|l| l.split_whitespace()).peekable();
    let head = toks.next().unwrap_or("");
    if !head.ends_with("OFF") {
        return Err(tr("Il file OFF non è valido.").into());
    }
    let mut num = || toks.next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
    let (nv, nf, _) = (num() as usize, num() as usize, num());
    let v: Vec<V3> = (0..nv).map(|_| [num(), num(), num()]).collect();
    let mut out = Vec::new();
    for _ in 0..nf {
        let k = num() as usize;
        let idx: Vec<usize> = (0..k).map(|_| num() as usize).collect();
        fan(&idx, &v, &mut out);
    }
    Ok(out)
}

fn ply(b: &[u8]) -> Result<Vec<Tri>, String> {
    let end = b.windows(10).position(|w| w == b"end_header").ok_or_else(|| tr("Il file PLY non è valido."))?;
    let header = String::from_utf8_lossy(&b[..end]);
    let mut body = end + 10;
    while body < b.len() && (b[body] == b'\r' || b[body] == b'\n') {
        body += 1;
        if b[body - 1] == b'\n' {
            break;
        }
    }
    let mut format = "ascii";
    // elements: (name, count, properties (name, type, list-count-type))
    let mut elems: Vec<(String, usize, Vec<(String, String, Option<String>)>)> = Vec::new();
    for l in header.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.as_slice() {
            ["format", f, ..] => {
                format = if f.starts_with("binary_little") {
                    "le"
                } else if f.starts_with("binary_big") {
                    "be"
                } else {
                    "ascii"
                }
            }
            ["element", n, c] => elems.push((n.to_string(), c.parse().unwrap_or(0), Vec::new())),
            ["property", "list", ct, it, name] => {
                if let Some(e) = elems.last_mut() {
                    e.2.push((name.to_string(), it.to_string(), Some(ct.to_string())));
                }
            }
            ["property", t, name] => {
                if let Some(e) = elems.last_mut() {
                    e.2.push((name.to_string(), t.to_string(), None));
                }
            }
            _ => {}
        }
    }
    let size = |t: &str| match t {
        "char" | "uchar" | "int8" | "uint8" => 1,
        "short" | "ushort" | "int16" | "uint16" => 2,
        "double" | "float64" | "int64" | "uint64" => 8,
        _ => 4,
    };
    let mut pos = body;
    let mut ascii_toks = String::from_utf8_lossy(&b[body..]).split_whitespace().map(String::from).collect::<Vec<_>>().into_iter();
    let mut read = |t: &str, pos: &mut usize| -> f64 {
        if format == "ascii" {
            return ascii_toks.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        }
        let n = size(t);
        if *pos + n > b.len() {
            *pos += n;
            return 0.0;
        }
        let mut s = b[*pos..*pos + n].to_vec();
        *pos += n;
        if format == "be" {
            s.reverse();
        }
        match (t, n) {
            ("float" | "float32", 4) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
            (_, 8) if t.starts_with("double") || t == "float64" => f64::from_le_bytes(s.try_into().unwrap()),
            ("char" | "int8", 1) => s[0] as i8 as f64,
            (_, 1) => s[0] as f64,
            ("short" | "int16", 2) => i16::from_le_bytes([s[0], s[1]]) as f64,
            (_, 2) => u16::from_le_bytes([s[0], s[1]]) as f64,
            ("int" | "int32", 4) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
            (_, 4) => u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
            _ => 0.0,
        }
    };
    let mut v: Vec<V3> = Vec::new();
    let mut out = Vec::new();
    for (name, count, props) in &elems {
        for _ in 0..*count {
            let mut p = [0.0; 3];
            let mut idx: Vec<usize> = Vec::new();
            for (pn, t, list) in props {
                if let Some(ct) = list {
                    let n = read(ct, &mut pos) as usize;
                    let vals: Vec<usize> = (0..n).map(|_| read(t, &mut pos) as usize).collect();
                    if pn == "vertex_indices" || pn == "vertex_index" {
                        idx = vals;
                    }
                } else {
                    let x = read(t, &mut pos);
                    match pn.as_str() {
                        "x" => p[0] = x,
                        "y" => p[1] = x,
                        "z" => p[2] = x,
                        _ => {}
                    }
                }
            }
            if name == "vertex" {
                v.push(p);
            } else if name == "face" {
                fan(&idx, &v, &mut out);
            }
        }
    }
    Ok(out)
}

fn unzip_first(b: &[u8], wanted: impl Fn(&str) -> bool) -> Result<Vec<u8>, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).map_err(|e| trf("Archivio non leggibile: {}", &[&e]))?;
    let name = (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string())).find(|n| wanted(n));
    let Some(name) = name else { return Err(tr("Nell'archivio manca il modello 3D.").into()) };
    let mut f = z.by_name(&name).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    f.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn fattr(n: &roxmltree::Node, k: &str) -> f64 {
    n.attribute(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)
}

/// 3MF: zip with 3D/3dmodel.model; objects with meshes or components, placed by <build> items
/// (transform "m00 m01 m02 m10 m11 m12 m20 m21 m22 m30 m31 m32", row vectors).
fn three_mf(b: &[u8]) -> Result<Vec<Tri>, String> {
    let xml = unzip_first(b, |n| n.to_lowercase().ends_with(".model"))?;
    let text = String::from_utf8_lossy(&xml);
    let doc = roxmltree::Document::parse(&text).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let scale = unit_scale(root.attribute("unit").unwrap_or("millimeter"));
    type M = [f64; 12];
    let ident: M = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    let parse_m = |s: Option<&str>| -> M {
        let v: Vec<f64> = s.unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() == 12 { v.try_into().unwrap() } else { ident }
    };
    let ap = |m: &M, p: V3| -> V3 {
        [p[0] * m[0] + p[1] * m[3] + p[2] * m[6] + m[9], p[0] * m[1] + p[1] * m[4] + p[2] * m[7] + m[10], p[0] * m[2] + p[1] * m[5] + p[2] * m[8] + m[11]]
    };
    let mul = |a: &M, b: &M| -> M {
        // a then b
        let mut r = [0.0; 12];
        for i in 0..4 {
            for j in 0..3 {
                let row = |k: usize| if i < 3 { a[i * 3 + k] } else { a[9 + k] };
                r[i * 3 + j] = row(0) * b[j] + row(1) * b[3 + j] + row(2) * b[6 + j] + if i == 3 { b[9 + j] } else { 0.0 };
            }
        }
        r
    };
    let objects: std::collections::HashMap<String, roxmltree::Node> = root.descendants().filter(|n| n.has_tag_name("object")).filter_map(|n| Some((n.attribute("id")?.to_string(), n))).collect();
    fn emit(
        id: &str,
        m: &[f64; 12],
        objects: &std::collections::HashMap<String, roxmltree::Node>,
        out: &mut Vec<Tri>,
        depth: usize,
        ap: &dyn Fn(&[f64; 12], V3) -> V3,
        mul: &dyn Fn(&[f64; 12], &[f64; 12]) -> [f64; 12],
        parse_m: &dyn Fn(Option<&str>) -> [f64; 12],
    ) {
        if depth > 16 {
            return;
        }
        let Some(o) = objects.get(id) else { return };
        if let Some(mesh) = o.children().find(|c| c.has_tag_name("mesh")) {
            let v: Vec<V3> = mesh.descendants().filter(|n| n.has_tag_name("vertex")).map(|n| ap(m, [fattr(&n, "x"), fattr(&n, "y"), fattr(&n, "z")])).collect();
            for t in mesh.descendants().filter(|n| n.has_tag_name("triangle")) {
                let i = |k: &str| t.attribute(k).and_then(|x| x.parse::<usize>().ok()).unwrap_or(usize::MAX);
                fan(&[i("v1"), i("v2"), i("v3")], &v, out);
            }
        }
        for c in o.descendants().filter(|n| n.has_tag_name("component")) {
            let cm = mul(&parse_m(c.attribute("transform")), m);
            emit(c.attribute("objectid").unwrap_or(""), &cm, objects, out, depth + 1, ap, mul, parse_m);
        }
    }
    let mut out = Vec::new();
    let items: Vec<_> = root.descendants().filter(|n| n.has_tag_name("item")).collect();
    if items.is_empty() {
        for id in objects.keys() {
            emit(id, &ident, &objects, &mut out, 0, &ap, &mul, &parse_m);
        }
    }
    for it in items {
        let m = parse_m(it.attribute("transform"));
        emit(it.attribute("objectid").unwrap_or(""), &m, &objects, &mut out, 0, &ap, &mul, &parse_m);
    }
    Ok(out.into_iter().map(|t| t.map(|p| p.map(|c| c * scale))).collect())
}

/// AMF (plain or zipped XML): objects with vertices and volumes of triangles; a constellation
/// places instances with deltax/deltay/deltaz and rz.
fn amf(b: &[u8]) -> Result<Vec<Tri>, String> {
    let data = if b.starts_with(b"PK") { unzip_first(b, |_| true)? } else { b.to_vec() };
    let text = String::from_utf8_lossy(&data);
    let doc = roxmltree::Document::parse(&text).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let scale = unit_scale(root.attribute("unit").unwrap_or("millimeter"));
    let num = |n: &roxmltree::Node, tag: &str| -> f64 { n.children().find(|c| c.has_tag_name(tag)).and_then(|c| c.text()).and_then(|t| t.trim().parse().ok()).unwrap_or(0.0) };
    let mut objects: std::collections::HashMap<String, Vec<Tri>> = std::collections::HashMap::new();
    for o in root.children().filter(|n| n.has_tag_name("object")) {
        let mut tris = Vec::new();
        for mesh in o.children().filter(|n| n.has_tag_name("mesh")) {
            let v: Vec<V3> = mesh
                .descendants()
                .filter(|n| n.has_tag_name("coordinates"))
                .map(|c| [num(&c, "x"), num(&c, "y"), num(&c, "z")])
                .collect();
            for t in mesh.descendants().filter(|n| n.has_tag_name("triangle")) {
                let i = |k: &str| num(&t, k) as usize;
                fan(&[i("v1"), i("v2"), i("v3")], &v, &mut tris);
            }
        }
        objects.insert(o.attribute("id").unwrap_or("").to_string(), tris);
    }
    let mut out = Vec::new();
    let insts: Vec<_> = root.descendants().filter(|n| n.has_tag_name("instance")).collect();
    if insts.is_empty() {
        for t in objects.values() {
            out.extend(t.iter().copied());
        }
    }
    for inst in insts {
        let Some(t) = objects.get(inst.attribute("objectid").unwrap_or("")) else { continue };
        let (dx, dy, dz, rz) = (num(&inst, "deltax"), num(&inst, "deltay"), num(&inst, "deltaz"), num(&inst, "rz").to_radians());
        let (s, c) = rz.sin_cos();
        out.extend(t.iter().map(|tri| tri.map(|p| [p[0] * c - p[1] * s + dx, p[0] * s + p[1] * c + dy, p[2] + dz])));
    }
    Ok(out.into_iter().map(|t| t.map(|p| p.map(|c| c * scale))).collect())
}

/// Seen from above (+Z): each pixel keeps the highest surface; white is the top, black the
/// lowest point, the background white (nothing to engrave). Estimated conventions.
pub fn depth_map(tris: &[Tri]) -> Result<Shape, String> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for t in tris {
        for p in t {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    let (wmm, hmm) = (hi[0] - lo[0], hi[1] - lo[1]);
    if !(wmm > 0.0 && hmm > 0.0) {
        return Err(tr("Il modello 3D visto dall'alto è vuoto.").into());
    }
    let mut ppm = DEPTH_PX_PER_MM;
    if wmm.max(hmm) * ppm > DEPTH_MAX_PX as f64 {
        ppm = DEPTH_MAX_PX as f64 / wmm.max(hmm);
    }
    let (pw, ph) = (((wmm * ppm).ceil() as u32).max(1), ((hmm * ppm).ceil() as u32).max(1));
    let mut z = vec![f64::NEG_INFINITY; (pw * ph) as usize];
    for t in tris {
        let px: Vec<[f64; 3]> = t.iter().map(|p| [(p[0] - lo[0]) * ppm, (hi[1] - p[1]) * ppm, p[2]]).collect();
        let (a, b, c) = (px[0], px[1], px[2]);
        let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if den.abs() < 1e-12 {
            continue;
        }
        let x0 = a[0].min(b[0]).min(c[0]).floor().max(0.0) as u32;
        let x1 = (a[0].max(b[0]).max(c[0]).ceil() as u32).min(pw - 1);
        let y0 = a[1].min(b[1]).min(c[1]).floor().max(0.0) as u32;
        let y1 = (a[1].max(b[1]).max(c[1]).ceil() as u32).min(ph - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let l0 = ((b[1] - c[1]) * (fx - c[0]) + (c[0] - b[0]) * (fy - c[1])) / den;
                let l1 = ((c[1] - a[1]) * (fx - c[0]) + (a[0] - c[0]) * (fy - c[1])) / den;
                let l2 = 1.0 - l0 - l1;
                if l0 < -1e-9 || l1 < -1e-9 || l2 < -1e-9 {
                    continue;
                }
                let zz = l0 * a[2] + l1 * b[2] + l2 * c[2];
                let i = (y * pw + x) as usize;
                if zz > z[i] {
                    z[i] = zz;
                }
            }
        }
    }
    let span = (hi[2] - lo[2]).max(1e-9);
    let gray: Vec<u8> = z.iter().map(|v| if v.is_finite() { (((v - lo[2]) / span) * 255.0).round().clamp(0.0, 255.0) as u8 } else { 255 }).collect();
    let (w, h) = (pw as f64 / ppm, ph as f64 / ppm);
    Ok(Shape::new(0, Geometry::Image { w, h, px_w: pw, px_h: ph, gray }, 0, w / 2.0 + 10.0, h / 2.0 + 10.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_OBJ: &str = "v 0 0 0\nv 10 0 0\nv 10 10 0\nv 0 10 0\nv 0 0 5\nv 10 0 5\nv 10 10 5\nv 0 10 5\nf 5 6 7 8\nf 1 2 3 4\n";

    #[test]
    fn obj_top_is_white() {
        let t = read("obj", CUBE_OBJ.as_bytes()).unwrap();
        assert_eq!(t.len(), 4);
        let s = depth_map(&t).unwrap();
        if let Geometry::Image { px_w, px_h, gray, w, .. } = s.geom {
            assert_eq!((px_w, px_h), (100, 100));
            assert!((w - 10.0).abs() < 1e-9);
            assert_eq!(gray[50 * 100 + 50], 255);
        } else {
            panic!()
        }
    }

    #[test]
    fn binary_stl_and_ascii_ply_and_off() {
        let mut b = vec![0u8; 80];
        b.extend(1u32.to_le_bytes());
        b.extend([0u8; 12]);
        for v in [[0f32, 0.0, 0.0], [5.0, 0.0, 1.0], [0.0, 5.0, 2.0]] {
            for c in v {
                b.extend(c.to_le_bytes());
            }
        }
        b.extend([0u8; 2]);
        assert_eq!(read("stl", &b).unwrap().len(), 1);
        let ply = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 1\n3 0 1 2\n";
        assert_eq!(read("ply", ply.as_bytes()).unwrap().len(), 1);
        let off = "OFF\n3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";
        assert_eq!(read("off", off.as_bytes()).unwrap().len(), 1);
    }
}
