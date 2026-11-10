//! Importing SVG vector files and bitmap images.

use crate::i18n::{tr, trf};
use crate::model::{BBox, Geometry, Polyline, Shape, nearest_layer};

/// SVG user units: 96 per inch, or 72 per inch for unitless Adobe Illustrator files
/// (SVGTranslator::Import 0x1000b7b30: creator comment → scale 0.35278 / 0.26458 mm per unit).
const PX_TO_MM: f64 = 25.4 / 96.0;
const PT_TO_MM: f64 = 25.4 / 72.0;

/// mm per SVG user unit. usvg already turns physical units (mm, cm, in, pt, pc) into 96-dpi px;
/// only a unitless file made by Adobe Illustrator is read at 72 per inch, like the original.
fn unit_scale(bytes: &[u8]) -> f64 {
    let text = String::from_utf8_lossy(bytes);
    let physical = text.find("<svg").and_then(|i| {
        let tag = &text[i..i + text[i..].find('>').unwrap_or(0)];
        let w = tag.split("width=").nth(1)?;
        let v: String = w.trim_start_matches(['"', '\'']).chars().take_while(|c| *c != '"' && *c != '\'').collect();
        Some(["mm", "cm", "in", "pt", "pc"].iter().any(|u| v.trim().ends_with(u)))
    });
    if physical != Some(true) && text.contains("Adobe Illustrator") { PT_TO_MM } else { PX_TO_MM }
}

pub fn import_svg(bytes: &[u8]) -> Result<Vec<Shape>, String> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(bytes, &opt).map_err(|e| trf("Impossibile leggere l'SVG: {}", &[&e]))?;
    let height = tree.size().height() as f64;
    // one entry per SVG shape element, so the drawing can be ungrouped into its pieces
    let mut elements: Vec<(usize, Vec<Polyline>)> = Vec::new();
    let scale = unit_scale(bytes);
    walk(tree.root(), height, scale, &mut elements);
    let mut shapes = Vec::new();
    for (layer, paths) in elements {
        let paths: Vec<Polyline> = paths.into_iter().map(Polyline::cleaned).filter(|p| p.pts.len() > 1).collect();
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
        shapes.push(Shape::new(0, Geometry::Path { subpaths: sub }, layer, c[0], c[1]));
    }
    if shapes.is_empty() {
        return Err(tr("Il file SVG non contiene forme utilizzabili.").into());
    }
    Ok(shapes)
}

fn walk(g: &usvg::Group, height: f64, scale: f64, out: &mut Vec<(usize, Vec<Polyline>)>) {
    for node in g.children() {
        match node {
            usvg::Node::Group(child) => walk(child, height, scale, out),
            usvg::Node::Path(p) => {
                let color = p
                    .stroke()
                    .and_then(|s| match s.paint() {
                        usvg::Paint::Color(c) => Some(*c),
                        _ => None,
                    })
                    .or_else(|| {
                        p.fill().and_then(|f| match f.paint() {
                            usvg::Paint::Color(c) => Some(*c),
                            _ => None,
                        })
                    })
                    .unwrap_or(usvg::Color::black());
                let layer = nearest_layer(color.red, color.green, color.blue);
                let ts = p.abs_transform();
                let map = |x: f32, y: f32| -> [f64; 2] {
                    let mut pt = usvg::tiny_skia_path::Point::from_xy(x, y);
                    ts.map_point(&mut pt);
                    [pt.x as f64 * scale, (height - pt.y as f64) * scale]
                };
                let mut polys: Vec<Polyline> = Vec::new();
                let mut cur: Vec<[f64; 2]> = Vec::new();
                let mut last = [0.0f32, 0.0f32];
                let mut start = [0.0f32, 0.0f32];
                for seg in p.data().segments() {
                    use usvg::tiny_skia_path::PathSegment as S;
                    match seg {
                        S::MoveTo(pt) => {
                            if cur.len() > 1 {
                                polys.push(Polyline { pts: std::mem::take(&mut cur), closed: false });
                            }
                            cur.clear();
                            cur.push(map(pt.x, pt.y));
                            last = [pt.x, pt.y];
                            start = last;
                        }
                        S::LineTo(pt) => {
                            cur.push(map(pt.x, pt.y));
                            last = [pt.x, pt.y];
                        }
                        S::QuadTo(c, e) => {
                            for i in 1..=12 {
                                let t = i as f32 / 12.0;
                                let u = 1.0 - t;
                                let x = u * u * last[0] + 2.0 * u * t * c.x + t * t * e.x;
                                let y = u * u * last[1] + 2.0 * u * t * c.y + t * t * e.y;
                                cur.push(map(x, y));
                            }
                            last = [e.x, e.y];
                        }
                        S::CubicTo(c1, c2, e) => {
                            for i in 1..=16 {
                                let t = i as f32 / 16.0;
                                let u = 1.0 - t;
                                let x = u * u * u * last[0] + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * e.x;
                                let y = u * u * u * last[1] + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * e.y;
                                cur.push(map(x, y));
                            }
                            last = [e.x, e.y];
                        }
                        S::Close => {
                            if cur.len() > 1 {
                                let f = cur[0];
                                let l = *cur.last().unwrap();
                                if (f[0] - l[0]).abs() < 1e-9 && (f[1] - l[1]).abs() < 1e-9 {
                                    cur.pop();
                                }
                                polys.push(Polyline { pts: std::mem::take(&mut cur), closed: true });
                            }
                            cur.clear();
                            last = start;
                            cur.push(map(start[0], start[1]));
                        }
                    }
                }
                if cur.len() > 1 {
                    polys.push(Polyline { pts: cur, closed: false });
                }
                out.push((layer, polys));
            }
            _ => {}
        }
    }
}

/// Import a bitmap as a grayscale image shape. Images are placed at 96 DPI (estimated).
pub fn import_image(path: &std::path::Path) -> Result<Shape, String> {
    let img = image::open(path).map_err(|e| trf("Impossibile aprire l'immagine: {}", &[&e]))?;
    let rgba = img.to_rgba8();
    let (mut w, mut h) = rgba.dimensions();
    let mut rgba = rgba;
    let max_side = 1600;
    if w.max(h) > max_side {
        let k = max_side as f64 / w.max(h) as f64;
        let nw = ((w as f64 * k).round() as u32).max(1);
        let nh = ((h as f64 * k).round() as u32).max(1);
        rgba = image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle);
        w = nw;
        h = nh;
    }
    let mut gray = Vec::with_capacity((w * h) as usize);
    for p in rgba.pixels() {
        let a = p[3] as f64 / 255.0;
        let l = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        gray.push((l * a + 255.0 * (1.0 - a)).round().clamp(0.0, 255.0) as u8);
    }
    let (ow, _) = img.to_rgba8().dimensions();
    let mm_per_px = 25.4 / 96.0 * (ow as f64 / w as f64);
    let wmm = w as f64 * mm_per_px;
    let hmm = h as f64 * mm_per_px;
    Ok(Shape::new(0, Geometry::Image { w: wmm, h: hmm, px_w: w, px_h: h, gray }, 0, wmm / 2.0 + 10.0, hmm / 2.0 + 10.0))
}

#[cfg(test)]
mod tests {
    use crate::model::Geometry;

    fn svg(body: &str) -> String {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"100\" viewBox=\"0 0 100 100\">{body}</svg>")
    }

    fn points(s: &crate::model::Shape) -> Vec<Vec<[f64; 2]>> {
        match &s.geom {
            Geometry::Path { subpaths } => subpaths.iter().map(|p| p.pts.clone()).collect(),
            _ => vec![],
        }
    }

    #[test]
    fn every_element_kind_is_kept() {
        for body in [
            r#"<path d="M10 10 H 50" stroke="red" fill="none"/>"#,
            r#"<line x1="10" y1="10" x2="50" y2="10" stroke="red"/>"#,
            r#"<path d="M10 10 L 50 10 L 50 40" stroke="none" fill="none"/>"#,
            r#"<polyline points="10,10 50,10 50,40" stroke="red" fill="none"/>"#,
            r#"<g transform="translate(5,5) scale(2)"><path d="M1 1 h10" stroke="red" fill="none"/></g>"#,
        ] {
            let shapes = super::import_svg(svg(body).as_bytes()).unwrap();
            assert_eq!(shapes.len(), 1, "{body}");
        }
    }

    #[test]
    fn near_duplicate_closing_point_is_removed() {
        // closes by repeating the start with a rounding difference, then Z
        let s = super::import_svg(svg(r#"<path d="M10 10 L50 10 L50 40 L10 40 L10.000001 10.000001 Z" stroke="red" fill="none"/>"#).as_bytes()).unwrap();
        let p = points(&s[0]);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].len(), 4);
    }

    #[test]
    fn illustrator_files_without_units_are_72_dpi() {
        let ai = "<?xml version=\"1.0\"?><!-- Generator: Adobe Illustrator 27.0 --><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"72\" height=\"72\" viewBox=\"0 0 72 72\"><path d=\"M0 10 H72\" stroke=\"red\"/></svg>";
        let s = super::import_svg(ai.as_bytes()).unwrap();
        assert!((s[0].bbox().w() - 25.4).abs() < 1e-3);
        let mm = "<!-- Generator: Adobe Illustrator --><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"72mm\" height=\"72mm\" viewBox=\"0 0 72 72\"><path d=\"M0 10 H72\" stroke=\"red\"/></svg>";
        let s = super::import_svg(mm.as_bytes()).unwrap();
        assert!((s[0].bbox().w() - 72.0).abs() < 1e-3);
    }

    #[test]
    fn each_element_becomes_its_own_shape() {
        let s = super::import_svg(svg(r#"<rect x="5" y="5" width="10" height="10" stroke="red" fill="none"/><rect x="30" y="5" width="10" height="10" stroke="red" fill="none"/>"#).as_bytes()).unwrap();
        assert_eq!(s.len(), 2);
    }
}
