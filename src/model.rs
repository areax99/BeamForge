//! Document model: shapes, layers (cut settings) and the device.
//! Default values come from MECHANICS.md.

use serde::{Deserialize, Serialize};

/// Layer palette: 30 cut layers plus the two tool layers T1/T2 (data 0x10245e9e8).
pub const PALETTE: [[u8; 3]; 32] = [
    [0x00, 0x00, 0x00], [0x00, 0x00, 0xff], [0xff, 0x00, 0x00], [0x00, 0xe0, 0x00],
    [0xd0, 0xd0, 0x00], [0xff, 0x80, 0x00], [0x00, 0xe0, 0xe0], [0xff, 0x00, 0xff],
    [0xb4, 0xb4, 0xb4], [0x00, 0x00, 0xa0], [0xa0, 0x00, 0x00], [0x00, 0xa0, 0x00],
    [0xa0, 0xa0, 0x00], [0xc0, 0x80, 0x00], [0x00, 0xa0, 0xff], [0xa0, 0x00, 0xa0],
    [0x80, 0x80, 0x80], [0x7d, 0x87, 0xb9], [0xbb, 0x77, 0x84], [0x4a, 0x6f, 0xe3],
    [0xd3, 0x3f, 0x6a], [0x8c, 0xd7, 0x8c], [0xf0, 0xb9, 0x8d], [0xf6, 0xc4, 0xe1],
    [0xfa, 0x9e, 0xd4], [0x50, 0x0a, 0x78], [0xb4, 0x5a, 0x00], [0x00, 0x47, 0x54],
    [0x86, 0xfa, 0x88], [0xff, 0xdb, 0x66], [0xf3, 0x69, 0x26], [0x0c, 0x96, 0xd9],
];
pub const LAYER_COUNT: usize = 32;
pub const CUT_LAYER_COUNT: usize = 30;

pub fn layer_name(i: usize) -> String {
    if i < CUT_LAYER_COUNT {
        format!("C{:02}", i)
    } else if i < LAYER_COUNT {
        format!("T{}", i - CUT_LAYER_COUNT + 1)
    } else {
        // the extra settings a material or interval test makes, one per cell
        format!("Test {}", i - LAYER_COUNT + 1)
    }
}

/// T1 and T2: drawn but never cut. Layers past the palette (test cells) are cut.
pub fn is_tool_layer(i: usize) -> bool {
    (CUT_LAYER_COUNT..LAYER_COUNT).contains(&i)
}

/// Nearest palette entry by squared RGB distance (like LB_GetColorIndex, 0x100c958a4).
pub fn nearest_layer(r: u8, g: u8, b: u8) -> usize {
    let mut best = 0;
    let mut best_d = i32::MAX;
    // "Consenti l'importazione sui livelli degli strumenti": colours may also land on T1 / T2
    let n = if crate::prefs::with(|p| p.import_on_tool_layers) { LAYER_COUNT } else { CUT_LAYER_COUNT };
    for (i, c) in PALETTE.iter().enumerate().take(n) {
        let dr = c[0] as i32 - r as i32;
        let dg = c[1] as i32 - g as i32;
        let db = c[2] as i32 - b as i32;
        let d = dr * dr + dg * dg + db * db;
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CutMode {
    Line,
    Fill,
    /// "Offset Fill" (CutSetting::TypeToName 0x1009ef5dc: type 2)
    OffsetFill,
    Image,
}

/// How a Fill layer groups its shapes (CUT_PARAMS scanOpt +0x94, WriteTo 0x1009df5f4:
/// 0 "individual", 1 "byGroup", 2 "mergeAll"; default 2).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FillGrouping {
    Individual,
    ByGroup,
    #[default]
    MergeAll,
}

impl CutMode {
    pub fn label(self) -> &'static str {
        match self {
            CutMode::Line => "Line",
            CutMode::Fill => "Fill",
            CutMode::OffsetFill => "Offset Fill",
            CutMode::Image => "Image",
        }
    }
}

/// What is drawn on a layer, which decides which cut modes can be chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerArt {
    /// Nothing on the layer yet: every mode stays available.
    Empty,
    /// Only pictures. The mode stays Image.
    Image,
    /// Only vectors. Image cannot be chosen.
    Vector,
    /// Pictures and vectors together. The mode stays as it is.
    Mixed,
}

impl LayerArt {
    /// Line, Fill and Offset Fill are for vectors. Image is for pictures.
    pub fn allows(self, mode: CutMode) -> bool {
        match self {
            LayerArt::Empty => true,
            LayerArt::Image => mode == CutMode::Image,
            LayerArt::Vector => mode != CutMode::Image,
            LayerArt::Mixed => false,
        }
    }

    /// The mode a layer of this kind is allowed to keep.
    pub fn clamp(self, mode: CutMode) -> CutMode {
        match self {
            LayerArt::Image => CutMode::Image,
            LayerArt::Vector if mode == CutMode::Image => CutMode::Line,
            _ => mode,
        }
    }
}

/// Image modes, in the order of the original menu (Soglia … Scala di grigi).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageMode {
    Threshold,
    Ordered,
    Atkinson,
    /// Floyd-Steinberg. The menu calls it "Dithering".
    Dithered,
    Stucki,
    Jarvis,
    /// "Carta di giornale".
    Newsprint,
    Halftone,
    Sketch,
    Grayscale,
    /// "Modalità punto" (dotmode): stops on each dot and fires at constant power for a time that
    /// follows the grey (DitherMode tooltip; RasterScanner::OutputDotMode 0x100273f0c).
    Dot,
    /// "Spaccato 3D" (3dslice): a grey relief cut in slices, one slice per pass (DitherMode 10,
    /// offered by CutSettingEdit::SetupDeviceControls 0x100b7f440 for some lasers only).
    Slice3D,
}

impl ImageMode {
    pub const ALL: [ImageMode; 12] = [
        ImageMode::Threshold,
        ImageMode::Ordered,
        ImageMode::Atkinson,
        ImageMode::Dithered,
        ImageMode::Stucki,
        ImageMode::Jarvis,
        ImageMode::Newsprint,
        ImageMode::Halftone,
        ImageMode::Sketch,
        ImageMode::Grayscale,
        ImageMode::Dot,
        ImageMode::Slice3D,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ImageMode::Threshold => "Threshold",
            ImageMode::Ordered => "Ordered",
            ImageMode::Atkinson => "Atkinson",
            ImageMode::Dithered => "Dither",
            ImageMode::Stucki => "Stucki",
            ImageMode::Jarvis => "Jarvis",
            ImageMode::Newsprint => "Newsprint",
            ImageMode::Halftone => "Halftone",
            ImageMode::Sketch => "Sketch",
            ImageMode::Grayscale => "Grayscale",
            ImageMode::Dot => "Dot Mode",
            ImageMode::Slice3D => "3D Sliced",
        }
    }
}

/// Default layer speed: 100 mm/s (CUT_PARAMS ctor 0x1009f4744) shown in mm/min.
pub const DEFAULT_SPEED_MM_MIN: f64 = 6000.0;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct CutSetting {
    pub name: String,
    pub mode: CutMode,
    /// mm/min
    pub speed: f64,
    /// percent
    pub max_power: f64,
    pub min_power: f64,
    pub passes: u32,
    /// mm between scan lines (Fill / Image)
    pub interval: f64,
    /// degrees
    pub angle: f64,
    pub bidir: bool,
    pub crosshatch: bool,
    pub overscan: bool,
    pub overscan_percent: f64,
    pub air: bool,
    pub output: bool,
    pub show: bool,
    pub image_mode: ImageMode,
    pub negative: bool,
    /// Spot-size compensation. The width itself is dotWidth (+0xa8, 0.08 mm); the switch starts off.
    pub dot_width_on: bool,
    pub dot_width: f64,
    /// Skip the dither and send the picture as varying power.
    pub pass_through: bool,
    /// Halftone screen density. The original Image window shows 50.
    pub cells_per_inch: f64,
    /// Halftone screen angle, degrees. The original Image window shows 22.5.
    pub halftone_angle: f64,
    /// Dot mode: the longest stop on a fully black dot, ms (dotTime; default estimated).
    pub dot_time_ms: f64,
    /// Dot mode: distance between dots along a line, mm (dotSpacing; default estimated).
    pub dot_spacing: f64,
    pub frequency: u32,
    // --- Common tab (Line mode) ---
    pub kerf_enabled: bool,
    /// mm, + grows outer shapes and shrinks holes
    pub kerf: f64,
    pub perforate: bool,
    /// mm of cut in perforation mode
    pub perf_len: f64,
    /// mm skipped in perforation mode
    pub perf_skip: f64,
    pub tabs_enabled: bool,
    pub manual_tabs: bool,
    /// mm
    pub tab_size: f64,
    /// true: even spacing; false: a fixed number of tabs per shape
    pub tabs_use_spacing: bool,
    /// mm
    pub tab_spacing: f64,
    pub tab_count: u32,
    pub limit_max_tabs: bool,
    pub tab_count_max: u32,
    pub tab_power_enabled: bool,
    /// percent of max power used across a tab
    pub tab_cut_power: f64,
    pub skip_inner_tabs: bool,
    // --- Advanced tab (Line mode) ---
    /// mm cut past the start of a closed shape
    pub overcut: f64,
    pub lead_in: bool,
    pub lead_in_len: f64,
    /// degrees between the lead and the path
    pub lead_in_angle: f64,
    pub lead_out: bool,
    pub lead_out_len: f64,
    pub lead_style: LeadStyle,
    // --- fill options (CUT_PARAMS::WriteTo 0x1009ded14) ---
    /// "subname" (sub-layer name)
    pub sub_name: String,
    /// "forceConstantPower" (+0x1c0, default off): the laser runs in constant power (M3)
    pub constant_power: bool,
    /// "zOffset" (+0x34), mm; only with a Z axis
    pub z_offset: f64,
    /// "zPerPass" (+0x40), mm; only with a Z axis
    pub z_per_pass: f64,
    /// "scanOpt" (+0x94)
    pub fill_grouping: FillGrouping,
    /// "rampLength" (+0x7c), mm
    pub ramp_length: f64,
    /// "rampOuter" (+0x80)
    pub ramp_outer: bool,
    /// "floodFill" (+0x92)
    pub flood_fill: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LeadStyle {
    #[default]
    Line,
    Arc,
}

impl Default for CutSetting {
    fn default() -> Self {
        // CUT_PARAMS::CUT_PARAMS() 0x1009f4744
        CutSetting {
            name: String::new(),
            mode: CutMode::Line,
            // 100 mm/s in the original (CUT_PARAMS ctor, +0x2c) = 6000 mm/min
            speed: DEFAULT_SPEED_MM_MIN,
            max_power: 20.0,
            min_power: 20.0,
            passes: 1,
            interval: 0.1,
            angle: 0.0,
            bidir: true,
            crosshatch: false,
            overscan: true,
            overscan_percent: 2.5,
            air: true,
            output: true,
            show: true,
            image_mode: ImageMode::Dithered,
            negative: false,
            dot_width_on: false,
            dot_width: 0.08,
            pass_through: false,
            cells_per_inch: 50.0,
            halftone_angle: 22.5,
            dot_time_ms: 1.0,
            dot_spacing: 0.1,
            frequency: 20000,
            // CutSetting::Reset 0x1009db52c / CUT_PARAMS ctor 0x1009f4744
            kerf_enabled: false,
            kerf: 0.0,
            perforate: false,
            perf_len: 0.1,
            perf_skip: 0.1,
            tabs_enabled: false,
            manual_tabs: false,
            tab_size: 0.5,
            tabs_use_spacing: true,
            tab_spacing: 50.0,
            tab_count: 1,
            limit_max_tabs: false,
            tab_count_max: 1,
            tab_power_enabled: false,
            tab_cut_power: 0.0,
            skip_inner_tabs: false,
            overcut: 0.0,
            lead_in: false,
            lead_in_len: 0.0,
            lead_in_angle: 0.0,
            lead_out: false,
            lead_out_len: 0.0,
            lead_style: LeadStyle::Arc,
            sub_name: String::new(),
            constant_power: false,
            z_offset: 0.0,
            z_per_pass: 0.0,
            fill_grouping: FillGrouping::MergeAll,
            ramp_length: 0.0,
            ramp_outer: false,
            flood_fill: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum OriginCorner {
    FrontLeft,
    RearLeft,
    FrontRight,
    RearRight,
}

impl OriginCorner {
    pub fn label(self) -> &'static str {
        match self {
            OriginCorner::FrontLeft => "Front left",
            OriginCorner::RearLeft => "Rear left",
            OriginCorner::FrontRight => "Front right",
            OriginCorner::RearRight => "Rear right",
        }
    }
}

impl Device {
    /// Whether the controller has a separate minimum power (CutSettingEdit::UpdatePowerControls
    /// 0x100b86830 asks the protocol; GRBL has only S, so no). RayEtch drives GRBL devices
    /// only, so the field is greyed out as in the original.
    pub fn supports_min_power(&self) -> bool {
        false
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Device {
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub s_max: f64,
    pub origin: OriginCorner,
    /// M4 dynamic power (GRBL) or M3 constant power (GRBL-M3)
    pub dynamic_power: bool,
    /// Device setting "AirAssistM7" (GetAirAssistOnOffCommand 0x10062af10): air on = M7 instead of M8
    pub air_assist_m7: bool,
    /// Fire button power, percent
    pub fire_power: f64,
    /// Setting "LaserFrame_Enable" (read by MainWindow::UpdateLaserFireButton 0x100c49e70):
    /// frame with the beam on. On by default at the person's request (original: off).
    pub laser_frame: bool,
    pub baud: u32,
    /// mm/s; MotionPlanner default 400 (0x100c0215c, constant 0x10193843c)
    pub rapid_speed: f64,
    /// mm/s²; MotionPlanner default 3000 (constant 0x10193843c), GRBL $120/$121
    pub accel: f64,
    /// Controller type shown in Periferiche (GRBL, GRBL-M3, grblHAL)
    pub controller: String,
    /// Serial port this laser uses; empty = pick it in the Laser panel
    pub port: String,
    /// Laser source type code 0..=9 (LaserSource enum, SourceDisplayName 0x100b21ec0,
    /// name table 0x100b6a320); 1 = Diode
    pub laser_source: u8,
    // --- Impostazioni della periferica (the person's pictures 9-12) ---
    /// "Coordinate della macchina": jobs in absolute coordinates go to the machine's own
    /// coordinates (G53 on each move) instead of the work coordinates (G54)
    pub machine_coords: bool,
    /// "Abilita offset puntatore": where the red-dot pointer sits from the beam, mm
    pub pointer_offset_on: bool,
    pub pointer_offset: [f64; 2],
    /// "Unità di controllo": speeds shown in mm/s instead of mm/min
    pub speed_mm_s: bool,
    /// "Collega le unità di controllo e di progettazione"
    pub link_units: bool,
    /// "Larghezza impulso linguetta (mm)"
    pub tab_pulse_width: f64,
    /// "Abilita la lista di controllo lavori" and its items, asked before each start
    pub job_checklist_on: bool,
    pub job_checklist: Vec<String>,
    /// "Abilita regolazione offset di scansione": per speed (mm/s), how far (mm) lines scanned
    /// right to left are moved to line up with those scanned left to right
    pub scan_offset_on: bool,
    pub scan_offsets: Vec<ScanOffset>,
    /// "Nome Laser 1"
    pub laser1_name: String,
    pub laser1_rf: bool,
    pub laser1_fiber: bool,
    /// "Abilita comandi Laser 2" and its options
    pub laser2_on: bool,
    pub laser2_name: String,
    pub one_laser_at_once: bool,
    pub laser2_rf: bool,
    pub laser2_fiber: bool,
    pub laser2_offset_on: bool,
    pub laser2_offset: [f64; 2],
    /// "Impostazioni della simulazione" (only the preview's timing): top speed per axis, mm/s
    pub max_speed: [f64; 2],
    /// acceleration for cutting and for engraving, per axis, mm/s²
    pub cut_accel: [f64; 2],
    pub engrave_accel: [f64; 2],
    /// "Precisione angolo" (mm) and "Velocità d'angolo minima" (mm/s)
    pub corner_precision: f64,
    pub min_corner_speed: f64,
    /// "Scala velocità taglio" and "Scala della velocità globale", percent
    pub cut_speed_scale: f64,
    pub global_speed_scale: f64,
    /// "Abilita porta di accesso API permanente" (Estensioni)
    pub api_port_on: bool,
    pub api_port: u16,
}

/// One row of the scan offset table.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct ScanOffset {
    /// mm/s
    pub speed: f64,
    /// "Spostamento linea", mm
    pub shift: f64,
    /// "Offset iniziale", mm
    pub start: f64,
    /// false: Laser 1, true: Laser 2
    pub laser2: bool,
}

impl Device {
    /// "Regolazione offset di scansione": how far a right-to-left line is moved at `speed`
    /// (mm/min), from the table by straight interpolation between its rows (ends held).
    pub fn scan_shift(&self, speed_mm_min: f64) -> f64 {
        if !self.scan_offset_on {
            return 0.0;
        }
        let v = speed_mm_min / 60.0;
        let mut rows: Vec<&ScanOffset> = self.scan_offsets.iter().filter(|r| !r.laser2).collect();
        rows.sort_by(|a, b| a.speed.total_cmp(&b.speed));
        match rows.as_slice() {
            [] => 0.0,
            [r] => r.shift + r.start,
            _ => {
                if v <= rows[0].speed {
                    return rows[0].shift + rows[0].start;
                }
                for w in rows.windows(2) {
                    if v <= w[1].speed {
                        let t = (v - w[0].speed) / (w[1].speed - w[0].speed).max(1e-9);
                        return (w[0].shift + w[0].start) + t * ((w[1].shift + w[1].start) - (w[0].shift + w[0].start));
                    }
                }
                let r = rows[rows.len() - 1];
                r.shift + r.start
            }
        }
    }
}

/// Laser source codes and display names, in the order of SourceDisplayName (0x100b21ec0).
pub const LASER_SOURCES: [&str; 10] = [
    "Sconosciuto",
    "Laser a diodo",
    "Diodo blu",
    "Blu + IR",
    "Blu + Fibra",
    "Blu + MOPA",
    "Laser MOPA",
    "Laser UV",
    "Laser CO2",
    "Laser CO2 + IR",
];

impl Default for Device {
    fn default() -> Self {
        Device {
            name: "GRBL".into(),
            width: 400.0,
            height: 400.0,
            s_max: 1000.0,
            origin: OriginCorner::FrontLeft,
            dynamic_power: true,
            air_assist_m7: false,
            fire_power: 2.0,
            laser_frame: true,
            baud: 115200,
            rapid_speed: 400.0,
            accel: 3000.0,
            controller: "GRBL".into(),
            port: String::new(),
            laser_source: 1,
            // defaults from the person's pictures of the original's device settings
            machine_coords: true,
            pointer_offset_on: false,
            pointer_offset: [0.0, 0.0],
            speed_mm_s: false,
            link_units: true,
            tab_pulse_width: 0.05,
            job_checklist_on: false,
            job_checklist: vec![
                "Il materiale è fissato e a fuoco".into(),
                "L'aria compressa è accesa".into(),
                "L'aspirazione è accesa".into(),
            ],
            scan_offset_on: false,
            scan_offsets: Vec::new(),
            laser1_name: String::new(),
            laser1_rf: false,
            laser1_fiber: false,
            laser2_on: false,
            laser2_name: String::new(),
            one_laser_at_once: false,
            laser2_rf: false,
            laser2_fiber: false,
            laser2_offset_on: false,
            laser2_offset: [0.0, 0.0],
            max_speed: [500.0, 400.0],
            cut_accel: [3000.0, 3000.0],
            engrave_accel: [3000.0, 3000.0],
            corner_precision: 0.01,
            min_corner_speed: 1.0,
            cut_speed_scale: 100.0,
            global_speed_scale: 100.0,
            api_port_on: false,
            api_port: 19000,
        }
    }
}

pub type P2 = [f64; 2];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Polyline {
    pub pts: Vec<P2>,
    pub closed: bool,
}

/// Points closer than this (mm) count as the same point.
pub const SAME_POINT: f64 = 1e-4;
/// Degrees → radians for the Ruota box (constant 0x3c8efa35 in QNumEdit_ValueChanged 0x100c53f38).
pub const DEG_TO_RAD: f64 = 0.0174533;
/// Below this width/height an object isn't resized by Make same width/height (0x38d1b717 in 0x100715fac).
pub const SAME_SIZE_MIN: f64 = 0.0001;

impl Polyline {
    /// Remove repeated points (a point equal to the one before it, and for closed outlines a last
    /// point equal to the first). Exported SVGs often close a shape by repeating its start with a
    /// tiny rounding difference; left in, such near-zero segments made the neighbouring lines
    /// disappear when drawn.
    pub fn cleaned(mut self) -> Polyline {
        let same = |a: P2, b: P2| (a[0] - b[0]).abs() < SAME_POINT && (a[1] - b[1]).abs() < SAME_POINT;
        self.pts.dedup_by(|b, a| same(*a, *b));
        if self.closed {
            while self.pts.len() > 1 && same(self.pts[0], *self.pts.last().unwrap()) {
                self.pts.pop();
            }
        }
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Geometry {
    Rect { w: f64, h: f64, radius: f64 },
    Ellipse { rx: f64, ry: f64 },
    /// `ry` Some: fitted in a box (half sizes r × ry) the original's way (`polygon_box_pts`);
    /// None: older documents, a circle of radius r with a vertex at the top.
    Polygon {
        sides: u32,
        r: f64,
        #[serde(default)]
        ry: Option<f64>,
    },
    /// Star fitted in a box (half sizes rx, ry): `points` tips, inner points at `inner` × radius;
    /// a dual star (`middle` Some) has a third ring at `middle` × radius.
    Star { points: u32, rx: f64, ry: f64, inner: f64, middle: Option<f64> },
    Path { subpaths: Vec<Polyline> },
    Text { text: String, size: f64, font: String, outline: Vec<Polyline> },
    Image { w: f64, h: f64, px_w: u32, px_h: u32, gray: Vec<u8> },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Shape {
    pub id: u64,
    pub geom: Geometry,
    pub layer: usize,
    /// world position of the local origin (mm, Y up)
    pub tx: f64,
    pub ty: f64,
    pub sx: f64,
    pub sy: f64,
    /// degrees, counter-clockwise
    pub rot: f64,
    /// Groups this shape belongs to, outermost first (groups can be nested).
    #[serde(default)]
    pub groups: Vec<u64>,
    /// Manually placed tabs (Shape::AddTab 0x1006d64b4): (path index, fraction of that
    /// path's length), so they follow the shape when it is moved, scaled or rotated.
    #[serde(default)]
    pub tabs: Vec<(usize, f64)>,
}

#[derive(Clone, Copy, Debug)]
pub struct BBox {
    pub min: P2,
    pub max: P2,
}

impl BBox {
    pub fn empty() -> Self {
        BBox { min: [f64::INFINITY; 2], max: [f64::NEG_INFINITY; 2] }
    }
    pub fn add(&mut self, p: P2) {
        self.min[0] = self.min[0].min(p[0]);
        self.min[1] = self.min[1].min(p[1]);
        self.max[0] = self.max[0].max(p[0]);
        self.max[1] = self.max[1].max(p[1]);
    }
    pub fn union(&mut self, o: &BBox) {
        if o.is_valid() {
            self.add(o.min);
            self.add(o.max);
        }
    }
    pub fn is_valid(&self) -> bool {
        self.min[0] <= self.max[0] && self.min[1] <= self.max[1]
    }
    pub fn w(&self) -> f64 {
        self.max[0] - self.min[0]
    }
    pub fn h(&self) -> f64 {
        self.max[1] - self.min[1]
    }
    pub fn center(&self) -> P2 {
        [(self.min[0] + self.max[0]) * 0.5, (self.min[1] + self.max[1]) * 0.5]
    }
    /// Anchor point for a 3x3 grid index (0 = top-left .. 8 = bottom-right).
    pub fn anchor(&self, idx: usize) -> P2 {
        let col = idx % 3;
        let row = idx / 3;
        let x = self.min[0] + self.w() * col as f64 * 0.5;
        let y = self.max[1] - self.h() * row as f64 * 0.5;
        [x, y]
    }
    pub fn contains(&self, o: &BBox) -> bool {
        self.min[0] <= o.min[0] && self.min[1] <= o.min[1] && self.max[0] >= o.max[0] && self.max[1] >= o.max[1]
    }
    pub fn intersects(&self, o: &BBox) -> bool {
        !(o.min[0] > self.max[0] || o.max[0] < self.min[0] || o.min[1] > self.max[1] || o.max[1] < self.min[1])
    }
}

fn ellipse_pts(rx: f64, ry: f64, n: usize) -> Vec<P2> {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            [rx * a.cos(), ry * a.sin()]
        })
        .collect()
}

fn rounded_rect(w: f64, h: f64, r: f64) -> Vec<P2> {
    let hw = w / 2.0;
    let hh = h / 2.0;
    let r = r.max(0.0).min(hw.abs()).min(hh.abs());
    if r <= 1e-6 {
        return vec![[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]];
    }
    let mut pts = Vec::new();
    let corners = [([hw - r, -hh + r], -90.0f64), ([hw - r, hh - r], 0.0), ([-hw + r, hh - r], 90.0), ([-hw + r, -hh + r], 180.0)];
    for (c, start) in corners {
        for k in 0..=8 {
            let a = (start + k as f64 * 90.0 / 8.0).to_radians();
            pts.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    pts
}

/// Regular polygon in a box (PolygonTool::BasicDraw 0x1007576b0, created by PolygonTool::BasicCreateShape
/// 0x100757654 from the two corners of the dragged box): vertex i at angle a = (i + 0.5)·2π/n,
/// point = (rx·sin a, −ry·cos a), so a triangle or pentagon has its tip up and a flat base, and
/// even polygons have flat top and bottom edges.
pub fn polygon_box_pts(sides: u32, rx: f64, ry: f64) -> Vec<P2> {
    let n = sides.max(3) as usize;
    (0..n)
        .map(|i| {
            let a = (i as f64 + 0.5) * std::f64::consts::TAU / n as f64;
            [rx * a.sin(), -ry * a.cos()]
        })
        .collect()
}

/// Star defaults from the original: 5 tips, inner ratio 0.4, dual star middle ratio 0.7
/// (StarShape ctor 0x1006d788c / DualStarShape ctor 0x1006b3fd0, constants 0x101917220..0x10191724f;
/// StarTool::BasicCreateShape 0x100757898 creates UndoCreateStar(…, 5, 0.4, 0)).
pub const STAR_POINTS: u32 = 5;
pub const STAR_INNER: f64 = 0.4;
pub const DUAL_STAR_MIDDLE: f64 = 0.7;

/// Star outline (StarShape::BuildPath 0x1006d7b3c, DualStarShape::BuildPath 0x1006b4278):
/// 2·points vertices (4·points for a dual star) at angle i·2π/n, point = (rx·k·sin a, ry·k·cos a),
/// k = 1 on even vertices, `inner` on odd ones, and `middle` on every 4th+2 of a dual star.
/// The first tip points up (+Y).
pub fn star_pts(points: u32, rx: f64, ry: f64, inner: f64, middle: Option<f64>) -> Vec<P2> {
    let per = if middle.is_some() { 4 } else { 2 };
    let n = (points.max(2) * per) as usize;
    (0..n)
        .map(|i| {
            let k = if i % 2 == 1 {
                inner
            } else if middle.is_some() && i % 4 == 2 {
                middle.unwrap_or(1.0)
            } else {
                1.0
            };
            let a = i as f64 * std::f64::consts::TAU / n as f64;
            [rx * k * a.sin(), ry * k * a.cos()]
        })
        .collect()
}

impl Geometry {
    /// Outline in local coordinates.
    pub fn local_paths(&self) -> Vec<Polyline> {
        match self {
            Geometry::Rect { w, h, radius } => vec![Polyline { pts: rounded_rect(*w, *h, *radius), closed: true }],
            Geometry::Ellipse { rx, ry } => {
                let n = crate::prefs::curve_segments(((rx.abs().max(ry.abs()) * 2.0) as usize).clamp(48, 360));
                vec![Polyline { pts: ellipse_pts(*rx, *ry, n), closed: true }]
            }
            Geometry::Polygon { sides, r, ry: Some(ry) } => vec![Polyline { pts: polygon_box_pts(*sides, *r, *ry), closed: true }],
            Geometry::Polygon { sides, r, ry: None } => {
                let n = (*sides).max(3) as usize;
                let pts = (0..n)
                    .map(|i| {
                        let a = std::f64::consts::FRAC_PI_2 + i as f64 / n as f64 * std::f64::consts::TAU;
                        [r * a.cos(), r * a.sin()]
                    })
                    .collect();
                vec![Polyline { pts, closed: true }]
            }
            Geometry::Star { points, rx, ry, inner, middle } => vec![Polyline { pts: star_pts(*points, *rx, *ry, *inner, *middle), closed: true }],
            Geometry::Path { subpaths } => subpaths.clone(),
            Geometry::Text { outline, .. } => outline.clone(),
            Geometry::Image { w, h, .. } => {
                let hw = w / 2.0;
                let hh = h / 2.0;
                vec![Polyline { pts: vec![[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]], closed: true }]
            }
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Geometry::Rect { .. } => "Rectangle",
            Geometry::Ellipse { .. } => "Ellipse",
            Geometry::Polygon { .. } => "Polygon",
            Geometry::Star { middle: None, .. } => "Star",
            Geometry::Star { .. } => "DualStar",
            Geometry::Path { .. } => "Path",
            Geometry::Text { .. } => "Text",
            Geometry::Image { .. } => "Image",
        }
    }
}

impl Shape {
    pub fn new(id: u64, geom: Geometry, layer: usize, x: f64, y: f64) -> Self {
        Shape { id, geom, layer, tx: x, ty: y, sx: 1.0, sy: 1.0, rot: 0.0, groups: Vec::new(), tabs: Vec::new() }
    }

    /// World positions of the manual tabs.
    pub fn tab_points(&self) -> Vec<P2> {
        let paths = self.world_paths();
        self.tabs
            .iter()
            .filter_map(|&(i, t)| paths.get(i).map(|pl| path_point_at(pl, t)))
            .collect()
    }

    /// The closest point of the shape's outline to `p`: (path index, fraction, distance).
    pub fn nearest_on_outline(&self, p: P2) -> Option<(usize, f64, f64)> {
        let mut best: Option<(usize, f64, f64)> = None;
        for (i, pl) in self.world_paths().iter().enumerate() {
            let mut pts = pl.pts.clone();
            if pl.closed && pts.len() > 2 {
                pts.push(pts[0]);
            }
            let total: f64 = pts.windows(2).map(|w| ((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))).sum();
            if total < 1e-9 {
                continue;
            }
            let mut acc = 0.0;
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let l2 = d[0] * d[0] + d[1] * d[1];
                let l = l2.sqrt();
                let u = if l2 > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
                let q = [a[0] + d[0] * u, a[1] + d[1] * u];
                let dist = (p[0] - q[0]).hypot(p[1] - q[1]);
                if best.is_none_or(|b| dist < b.2) {
                    best = Some((i, (acc + u * l) / total, dist));
                }
                acc += l;
            }
        }
        best
    }

    pub fn to_world(&self, p: P2) -> P2 {
        let (s, c) = self.rot.to_radians().sin_cos();
        let x = p[0] * self.sx;
        let y = p[1] * self.sy;
        [self.tx + x * c - y * s, self.ty + x * s + y * c]
    }

    pub fn to_local(&self, p: P2) -> P2 {
        let (s, c) = self.rot.to_radians().sin_cos();
        let dx = p[0] - self.tx;
        let dy = p[1] - self.ty;
        let x = dx * c + dy * s;
        let y = -dx * s + dy * c;
        [x / self.sx, y / self.sy]
    }

    pub fn world_paths(&self) -> Vec<Polyline> {
        self.geom
            .local_paths()
            .into_iter()
            .map(|pl| Polyline { pts: pl.pts.iter().map(|p| self.to_world(*p)).collect(), closed: pl.closed })
            .collect()
    }

    pub fn bbox(&self) -> BBox {
        let mut b = BBox::empty();
        for pl in self.world_paths() {
            for p in pl.pts {
                b.add(p);
            }
        }
        b
    }

    /// Scale the shape about a fixed world point.
    pub fn scale_about(&mut self, fx: f64, fy: f64, about: P2) {
        // Scaling happens along the world axes.
        let r = self.rot.rem_euclid(90.0);
        let axis_aligned = r < 1e-6 || (90.0 - r) < 1e-6;
        let uniform = (fx - fy).abs() < 1e-12;
        if uniform {
            self.sx *= fx;
            self.sy *= fx;
        } else if axis_aligned {
            let quarter = ((self.rot / 90.0).round() as i64).rem_euclid(2) == 1;
            if quarter {
                self.sx *= fy;
                self.sy *= fx;
            } else {
                self.sx *= fx;
                self.sy *= fy;
            }
        } else if !matches!(self.geom, Geometry::Image { .. }) {
            // A rotated shape stretched along world axes becomes a plain path.
            let paths: Vec<Polyline> = self
                .world_paths()
                .into_iter()
                .map(|pl| Polyline {
                    pts: pl.pts.iter().map(|p| [about[0] + (p[0] - about[0]) * fx, about[1] + (p[1] - about[1]) * fy]).collect(),
                    closed: pl.closed,
                })
                .collect();
            let mut b = BBox::empty();
            for pl in &paths {
                for p in &pl.pts {
                    b.add(*p);
                }
            }
            let c = b.center();
            self.geom = Geometry::Path {
                subpaths: paths
                    .into_iter()
                    .map(|pl| Polyline { pts: pl.pts.iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect(), closed: pl.closed })
                    .collect(),
            };
            self.tx = c[0];
            self.ty = c[1];
            self.sx = 1.0;
            self.sy = 1.0;
            self.rot = 0.0;
            return;
        } else {
            let k = (fx * fy).abs().sqrt();
            self.sx *= k;
            self.sy *= k;
        }
        self.tx = about[0] + (self.tx - about[0]) * fx;
        self.ty = about[1] + (self.ty - about[1]) * fy;
    }

    pub fn rotate_about(&mut self, deg: f64, about: P2) {
        let (s, c) = (deg * DEG_TO_RAD).sin_cos();
        let dx = self.tx - about[0];
        let dy = self.ty - about[1];
        self.tx = about[0] + dx * c - dy * s;
        self.ty = about[1] + dx * s + dy * c;
        self.rot = (self.rot + deg).rem_euclid(360.0);
    }

    pub fn mirror(&mut self, horizontal: bool, about: P2) {
        if horizontal {
            self.tx = 2.0 * about[0] - self.tx;
            self.sx = -self.sx;
            self.rot = (-self.rot).rem_euclid(360.0);
        } else {
            self.ty = 2.0 * about[1] - self.ty;
            self.sy = -self.sy;
            self.rot = (-self.rot).rem_euclid(360.0);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Document {
    pub shapes: Vec<Shape>,
    pub layers: Vec<CutSetting>,
    /// Cut order of layers (layer indices).
    pub layer_order: Vec<usize>,
    pub next_id: u64,
    pub device: Device,
    /// Laser-panel states kept with the project when the settings say so (Impostazioni editor:
    /// "Salva Taglia selezionata / Origine selezione nel progetto", "Salva origine lavoro")
    pub job_state: SavedJobState,
}

/// What a project remembers of the Laser panel; None = not saved with it.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SavedJobState {
    pub cut_selected: Option<bool>,
    pub selection_origin: Option<bool>,
    pub start_from: Option<crate::gcode::StartFrom>,
    pub job_origin: Option<usize>,
}

impl Default for Document {
    fn default() -> Self {
        let layers = (0..LAYER_COUNT)
            .map(|i| {
                let mut c = CutSetting::default();
                c.name = layer_name(i);
                if is_tool_layer(i) {
                    c.output = false;
                }
                c
            })
            .collect();
        Document { shapes: Vec::new(), layers, layer_order: Vec::new(), next_id: 1, device: Device::default(), job_state: SavedJobState::default() }
    }
}

impl Document {
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_shape(&mut self, mut s: Shape) -> u64 {
        if s.id == 0 {
            s.id = self.alloc_id();
        }
        let id = s.id;
        self.touch_layer(s.layer);
        self.shapes.push(s);
        id
    }

    pub fn touch_layer(&mut self, layer: usize) {
        if !self.layer_order.contains(&layer) {
            self.layer_order.push(layer);
        }
    }

    /// Pictures, vectors, or both, for the shapes currently on this layer.
    pub fn layer_art(&self, layer: usize) -> LayerArt {
        let mut image = false;
        let mut vector = false;
        for s in &self.shapes {
            if s.layer != layer {
                continue;
            }
            if matches!(s.geom, Geometry::Image { .. }) {
                image = true;
            } else {
                vector = true;
            }
            if image && vector {
                break;
            }
        }
        match (image, vector) {
            (false, false) => LayerArt::Empty,
            (true, false) => LayerArt::Image,
            (false, true) => LayerArt::Vector,
            (true, true) => LayerArt::Mixed,
        }
    }

    /// Keep an image layer on Image, and take Image away from a vector layer.
    /// Returns whether the stored mode changed.
    pub fn lock_cut_mode(&mut self, layer: usize) -> bool {
        if layer >= self.layers.len() {
            return false;
        }
        let mode = self.layer_art(layer).clamp(self.layers[layer].mode);
        if mode == self.layers[layer].mode {
            return false;
        }
        self.layers[layer].mode = mode;
        true
    }

    /// Layers in cut order that have at least one shape.
    pub fn used_layers(&self) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for &l in &self.layer_order {
            if self.shapes.iter().any(|s| s.layer == l) && !out.contains(&l) {
                out.push(l);
            }
        }
        for s in &self.shapes {
            if !out.contains(&s.layer) {
                out.push(s.layer);
            }
        }
        out
    }

    pub fn find(&self, id: u64) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }

    pub fn bbox_of(&self, ids: &[u64]) -> BBox {
        let mut b = BBox::empty();
        for s in &self.shapes {
            if ids.contains(&s.id) {
                b.union(&s.bbox());
            }
        }
        b
    }

    pub fn bbox_all(&self) -> BBox {
        let mut b = BBox::empty();
        for s in &self.shapes {
            b.union(&s.bbox());
        }
        b
    }
}

/// Distance from point to segment.
/// The point a fraction `t` of the way along a path (closed paths wrap back to the start).
pub fn path_point_at(pl: &Polyline, t: f64) -> P2 {
    let mut pts = pl.pts.clone();
    if pl.closed && pts.len() > 2 {
        pts.push(pts[0]);
    }
    if pts.len() < 2 {
        return pts.first().copied().unwrap_or([0.0, 0.0]);
    }
    let total: f64 = pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
    let mut s = t.clamp(0.0, 1.0) * total;
    for w in pts.windows(2) {
        let l = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if s <= l && l > 0.0 {
            let f = s / l;
            return [w[0][0] + (w[1][0] - w[0][0]) * f, w[0][1] + (w[1][1] - w[0][1]) * f];
        }
        s -= l;
    }
    *pts.last().unwrap()
}

pub fn dist_seg(p: P2, a: P2, b: P2) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let x = a[0] + t * dx - p[0];
    let y = a[1] + t * dy - p[1];
    (x * x + y * y).sqrt()
}

/// Even-odd point in polygon test over several rings.
pub fn point_in_rings(p: P2, rings: &[Polyline]) -> bool {
    let mut inside = false;
    for r in rings.iter().filter(|r| r.closed) {
        let n = r.pts.len();
        if n < 3 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let a = r.pts[i];
            let b = r.pts[j];
            if (a[1] > p[1]) != (b[1] > p[1]) {
                let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if p[0] < x {
                    inside = !inside;
                }
            }
            j = i;
        }
    }
    inside
}

#[cfg(test)]
mod shape_tests {
    use super::*;

    #[test]
    fn polygons_and_stars_follow_the_original() {
        // triangle: tip up, flat base at -0.5·ry
        let t = polygon_box_pts(3, 10.0, 10.0);
        assert!((t[1][1] - 10.0).abs() < 1e-9 && t[1][0].abs() < 1e-9);
        assert!((t[0][1] + 5.0).abs() < 1e-9 && (t[2][1] + 5.0).abs() < 1e-9);
        // octagon: flat top edge
        let o = polygon_box_pts(8, 10.0, 10.0);
        assert!(o.iter().filter(|p| (p[1] - o.iter().map(|q| q[1]).fold(f64::MIN, f64::max)).abs() < 1e-9).count() == 2);
        // star: 10 vertices, first tip up, inner at 0.4
        let s = star_pts(STAR_POINTS, 10.0, 10.0, STAR_INNER, None);
        assert_eq!(s.len(), 10);
        assert!((s[0][1] - 10.0).abs() < 1e-9);
        assert!(((s[1][0].powi(2) + s[1][1].powi(2)).sqrt() - 4.0).abs() < 1e-9);
        assert_eq!(star_pts(STAR_POINTS, 10.0, 10.0, STAR_INNER, Some(DUAL_STAR_MIDDLE)).len(), 20);
    }
}

#[cfg(test)]
mod cut_mode_tests {
    use super::*;

    #[test]
    fn cut_mode_follows_the_artwork() {
        let mut doc = Document::default();
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 0.0, 0.0));
        doc.layers[0].mode = CutMode::Image;
        assert!(doc.lock_cut_mode(0));
        assert_eq!(doc.layers[0].mode, CutMode::Line);
        let art = doc.layer_art(0);
        assert!(!art.allows(CutMode::Image));
        assert!(art.allows(CutMode::Fill));
        assert!(art.allows(CutMode::OffsetFill));

        let mut doc = Document::default();
        doc.add_shape(Shape::new(
            0,
            Geometry::Image { w: 10.0, h: 10.0, px_w: 2, px_h: 2, gray: vec![0, 0, 0, 0] },
            0,
            5.0,
            5.0,
        ));
        doc.layers[0].mode = CutMode::Fill;
        assert!(doc.lock_cut_mode(0));
        assert_eq!(doc.layers[0].mode, CutMode::Image);
        let art = doc.layer_art(0);
        assert!(art.allows(CutMode::Image));
        assert!(!art.allows(CutMode::Line));
        assert!(!art.allows(CutMode::OffsetFill));

        // an empty layer keeps whatever mode the editor was showing
        let mut doc = Document::default();
        doc.layers[0].mode = CutMode::Fill;
        assert!(!doc.lock_cut_mode(0));
        assert_eq!(doc.layers[0].mode, CutMode::Fill);
        assert!(doc.layer_art(0).allows(CutMode::Image));
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::BBox;

    #[test]
    fn nine_reference_points() {
        let b = BBox { min: [10.0, 20.0], max: [30.0, 60.0] };
        assert_eq!(b.anchor(0), [10.0, 60.0]); // top-left
        assert_eq!(b.anchor(4), [20.0, 40.0]); // centre
        assert_eq!(b.anchor(8), [30.0, 20.0]); // bottom-right
        assert_eq!(b.anchor(5), [30.0, 40.0]); // middle-right
    }
}
