//! Laser Tools → Test intervallo / Test materiale. Both build their own small job (not added to
//! the project) that can be previewed, framed, started or saved as G-code.
//! Rules from IntervalTest (showEvent 0x100d3860c, Generate 0x100d389ac, accepted 0x100d387cc)
//! and MaterialTest (showEvent 0x100d3fc18, Generate 0x100d408f8, SetCutParam 0x100d42988).
//! See MECHANICS.md, "Test intervallo" and "Test materiale".

use crate::model::*;

// ---------------------------------------------------------------- interval test

/// Defaults set in IntervalTest::showEvent 0x100d3860c (speed 70 mm/s shown as mm/min).
pub const IT_SPEED_MM_MIN: f64 = 4200.0;
pub const IT_POWER: f64 = 20.0;
pub const IT_STEPS: u32 = 10;
pub const IT_MIN_INTERVAL: f64 = 0.07;
pub const IT_MAX_INTERVAL: f64 = 0.15;
pub const IT_SIZE: f64 = 15.0;
/// Squares per row; rows are centred on the bed (Generate: i / 5, i % 5).
pub const IT_PER_ROW: u32 = 5;
/// Label height as a share of the square (0x3f19999a in Generate; placement estimated).
pub const IT_LABEL_SCALE: f64 = 0.6;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IntervalTest {
    pub speed: f64,
    pub power: f64,
    pub steps: u32,
    pub min_interval: f64,
    pub max_interval: f64,
    pub size: f64,
    pub dithered: bool,
}

impl Default for IntervalTest {
    fn default() -> Self {
        Self { speed: IT_SPEED_MM_MIN, power: IT_POWER, steps: IT_STEPS, min_interval: IT_MIN_INTERVAL, max_interval: IT_MAX_INTERVAL, size: IT_SIZE, dithered: false }
    }
}

impl IntervalTest {
    pub fn interval_at(&self, i: u32) -> f64 {
        if self.steps <= 1 {
            return self.min_interval;
        }
        self.min_interval + i as f64 * (self.max_interval - self.min_interval) / (self.steps - 1) as f64
    }

    /// One filled square per step, its interval written beside it turned 90°, on a pitch of two
    /// squares, five per row, centred on the bed.
    pub fn build(&self, base: &Document) -> Document {
        let mut doc = test_doc(base);
        let s = self.size.max(1.0);
        let n = self.steps.max(1);
        let rows = n.div_ceil(IT_PER_ROW);
        let cols = n.min(IT_PER_ROW);
        let cx = base.device.width * 0.5;
        let cy = base.device.height * 0.5;
        let x0 = cx - (cols as f64 - 1.0) * s;
        let y0 = cy + (rows as f64 - 1.0) * s;
        for i in 0..n {
            let (col, row) = (i % IT_PER_ROW, i / IT_PER_ROW);
            let x = x0 + col as f64 * 2.0 * s;
            let y = y0 - row as f64 * 2.0 * s;
            let interval = self.interval_at(i);
            let mut cs = CutSetting::default();
            cs.name = format!("{:.3} mm", interval);
            cs.speed = self.speed;
            cs.max_power = self.power;
            cs.min_power = self.power;
            cs.interval = interval;
            cs.mode = if self.dithered { CutMode::Image } else { CutMode::Fill };
            cs.image_mode = ImageMode::Dithered;
            let layer = push_layer(&mut doc, cs);
            if self.dithered {
                // a left-to-right grey ramp, one pixel per scan line
                let px = ((s / interval).round() as u32).clamp(2, 2000);
                let gray: Vec<u8> = (0..px * px).map(|k| ((k % px) as f64 / (px - 1) as f64 * 255.0) as u8).collect();
                doc.add_shape(Shape::new(0, Geometry::Image { w: s, h: s, px_w: px, px_h: px, gray }, layer, x, y));
            } else {
                doc.add_shape(Shape::new(0, Geometry::Rect { w: s, h: s, radius: 0.0 }, layer, x, y));
            }
            // "%1.2f" in mm (0x100d389ac); engraved with the square's own setting
            let text = format!("{:.2}", interval);
            add_label(&mut doc, &text, s * IT_LABEL_SCALE * 0.5, layer, [x - s * 0.5 - s * 0.35, y], 90.0);
        }
        doc
    }
}

// ---------------------------------------------------------------- material test

/// What a row or column varies (MaterialTest::SetCutParam 0x100d42988).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MtParam {
    Speed = 0,
    Power = 1,
    Interval = 2,
    Passes = 3,
    Frequency = 4,
}

impl MtParam {
    pub fn label(self) -> &'static str {
        crate::i18n::tr(match self {
            MtParam::Speed => "Velocità",
            MtParam::Power => "Potenza",
            MtParam::Interval => "Intervallo",
            MtParam::Passes => "Passate",
            MtParam::Frequency => "Frequenza",
        })
    }

    pub fn unit(self) -> &'static str {
        match self {
            MtParam::Speed => "mm/min",
            MtParam::Power => "%",
            MtParam::Interval => "mm",
            MtParam::Passes => "",
            MtParam::Frequency => "kHz",
        }
    }

/// Decimals each parameter is written with (speed 0, power 1, interval 3, passes 0, kHz 1).
    pub fn decimals(self) -> usize {
        match self {
            MtParam::Speed | MtParam::Passes => 0,
            MtParam::Power | MtParam::Frequency => 1,
            MtParam::Interval => 3,
        }
    }

    /// A value as the test writes it: QLocale::toString(v, 'f', decimals) in the system's
    /// number style — decimal comma or point, thousands grouped from 10 000 up ("18.000", "8333").
    pub fn fmt(self, v: f64) -> String {
        let dec = crate::i18n::decimal_char();
        let group = if dec == ',' { '.' } else { ',' };
        let s = format!("{:.*}", self.decimals(), v.abs());
        let (int, frac) = s.split_once('.').map(|(a, b)| (a.to_string(), Some(b.to_string()))).unwrap_or((s.clone(), None));
        let mut grouped = String::new();
        if int.len() >= 5 {
            for (i, ch) in int.chars().enumerate() {
                if i > 0 && (int.len() - i) % 3 == 0 {
                    grouped.push(group);
                }
                grouped.push(ch);
            }
        } else {
            grouped = int;
        }
        let mut out = if v < 0.0 { format!("-{grouped}") } else { grouped };
        if let Some(f) = frac {
            out.push(dec);
            out.push_str(&f);
        }
        out
    }

    /// Put a value into a cut setting: power sets max and min, frequency in kHz × 1000, passes
    /// truncated (SetCutParam 0x100d42988).
    pub fn apply(self, cs: &mut CutSetting, v: f64) {
        match self {
            MtParam::Speed => cs.speed = v,
            MtParam::Power => {
                cs.max_power = v;
                cs.min_power = v;
            }
            MtParam::Interval => cs.interval = v,
            MtParam::Passes => cs.passes = (v as u32).max(1),
            MtParam::Frequency => cs.frequency = (v * MT_KHZ_TO_HZ) as u32,
        }
    }
}

pub const MT_KHZ_TO_HZ: f64 = 1000.0;
/// Rows' speed range the person's pictures of the original show (600 … 18 000 mm/min).
pub const MT_DEFAULT_SPEED_MIN: f64 = 600.0;
pub const MT_DEFAULT_SPEED_MAX: f64 = 18000.0;
/// Space between cells, mm (Generate 0x100d408f8: 1.5, or 0.3 on galvo machines).
pub const MT_GAP: f64 = 1.5;
/// Labels are 0.75 × the smaller of cell width, cell height and 10 mm (Generate 0x100d408f8).
pub const MT_LABEL_SCALE: f64 = 0.75;
pub const MT_LABEL_MAX: f64 = 10.0;
/// The border is the test's bounds grown by 3 mm on every side (Rect2::Expand(3.0)).
pub const MT_BORDER_MARGIN: f64 = 3.0;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Axis {
    pub count: u32,
    pub param: MtParam,
    pub min: f64,
    pub max: f64,
    /// cell width (columns) or height (rows), mm
    pub size: f64,
    /// centre of the test on this axis, mm
    pub center: f64,
}

impl Axis {
    pub fn value_at(&self, i: u32) -> f64 {
        if self.count <= 1 {
            return self.min;
        }
        self.min + i as f64 * (self.max - self.min) / (self.count - 1) as f64
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MaterialTest {
    pub title: String,
    /// Vertical / rows
    pub rows: Axis,
    /// Horizontal / columns
    pub cols: Axis,
    /// 00 material, 01 text, 02 border (Modifica impostazione …)
    pub material: CutSetting,
    pub text: CutSetting,
    pub border: CutSetting,
    pub enable_text: bool,
    pub enable_border: bool,
}

impl MaterialTest {
    pub fn new(device: &Device) -> Self {
        let mut material = CutSetting::default();
        material.name = "Materiale".into();
        material.mode = CutMode::Fill;
        let mut text = CutSetting::default();
        text.name = "Testo".into();
        text.mode = CutMode::Fill;
        text.speed = 6000.0;
        text.max_power = 20.0;
        text.min_power = 20.0;
        let mut border = CutSetting::default();
        border.name = "Bordo".into();
        border.mode = CutMode::Line;
        border.speed = 1200.0;
        border.max_power = 30.0;
        border.min_power = 30.0;
        // the person's picture: rows vary speed up to 18000 mm/min, 10 columns of power 10–100 %;
        // centre = half the bed (showEvent 0x100d3fc18)
        Self {
            title: String::new(),
            rows: Axis { count: 10, param: MtParam::Speed, min: MT_DEFAULT_SPEED_MIN, max: MT_DEFAULT_SPEED_MAX, size: 5.0, center: device.height * 0.5 },
            cols: Axis { count: 10, param: MtParam::Power, min: 10.0, max: 100.0, size: 5.0, center: device.width * 0.5 },
            material,
            text,
            border,
            enable_text: true,
            enable_border: true,
        }
    }

    /// Built-in presets (Impostazioni predefinite integrate). Their numbers live in a compressed
    /// resource of the original that wasn't read: these values are estimates.
    pub fn preset(&mut self, which: usize) {
        let (rows, cols, mode) = match which {
            // CO2 - Cut test
            0 => ((MtParam::Speed, 300.0, 1800.0), (MtParam::Power, 30.0, 90.0), CutMode::Line),
            // CO2 - Engrave test
            1 => ((MtParam::Speed, 6000.0, 30000.0), (MtParam::Power, 10.0, 60.0), CutMode::Fill),
            // Diode - Cut test
            2 => ((MtParam::Speed, 60.0, 600.0), (MtParam::Power, 50.0, 100.0), CutMode::Line),
            // Diode - Engrave test
            _ => ((MtParam::Speed, MT_DEFAULT_SPEED_MIN, MT_DEFAULT_SPEED_MAX), (MtParam::Power, 10.0, 100.0), CutMode::Fill),
        };
        self.rows.param = rows.0;
        self.rows.min = rows.1;
        self.rows.max = rows.2;
        self.cols.param = cols.0;
        self.cols.min = cols.1;
        self.cols.max = cols.2;
        self.material.mode = mode;
    }

    /// Size of the finished test (cells, labels and border), mm.
    pub fn output_size(&self, base: &Document) -> (f64, f64) {
        let d = self.build(base);
        let b = d.bbox_all();
        if b.is_valid() { (b.w(), b.h()) } else { (0.0, 0.0) }
    }

    /// The grid: column c, row r gets the material setting with the column's and the row's
    /// value; labels above and to the left; a border around it all; centred on Centro X / Y.
    pub fn build(&self, base: &Document) -> Document {
        let mut doc = test_doc(base);
        let (cw, ch) = (self.cols.size.max(0.5), self.rows.size.max(0.5));
        let (nc, nr) = (self.cols.count.max(1), self.rows.count.max(1));
        let label_h = MT_LABEL_SCALE * cw.min(ch).min(MT_LABEL_MAX);
        let text_layer = 1;
        let border_layer = 2;
        doc.layers[text_layer] = CutSetting { name: self.text.name.clone(), ..self.text.clone() };
        doc.layers[border_layer] = CutSetting { name: self.border.name.clone(), ..self.border.clone() };
        // cells, built with the grid's top-left at (0, 0); the top row holds the rows' largest
        // value and the bottom row the smallest, as the original's preview shows
        let row_value = |r: u32| self.rows.value_at(nr - 1 - r);
        for r in 0..nr {
            for c in 0..nc {
                let mut cs = self.material.clone();
                self.cols.param.apply(&mut cs, self.cols.value_at(c));
                self.rows.param.apply(&mut cs, row_value(r));
                cs.name = format!("{} {} / {} {}", self.rows.param.fmt(row_value(r)), self.rows.param.unit(), self.cols.param.fmt(self.cols.value_at(c)), self.cols.param.unit());
                let layer = push_layer(&mut doc, cs);
                let x = c as f64 * (cw + MT_GAP) + cw * 0.5;
                let y = -(r as f64 * (ch + MT_GAP) + ch * 0.5);
                doc.add_shape(Shape::new(0, Geometry::Rect { w: cw, h: ch, radius: 0.0 }, layer, x, y));
            }
        }
        if self.enable_text {
            let grid_w = nc as f64 * (cw + MT_GAP) - MT_GAP;
            let grid_h = nr as f64 * (ch + MT_GAP) - MT_GAP;
            // row values to the left of each row
            for r in 0..nr {
                let y = -(r as f64 * (ch + MT_GAP) + ch * 0.5);
                add_label_anchored(&mut doc, &self.rows.param.fmt(row_value(r)), label_h, text_layer, [-MT_GAP, y], 0.0, Anchor::End);
            }
            // column values under each column, turned 90° to read upwards (SetRotation(±π/2))
            for c in 0..nc {
                let x = c as f64 * (cw + MT_GAP) + cw * 0.5;
                add_label_anchored(&mut doc, &self.cols.param.fmt(self.cols.value_at(c)), label_h, text_layer, [x, -grid_h - MT_GAP], 90.0, Anchor::End);
            }
            // the columns' name under their values, the rows' name turned 90° to their left
            let bottom = doc.bbox_all().min[1];
            add_label_anchored(&mut doc, &axis_title(&self.cols), label_h, text_layer, [grid_w * 0.5, bottom - MT_GAP * 2.0 - label_h * 0.5], 0.0, Anchor::Middle);
            let left = doc.bbox_all().min[0];
            add_label_anchored(&mut doc, &axis_title(&self.rows), label_h, text_layer, [left - MT_GAP - label_h * 0.5, -grid_h * 0.5], 90.0, Anchor::Middle);
            // above: the title, then every setting that stays the same in all cells
            let mut lines: Vec<String> = Vec::new();
            if !self.title.trim().is_empty() {
                lines.push(self.title.trim().to_string());
            }
            for p in [MtParam::Speed, MtParam::Power, MtParam::Interval, MtParam::Passes] {
                if p == self.rows.param || p == self.cols.param {
                    continue;
                }
                // speed and power only matter as fixed values for a fill; interval only for fills
                let v = match p {
                    MtParam::Speed => self.material.speed,
                    MtParam::Power => self.material.max_power,
                    MtParam::Interval if self.material.mode == CutMode::Line => continue,
                    MtParam::Interval => self.material.interval,
                    MtParam::Passes => self.material.passes as f64,
                    MtParam::Frequency => continue,
                };
                let unit = if p.unit().is_empty() { String::new() } else { format!(" {}", p.unit()) };
                lines.push(format!("{}: {}{}", p.label(), p.fmt(v), unit));
            }
            let mut y = MT_GAP * 2.0 + label_h * 0.5 + (lines.len().max(1) - 1) as f64 * label_h * 1.3;
            for l in &lines {
                add_label_anchored(&mut doc, l, label_h, text_layer, [grid_w * 0.5, y], 0.0, Anchor::Middle);
                y -= label_h * 1.3;
            }
        }
        if self.enable_border {
            let mut b = doc.bbox_all();
            b.min[0] -= MT_BORDER_MARGIN;
            b.min[1] -= MT_BORDER_MARGIN;
            b.max[0] += MT_BORDER_MARGIN;
            b.max[1] += MT_BORDER_MARGIN;
            let c = b.center();
            doc.add_shape(Shape::new(0, Geometry::Rect { w: b.w(), h: b.h(), radius: 0.0 }, border_layer, c[0], c[1]));
        }
        // move the whole test so its middle sits on Centro X / Centro Y
        let c = doc.bbox_all().center();
        let (dx, dy) = (self.cols.center - c[0], self.rows.center - c[1]);
        for s in doc.shapes.iter_mut() {
            s.tx += dx;
            s.ty += dy;
        }
        doc
    }
}

fn axis_title(a: &Axis) -> String {
    if a.param.unit().is_empty() { a.param.label().to_string() } else { format!("{} ({})", a.param.label(), a.param.unit()) }
}

// ---------------------------------------------------------------- helpers

/// An empty document with the project's device and palette layers.
fn test_doc(base: &Document) -> Document {
    let mut d = Document::default();
    d.device = base.device.clone();
    d.layers = base.layers.clone();
    d.layers.truncate(LAYER_COUNT);
    d
}

fn push_layer(doc: &mut Document, mut cs: CutSetting) -> usize {
    cs.output = true;
    cs.show = true;
    doc.layers.push(cs);
    doc.layers.len() - 1
}

#[derive(Clone, Copy)]
enum Anchor {
    Start,
    Middle,
    End,
}

fn add_label(doc: &mut Document, text: &str, height: f64, layer: usize, at: P2, rot: f64) {
    add_label_anchored(doc, text, height, layer, at, rot, Anchor::Middle);
}

/// Text outline `height` mm tall, turned `rot` degrees, placed so its start / middle / end (along
/// the reading direction) sits on `at`, centred across it.
fn add_label_anchored(doc: &mut Document, text: &str, height: f64, layer: usize, at: P2, rot: f64, anchor: Anchor) {
    let font = crate::text::fonts()[0].name.clone();
    let outline = crate::text::text_outline(text, height, &font);
    if outline.is_empty() {
        return;
    }
    let mut b = BBox::empty();
    for pl in &outline {
        for p in &pl.pts {
            b.add(*p);
        }
    }
    let along = match anchor {
        Anchor::Start => b.min[0],
        Anchor::Middle => b.center()[0],
        Anchor::End => b.max[0],
    };
    let local = [along, b.center()[1]];
    let mut s = Shape::new(0, Geometry::Text { text: text.into(), size: height, font, outline }, layer, 0.0, 0.0);
    s.rot = rot;
    let w = s.to_world(local);
    s.tx = at[0] - w[0];
    s.ty = at[1] - w[1];
    doc.add_shape(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_steps_are_even() {
        let t = IntervalTest::default();
        assert!((t.interval_at(0) - 0.07).abs() < 1e-9);
        assert!((t.interval_at(9) - 0.15).abs() < 1e-9);
        let d = t.build(&Document::default());
        // ten squares, each on its own setting, plus their labels
        assert_eq!(d.layers.len(), LAYER_COUNT + 10);
        assert!(d.shapes.iter().filter(|s| matches!(s.geom, Geometry::Rect { .. })).count() == 10);
    }

    #[test]
    fn material_grid_is_centred() {
        let doc = Document::default();
        let t = MaterialTest::new(&doc.device);
        let d = t.build(&doc);
        assert_eq!(d.layers.len(), LAYER_COUNT + 100);
        let b = d.bbox_all();
        assert!((b.center()[0] - doc.device.width * 0.5).abs() < 1e-6);
        assert!((b.center()[1] - doc.device.height * 0.5).abs() < 1e-6);
        // top-left cell: 10 % at the fastest speed; bottom-right: 100 % at the slowest
        let first = &d.layers[LAYER_COUNT];
        let last = &d.layers[LAYER_COUNT + 99];
        assert_eq!((first.max_power, first.speed), (10.0, 18000.0));
        assert_eq!((last.max_power, last.speed), (100.0, 600.0));
    }

    #[test]
    fn numbers_like_the_original() {
        crate::i18n::set(crate::i18n::Lang::It);
        let dec = crate::i18n::decimal_char();
        if dec == ',' {
            assert_eq!(MtParam::Speed.fmt(18000.0), "18.000");
            assert_eq!(MtParam::Speed.fmt(8333.3), "8333");
            assert_eq!(MtParam::Power.fmt(10.0), "10,0");
        } else {
            assert_eq!(MtParam::Speed.fmt(18000.0), "18,000");
            assert_eq!(MtParam::Power.fmt(10.0), "10.0");
        }
    }
}
