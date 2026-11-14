//! Text to outlines using TrueType fonts (built-in fonts plus system fonts on macOS).

use crate::model::{P2, Polyline};
use std::sync::{Arc, OnceLock};

pub struct FontEntry {
    pub name: String,
    pub data: Arc<Vec<u8>>,
}

pub fn fonts() -> &'static Vec<FontEntry> {
    static FONTS: OnceLock<Vec<FontEntry>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut v = vec![
            FontEntry { name: "Sans (built-in)".into(), data: Arc::new(epaint_default_fonts::UBUNTU_LIGHT.to_vec()) },
            FontEntry { name: "Mono (built-in)".into(), data: Arc::new(epaint_default_fonts::HACK_REGULAR.to_vec()) },
        ];
        let dirs = ["/System/Library/Fonts/Supplemental", "/Library/Fonts", "/usr/share/fonts/truetype", "C:\\Windows\\Fonts"];
        let mut found: Vec<(String, std::path::PathBuf)> = Vec::new();
        for d in dirs {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if ext == "ttf" || ext == "otf" {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            found.push((stem.to_string(), p.clone()));
                        }
                    }
                }
            }
        }
        found.sort();
        found.dedup_by(|a, b| a.0 == b.0);
        for (name, path) in found.into_iter().take(200) {
            if let Ok(bytes) = std::fs::read(&path) {
                if ttf_parser::Face::parse(&bytes, 0).is_ok() {
                    v.push(FontEntry { name, data: Arc::new(bytes) });
                }
            }
        }
        v
    })
}

struct Builder {
    paths: Vec<Polyline>,
    cur: Vec<P2>,
    last: P2,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl Builder {
    fn pt(&self, x: f32, y: f32) -> P2 {
        [self.ox + x as f64 * self.scale, self.oy + y as f64 * self.scale]
    }
    fn flush(&mut self, closed: bool) {
        if self.cur.len() > 1 {
            let mut pts = std::mem::take(&mut self.cur);
            if closed && pts.len() > 2 {
                let f = pts[0];
                let l = *pts.last().unwrap();
                if (f[0] - l[0]).abs() < 1e-9 && (f[1] - l[1]).abs() < 1e-9 {
                    pts.pop();
                }
            }
            self.paths.push(Polyline { pts, closed });
        }
        self.cur.clear();
    }
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush(true);
        let p = self.pt(x, y);
        self.cur.push(p);
        self.last = p;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cur.push(p);
        self.last = p;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let c = self.pt(x1, y1);
        let e = self.pt(x, y);
        let s = self.last;
        // "Qualità della curva" (Impostazioni > Schermo): pieces per curve
        let n = crate::prefs::curve_segments(8);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            self.cur.push([u * u * s[0] + 2.0 * u * t * c[0] + t * t * e[0], u * u * s[1] + 2.0 * u * t * c[1] + t * t * e[1]]);
        }
        self.last = e;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.pt(x1, y1);
        let c2 = self.pt(x2, y2);
        let e = self.pt(x, y);
        let s = self.last;
        let n = crate::prefs::curve_segments(10);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let a = u * u * u;
            let b = 3.0 * u * u * t;
            let cc = 3.0 * u * t * t;
            let d = t * t * t;
            self.cur.push([a * s[0] + b * c1[0] + cc * c2[0] + d * e[0], a * s[1] + b * c1[1] + cc * c2[1] + d * e[1]]);
        }
        self.last = e;
    }
    fn close(&mut self) {
        self.flush(true);
    }
}

/// Fonts draw some letters (a "1", accented letters) from pieces that overlap; filled even-odd,
/// the overlap would come out empty. Merge one glyph's pieces by the font's own non-zero rule
/// into clean outlines and holes.
fn merge_glyph(paths: &mut Vec<Polyline>, first: usize) {
    use i_overlay::core::fill_rule::FillRule;
    use i_overlay::float::simplify::SimplifyShape;
    if paths.len() - first < 2 {
        return;
    }
    let contours: Vec<Vec<[f64; 2]>> = paths[first..].iter().filter(|p| p.closed && p.pts.len() >= 3).map(|p| p.pts.clone()).collect();
    if contours.len() != paths.len() - first {
        return;
    }
    let merged = contours.simplify_shape(FillRule::NonZero);
    if merged.is_empty() {
        return;
    }
    paths.truncate(first);
    for shape in merged {
        for c in shape {
            paths.push(Polyline { pts: c, closed: true });
        }
    }
}

/// Letters of the scripts written right to left.
fn is_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

/// Build outlines (mm, Y up) centred on the origin. `size` is the cap height-ish em size in mm.
pub fn text_outline(text: &str, size: f64, font_name: &str) -> Vec<Polyline> {
    let list = fonts();
    let entry = list.iter().find(|f| f.name == font_name).unwrap_or(&list[0]);
    let Ok(face) = ttf_parser::Face::parse(&entry.data, 0) else { return Vec::new() };
    let upem = face.units_per_em() as f64;
    let scale = size / upem;
    let line_h = (face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) * scale;
    let mut b = Builder { paths: Vec::new(), cur: Vec::new(), last: [0.0, 0.0], scale, ox: 0.0, oy: 0.0 };
    let rtl = crate::prefs::with(|p| p.right_to_left_text);
    for (li, line) in text.split('\n').enumerate() {
        let mut x = 0.0;
        let y = -(li as f64) * line_h;
        // "Abilita il supporto del testo da destra a sinistra": a line written in a right-to-left
        // script (Hebrew, Arabic, Syriac, Thaana...) is laid out from its last letter, so it reads
        // right to left
        let chars: Vec<char> = if rtl && line.chars().any(is_rtl) { line.chars().rev().collect() } else { line.chars().collect() };
        for ch in chars {
            let Some(gid) = face.glyph_index(ch) else {
                x += size * 0.5;
                continue;
            };
            b.ox = x;
            b.oy = y;
            let first = b.paths.len();
            face.outline_glyph(gid, &mut b);
            b.flush(true);
            merge_glyph(&mut b.paths, first);
            x += face.glyph_hor_advance(gid).unwrap_or(0) as f64 * scale;
        }
    }
    // centre
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in b.paths.iter().flat_map(|p| p.pts.iter()) {
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[1]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[1]);
    }
    if !min[0].is_finite() {
        return Vec::new();
    }
    let c = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    b.paths
        .into_iter()
        .map(|pl| Polyline { pts: pl.pts.into_iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect(), closed: pl.closed })
        .collect()
}
