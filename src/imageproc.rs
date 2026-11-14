//! Image modes for a raster layer.
//!
//! The mode names come from the original string table (MECHANICS.md) plus
//! "Carta di giornale" on the original Italian menu. Floyd-Steinberg weights
//! are the ones in `ImageDither` `0x10021d86c` (7/16, 3/16, 5/16, 1/16).
//! The other kernels are the published ones for those named modes.

use crate::model::{CutSetting, ImageMode};

const FLOYD: &[(i32, i32, f64)] = &[
    (1, 0, 7.0 / 16.0),
    (-1, 1, 3.0 / 16.0),
    (0, 1, 5.0 / 16.0),
    (1, 1, 1.0 / 16.0),
];

/// Atkinson keeps only 6/8 of the error, which is why light areas stay light.
const ATKINSON: &[(i32, i32, f64)] = &[
    (1, 0, 1.0 / 8.0),
    (2, 0, 1.0 / 8.0),
    (-1, 1, 1.0 / 8.0),
    (0, 1, 1.0 / 8.0),
    (1, 1, 1.0 / 8.0),
    (0, 2, 1.0 / 8.0),
];

const STUCKI: &[(i32, i32, f64)] = &[
    (1, 0, 8.0 / 42.0),
    (2, 0, 4.0 / 42.0),
    (-2, 1, 2.0 / 42.0),
    (-1, 1, 4.0 / 42.0),
    (0, 1, 8.0 / 42.0),
    (1, 1, 4.0 / 42.0),
    (2, 1, 2.0 / 42.0),
    (-2, 2, 1.0 / 42.0),
    (-1, 2, 2.0 / 42.0),
    (0, 2, 4.0 / 42.0),
    (1, 2, 2.0 / 42.0),
    (2, 2, 1.0 / 42.0),
];

const JARVIS: &[(i32, i32, f64)] = &[
    (1, 0, 7.0 / 48.0),
    (2, 0, 5.0 / 48.0),
    (-2, 1, 3.0 / 48.0),
    (-1, 1, 5.0 / 48.0),
    (0, 1, 7.0 / 48.0),
    (1, 1, 5.0 / 48.0),
    (2, 1, 3.0 / 48.0),
    (-2, 2, 1.0 / 48.0),
    (-1, 2, 3.0 / 48.0),
    (0, 2, 5.0 / 48.0),
    (1, 2, 3.0 / 48.0),
    (2, 2, 1.0 / 48.0),
];

/// Bayer 8×8. A cell burns when its level is above `(value + 0.5) / 64`.
const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Clustered dots, two growing dots per 8×8 cell, the newspaper look.
const NEWSPRINT: [[u8; 8]; 8] = [
    [24, 10, 12, 26, 35, 47, 49, 37],
    [8, 0, 2, 14, 45, 59, 61, 51],
    [22, 6, 4, 16, 43, 57, 63, 53],
    [30, 20, 18, 28, 33, 41, 55, 39],
    [34, 46, 48, 36, 25, 11, 13, 27],
    [44, 58, 60, 50, 9, 1, 3, 15],
    [42, 56, 62, 52, 23, 7, 5, 17],
    [32, 40, 54, 38, 31, 21, 19, 29],
];

/// `grid` holds levels in 0..=1 (1 = burn fully). Negative entries are outside
/// the picture and stay untouched. `x0`/`y_top` are the world position of the
/// top-left sample corner; `interval` is the sample step in mm.
pub fn apply(cs: &CutSetting, grid: &mut [f64], cols: usize, rows: usize, x0: f64, y_top: f64, interval: f64) {
    if cols == 0 || rows == 0 || cs.pass_through {
        return;
    }
    match cs.image_mode {
        ImageMode::Threshold => threshold(grid, 0.5),
        ImageMode::Ordered => matrix(grid, cols, &BAYER),
        ImageMode::Atkinson => diffuse(grid, cols, rows, ATKINSON),
        ImageMode::Dithered => diffuse(grid, cols, rows, FLOYD),
        ImageMode::Stucki => diffuse(grid, cols, rows, STUCKI),
        ImageMode::Jarvis => diffuse(grid, cols, rows, JARVIS),
        ImageMode::Newsprint => matrix(grid, cols, &NEWSPRINT),
        ImageMode::Halftone => halftone(grid, cols, rows, x0, y_top, interval, cs.cells_per_inch, cs.halftone_angle),
        ImageMode::Sketch => sketch(grid, cols, rows),
        // these keep the grey: Dot turns it into stop time, 3D Slice into depth (one level per pass)
        ImageMode::Grayscale | ImageMode::Dot | ImageMode::Slice3D => {}
    }
}

fn threshold(grid: &mut [f64], cut: f64) {
    for v in grid.iter_mut() {
        if *v >= 0.0 {
            *v = if *v >= cut { 1.0 } else { 0.0 };
        }
    }
}

fn matrix(grid: &mut [f64], cols: usize, m: &[[u8; 8]; 8]) {
    for (i, v) in grid.iter_mut().enumerate() {
        if *v < 0.0 {
            continue;
        }
        let x = i % cols;
        let y = i / cols;
        let t = (m[y % 8][x % 8] as f64 + 0.5) / 64.0;
        *v = if *v >= t { 1.0 } else { 0.0 };
    }
}

fn diffuse(grid: &mut [f64], cols: usize, rows: usize, kernel: &[(i32, i32, f64)]) {
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            if grid[i] < 0.0 {
                continue;
            }
            let old = grid[i];
            let new = if old >= 0.5 { 1.0 } else { 0.0 };
            grid[i] = new;
            let err = old - new;
            for (dx, dy, w) in kernel {
                let cc = c as i32 + dx;
                let rr = r as i32 + dy;
                if rr >= 0 && (rr as usize) < rows && cc >= 0 && (cc as usize) < cols {
                    let j = rr as usize * cols + cc as usize;
                    if grid[j] >= 0.0 {
                        grid[j] += err * w;
                    }
                }
            }
        }
    }
}

/// Round dots. Cell size is 25.4 mm / cells-per-inch, rotated by the halftone angle.
/// A darker sample grows a larger dot (it burns farther from the cell centre).
fn halftone(grid: &mut [f64], cols: usize, rows: usize, x0: f64, y_top: f64, interval: f64, cells_per_inch: f64, angle_deg: f64) {
    let cell = 25.4 / cells_per_inch.max(1.0);
    let (s, c) = angle_deg.to_radians().sin_cos();
    let step = interval.max(1e-6);
    for r in 0..rows {
        for col in 0..cols {
            let i = r * cols + col;
            if grid[i] < 0.0 {
                continue;
            }
            let x = x0 + (col as f64 + 0.5) * step;
            let y = y_top - (r as f64 + 0.5) * step;
            let xr = x * c + y * s;
            let yr = -x * s + y * c;
            let fx = (xr / cell).rem_euclid(1.0) - 0.5;
            let fy = (yr / cell).rem_euclid(1.0) - 0.5;
            // 0 at the centre, 1 at the corner of the cell
            let dist = ((fx * fx + fy * fy).sqrt() / std::f64::consts::FRAC_1_SQRT_2).clamp(0.0, 1.0);
            grid[i] = if grid[i] >= dist { 1.0 } else { 0.0 };
        }
    }
}

/// Edges burn, flat areas do not (a pencil-sketch of the picture).
fn sketch(grid: &mut [f64], cols: usize, rows: usize) {
    let src = grid.to_vec();
    let at = |rr: i32, cc: i32| -> f64 {
        if rr < 0 || cc < 0 || rr as usize >= rows || cc as usize >= cols {
            return 0.0;
        }
        let v = src[rr as usize * cols + cc as usize];
        if v < 0.0 { 0.0 } else { v }
    };
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            if src[i] < 0.0 {
                continue;
            }
            let ri = r as i32;
            let ci = c as i32;
            let gx = -at(ri - 1, ci - 1) + at(ri - 1, ci + 1)
                - 2.0 * at(ri, ci - 1) + 2.0 * at(ri, ci + 1)
                - at(ri + 1, ci - 1) + at(ri + 1, ci + 1);
            let gy = -at(ri - 1, ci - 1) - 2.0 * at(ri - 1, ci) - at(ri - 1, ci + 1)
                + at(ri + 1, ci - 1) + 2.0 * at(ri + 1, ci) + at(ri + 1, ci + 1);
            let mag = (gx * gx + gy * gy).sqrt();
            grid[i] = if mag >= 0.45 { 1.0 } else { 0.0 };
        }
    }
}

/// A small example picture: a left-to-right shade, five grey steps, then a light patch.
/// Levels are 1 = dark. Used by the cut-settings window.
pub fn example_grid(cs: &CutSetting, w: usize, h: usize) -> Vec<f64> {
    let mut grid = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let u = x as f64 / w.max(1) as f64;
            let v = y as f64 / h.max(1) as f64;
            grid[y * w + x] = if u < 0.58 {
                1.0 - u / 0.58
            } else if u < 0.80 {
                let band = (v * 5.0).floor().clamp(0.0, 4.0);
                1.0 - band / 4.0
            } else {
                0.22
            };
        }
    }
    // One sample per pixel. Halftone cells stay a few pixels wide at 50 cells/inch.
    let interval = 0.1;
    apply(cs, &mut grid, w, h, 0.0, h as f64 * interval, interval);
    grid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CutSetting;

    fn cs(mode: ImageMode) -> CutSetting {
        let mut c = CutSetting::default();
        c.image_mode = mode;
        c.cells_per_inch = 50.0;
        c.halftone_angle = 22.5;
        c
    }

    #[test]
    fn threshold_splits_a_ramp_and_stucki_does_not() {
        let mut ramp: Vec<f64> = (0..16).map(|i| i as f64 / 15.0).collect();
        threshold(&mut ramp, 0.5);
        assert!(ramp.iter().all(|v| *v == 0.0 || *v == 1.0));
        assert_eq!(ramp.iter().filter(|v| **v == 1.0).count(), 8);

        let mut soft: Vec<f64> = (0..32).map(|i| i as f64 / 31.0).collect();
        let mut hard = soft.clone();
        diffuse(&mut soft, 32, 1, STUCKI);
        threshold(&mut hard, 0.5);
        // error diffusion turns some mid pixels the other way; a hard cut does not
        assert_ne!(soft, hard);
    }

    #[test]
    fn grayscale_and_pass_through_keep_the_levels() {
        let mut g = vec![0.2, 0.8];
        let mut c = cs(ImageMode::Grayscale);
        apply(&c, &mut g, 2, 1, 0.0, 0.0, 0.1);
        assert_eq!(g, vec![0.2, 0.8]);
        c.pass_through = true;
        c.image_mode = ImageMode::Stucki;
        let mut g = vec![0.2, 0.8];
        apply(&c, &mut g, 2, 1, 0.0, 0.0, 0.1);
        assert_eq!(g, vec![0.2, 0.8]);
    }

    #[test]
    fn halftone_angle_changes_the_dots() {
        let mut a = cs(ImageMode::Halftone);
        a.halftone_angle = 0.0;
        let mut b = a.clone();
        b.halftone_angle = 22.5;
        let ga = example_grid(&a, 48, 24);
        let gb = example_grid(&b, 48, 24);
        assert_ne!(ga, gb);
        assert!(ga.iter().any(|v| *v == 0.0) && ga.iter().any(|v| *v == 1.0));
    }

    #[test]
    fn outside_samples_stay_outside() {
        let mut g = vec![-1.0, 0.9, -1.0];
        apply(&cs(ImageMode::Threshold), &mut g, 3, 1, 0.0, 0.0, 0.1);
        assert_eq!(g[0], -1.0);
        assert_eq!(g[2], -1.0);
        assert_eq!(g[1], 1.0);
    }
}
