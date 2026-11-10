//! Importing every file kind the original's Import dialog lists (TranslatorDescriptor registry
//! sub_1000fe618): vector drawings, projects, plotter and machine files, 3D models and PCB files.
//! Each reader turns its file into pieces (a layer and its outlines, mm, Y up); `shapes_from`
//! makes BeamForge shapes of them. The rules each reader follows are in MECHANICS.md.

pub mod mesh;
pub mod pcb;
pub mod pdf;
pub mod project;
pub mod ruida;
pub mod vector;

use crate::i18n::{tr, trf};
use crate::model::{BBox, Geometry, Polyline, Shape};

/// One drawn piece of an imported file: its layer and outlines (mm, Y up).
pub struct Piece {
    pub layer: usize,
    pub paths: Vec<Polyline>,
}

/// What a file turned into.
pub enum Imported {
    /// Vector pieces, grouped together when there are several (Ungroup splits them).
    Shapes(Vec<Shape>),
    /// Shapes that keep their own geometry and position (projects).
    Placed(Vec<Shape>),
    /// A picture for an Image layer (bitmaps and 3D depth maps).
    Image(Shape),
}

/// The file groups of the original's Import dialog, in its order (picture 18 of the person).
pub const IMAGE_EXTS: &[&str] = &["bmp", "jpg", "jpeg", "png", "gif", "tif", "tiff", "tga", "webp"];
pub const VECTOR_EXTS: &[&str] =
    &["ai", "pdf", "sc", "dxf", "gc", "gcode", "nc", "hpgl", "plt", "lbrn", "lbrn2", "mage", "lbt", "rd", "rld", "scpro2", "svg"];
pub const MODEL_EXTS: &[&str] = &["3mf", "amf", "obj", "off", "ply", "stl"];
pub const PCB_EXTS: &[&str] = &[
    "gbr", "grb", "gbx", "ger", "pho", "gtl", "gbl", "gts", "gbs", "gto", "gbo", "gtp", "gbp", "gko", "gm1", "g1", "g2", "g3",
    "g4", "g5", "g6", "g7", "g8", "g9", "g1l", "g2l", "g3l", "g4l", "g5l", "g6l", "g7l", "g8l", "g9l", "drl", "xln", "exc", "xnc",
    "gbrjob",
];

/// Every extension BeamForge imports.
pub fn all_exts() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for e in VECTOR_EXTS.iter().chain(MODEL_EXTS).chain(PCB_EXTS).chain(IMAGE_EXTS) {
        if !v.contains(e) {
            v.push(e);
        }
    }
    v
}

/// Read one file of any supported kind.
pub fn import_file(path: &std::path::Path) -> Result<Imported, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if IMAGE_EXTS.contains(&ext.as_str()) {
        return crate::import::import_image(path).map(Imported::Image);
    }
    let bytes = std::fs::read(path).map_err(|e| trf("Impossibile leggere il file: {}", &[&e]))?;
    let pieces = match ext.as_str() {
        "svg" => return crate::import::import_svg(&bytes).map(Imported::Shapes),
        "dxf" => vector::dxf(&bytes)?,
        "hpgl" | "plt" => vector::hpgl(&bytes)?,
        "gc" | "gcode" | "nc" => vector::gcode(&bytes)?,
        "ai" | "pdf" => pdf::import(&bytes)?,
        "lbrn" | "lbrn2" | "mage" | "lbt" => return project::lightburn(&bytes).map(Imported::Placed),
        "sc" | "scpro2" => project::signcut(&bytes)?,
        "rd" => ruida::rd(&bytes)?,
        "rld" => ruida::rld(&bytes)?,
        "stl" | "obj" | "off" | "ply" | "amf" | "3mf" => {
            let tris = mesh::read(&ext, &bytes)?;
            return mesh::depth_map(&tris).map(Imported::Image);
        }
        "gbrjob" => pcb::gerber_job(path, &bytes)?,
        "drl" | "xln" | "exc" | "xnc" => pcb::excellon(&bytes)?,
        e if PCB_EXTS.contains(&e) => {
            // Gerber and Excellon share some extensions in the wild: look at the content
            if pcb::looks_like_excellon(&bytes) { pcb::excellon(&bytes)? } else { pcb::gerber(&bytes, pcb::layer_for_name(path))? }
        }
        _ => return Err(trf("Formato di file non supportato: .{}", &[&ext])),
    };
    shapes_from(pieces).map(Imported::Shapes)
}

/// Turn pieces into shapes whose origin is the centre of their outline (like SVG import).
pub fn shapes_from(pieces: Vec<Piece>) -> Result<Vec<Shape>, String> {
    let mut shapes = Vec::new();
    for piece in pieces {
        let paths: Vec<Polyline> = piece
            .paths
            .into_iter()
            .map(Polyline::cleaned)
            .filter(|p| p.pts.len() > 1 || (p.pts.len() == 1 && !p.closed))
            .filter(|p| p.pts.iter().all(|q| q[0].is_finite() && q[1].is_finite()))
            .collect();
        if paths.is_empty() {
            continue;
        }
        let mut b = BBox::empty();
        for p in paths.iter().flat_map(|p| p.pts.iter()) {
            b.add(*p);
        }
        let c = b.center();
        let sub = paths
            .into_iter()
            .map(|pl| Polyline { pts: pl.pts.into_iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect(), closed: pl.closed })
            .collect();
        shapes.push(Shape::new(0, Geometry::Path { subpaths: sub }, piece.layer, c[0], c[1]));
    }
    if shapes.is_empty() {
        return Err(tr("Il file non contiene forme utilizzabili.").into());
    }
    Ok(shapes)
}

/// Points of a circular arc from angle a0 to a1 (radians, any direction), first point excluded.
pub fn arc_points(c: [f64; 2], r: f64, a0: f64, a1: f64) -> Vec<[f64; 2]> {
    let sweep = a1 - a0;
    let n = ((sweep.abs() * r.abs().sqrt() * 4.0).ceil() as usize).clamp(4, 720);
    (1..=n).map(|i| {
        let a = a0 + sweep * i as f64 / n as f64;
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    }).collect()
}

/// A full circle as a closed outline.
pub fn circle(c: [f64; 2], r: f64) -> Polyline {
    let n = ((r.abs().sqrt() * 24.0).ceil() as usize).clamp(16, 360);
    Polyline {
        pts: (0..n).map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        }).collect(),
        closed: true,
    }
}

/// Cubic Bézier points from p0 (excluded) to p3.
pub fn cubic(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], p3: [f64; 2]) -> Vec<[f64; 2]> {
    (1..=16).map(|i| {
        let t = i as f64 / 16.0;
        let u = 1.0 - t;
        let k = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        [k[0] * p0[0] + k[1] * p1[0] + k[2] * p2[0] + k[3] * p3[0], k[0] * p0[1] + k[1] * p1[1] + k[2] * p2[1] + k[3] * p3[1]]
    }).collect()
}

/// Close an outline when its ends meet within `tol` (PathShape::Close, HPGL uses 0.001).
pub fn finish(mut pts: Vec<[f64; 2]>, tol: f64) -> Polyline {
    let closed = pts.len() > 2 && {
        let (a, b) = (pts[0], *pts.last().unwrap());
        (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol
    };
    if closed {
        pts.pop();
    }
    Polyline { pts, closed }
}

/// Decode standard base64 (embedded pictures in project files); whitespace is skipped.
pub fn base64(s: &str) -> Vec<u8> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        if let Some(v) = val(c) {
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
                acc &= (1 << bits) - 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round() {
        assert_eq!(base64("aGVsbG8="), b"hello");
    }

    /// Manual check on real files: BF_TRY="a.ai|b.webp" cargo test try_files -- --ignored --nocapture
    #[test]
    #[ignore]
    fn try_files() {
        for p in std::env::var("BF_TRY").unwrap_or_default().split('|').filter(|s| !s.is_empty()) {
            match import_file(std::path::Path::new(p)) {
                Ok(Imported::Shapes(s)) | Ok(Imported::Placed(s)) => {
                    let n: usize = s.iter().map(|x| x.geom.local_paths().len()).sum();
                    let mut b = BBox::empty();
                    for x in &s {
                        b.union(&x.bbox());
                    }
                    println!("{p}: {} shapes, {n} outlines, {:.1} x {:.1} mm", s.len(), b.w(), b.h());
                }
                Ok(Imported::Image(s)) => println!("{p}: image {:.1} x {:.1} mm", s.bbox().w(), s.bbox().h()),
                Err(e) => println!("{p}: ERROR {e}"),
            }
        }
    }

    #[test]
    fn every_listed_extension_has_a_reader() {
        for e in all_exts() {
            let p = std::path::PathBuf::from(format!("/nonexistent/file.{e}"));
            let r = import_file(&p);
            if let Err(msg) = r {
                assert!(!msg.contains("non supportato"), "{e}: {msg}");
            }
        }
    }
}
