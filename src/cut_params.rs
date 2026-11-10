use serde::{Serialize, Deserialize};

/// Parameters that control how a shape is cut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutParams {
    /// Type of cut (0 = line, 1 = shape, …) – kept for compatibility.
    pub cut_type: u8,
    /// Cutting speed in mm/min.
    #[serde(alias = "speed_mm_s")]
    pub speed_mm_min: f64,
    /// Minimum power as a fraction (0.0‑1.0).
    pub min_power: f64,
    /// Maximum power as a fraction (0.0‑1.0).
    pub max_power: f64,
    /// Number of passes.
    pub passes: u32,
    /// Interval between passes in mm.
    pub interval_mm: f64,
    /// Angle of hatching in degrees.
    pub angle_deg: f64,
    /// Bidirectional cut (true = cut both ways).
    pub bidirectional: bool,
    /// Cross‑hatch enabled.
    pub cross_hatch: bool,
    /// Overscan percentage (e.g. 0.025 = 2.5%).
    pub overscan: f64,
    /// PWM frequency in Hz.
    pub pwm_freq_hz: u32,
    /// Pulses per inch (used by some engravers).
    pub ppi: u32,
    /// Air‑assist on/off.
    pub air_assist: bool,
    /// Scan option – `merge_all` is the default used by LightBurn.
    pub scan_opt_merge_all: bool,
    /// Width of a dot in mm (used for raster‑dot mode).
    pub dot_width_mm: f64,
}

impl Default for CutParams {
    fn default() -> Self {
        CutParams {
            cut_type: 0,
            speed_mm_min: 100.0,
            min_power: 0.20,
            max_power: 0.20,
            passes: 1,
            interval_mm: 0.1,
            angle_deg: 0.0,
            bidirectional: true,
            cross_hatch: false,
            overscan: 0.025,
            pwm_freq_hz: 20_000,
            ppi: 200,
            air_assist: true,
            scan_opt_merge_all: true,
            dot_width_mm: 0.08,
        }
    }
}
