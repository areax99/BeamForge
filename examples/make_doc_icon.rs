//! Draws the icon of saved BeamForge projects (.beamforge files): a light page with a folded
//! corner, BeamForge's orange diamond and laser beam, and an orange band at the bottom.
//! Writes assets/document.png (1024×1024, for macOS and Linux) and assets/document.ico
//! (16…256 px, for Windows).
//!
//! Run from the app folder:  cargo run --example make_doc_icon

use image::{Rgba, RgbaImage, imageops::FilterType};

const N: u32 = 1024;

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

fn over(bottom: [f32; 4], top: [f32; 4]) -> [f32; 4] {
    let a = top[3] + bottom[3] * (1.0 - top[3]);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let c = |i: usize| (top[i] * top[3] + bottom[i] * bottom[3] * (1.0 - top[3])) / a;
    [c(0), c(1), c(2), a]
}

// Page outline (a portrait sheet, like macOS document icons) and its folded corner.
const X0: f32 = 196.0;
const X1: f32 = 828.0;
const Y0: f32 = 80.0;
const Y1: f32 = 944.0;
const FOLD: f32 = 190.0;
const R: f32 = 28.0;

fn in_page(x: f32, y: f32) -> bool {
    if x < X0 || x > X1 || y < Y0 || y > Y1 {
        return false;
    }
    // the folded corner is cut off the top right
    if (X1 - x) + (y - Y0) < FOLD {
        return false;
    }
    // rounded bottom corners and top-left corner
    for (cx, cy) in [(X0 + R, Y0 + R), (X0 + R, Y1 - R), (X1 - R, Y1 - R)] {
        let qx = if cx < 512.0 { x < cx } else { x > cx };
        let qy = if cy < 512.0 { y < cy } else { y > cy };
        if qx && qy && (x - cx).powi(2) + (y - cy).powi(2) > R * R {
            return false;
        }
    }
    true
}

fn sample(x: f32, y: f32) -> [f32; 4] {
    let mut px = [0.0f32; 4];
    // soft shadow under the page
    let sx = x - 6.0;
    let sy = y - 14.0;
    if in_page(sx, sy) {
        px = [0.0, 0.0, 0.0, 0.22];
    }
    if !in_page(x, y) {
        // the folded flap: a triangle under the cut, darker paper
        let in_flap = x <= X1 && y >= Y0 && (X1 - x) + (y - Y0) < FOLD && x >= X1 - FOLD && y <= Y0 + FOLD;
        if in_flap {
            let t = ((X1 - x) - (y - Y0)).abs() / FOLD;
            return over(px, mix([0.80, 0.82, 0.86, 1.0], [0.70, 0.72, 0.77, 1.0], t));
        }
        return px;
    }
    // paper, slightly cooler at the bottom, thin grey edge
    let t = (y - Y0) / (Y1 - Y0);
    let mut c = mix([0.98, 0.98, 0.99, 1.0], [0.91, 0.92, 0.94, 1.0], t);
    let edge = !in_page(x - 6.0, y) || !in_page(x + 6.0, y) || !in_page(x, y - 6.0) || !in_page(x, y + 6.0);
    if edge {
        c = mix(c, [0.62, 0.64, 0.68, 1.0], 0.7);
    }
    px = over(px, c);

    // faint design lines on the sheet, like a cut layout
    let (cx, cy) = (512.0, 470.0);
    for (ly, w) in [(250.0, 180.0), (290.0, 120.0)] {
        if (y - ly).abs() < 5.0 && (x - (X0 + 70.0)) >= 0.0 && x < X0 + 70.0 + w {
            px = over(px, [0.55, 0.58, 0.64, 0.6]);
        }
    }

    // glow, beam and diamond: the same mark as the app icon, smaller
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    px = over(px, [1.0, 0.48, 0.14, (1.0 - d / 260.0).max(0.0).powi(2) * 0.35]);
    let (a, b) = (150.0, 190.0);
    let diamond_top = cy - b;
    if y >= 300.0 && y <= diamond_top + 14.0 {
        let fade = ((y - 300.0) / 60.0).clamp(0.0, 1.0);
        let dx = (x - cx).abs();
        px = over(px, [1.0, 0.45, 0.12, (1.0 - dx / 24.0).max(0.0).powi(2) * 0.8 * fade]);
        if dx <= 5.0 {
            px = over(px, [1.0, 0.6, 0.3, fade]);
        }
    }
    let k = (x - cx).abs() / a + (y - cy).abs() / b;
    if k <= 1.0 {
        let shade = (y - (cy - b)) / (2.0 * b);
        let mut c = mix([1.0, 0.62, 0.25, 1.0], [0.78, 0.22, 0.08, 1.0], shade);
        if x < cx {
            c = mix(c, [1.0, 0.85, 0.6, 1.0], 0.18);
        }
        if k > 0.96 {
            c = mix(c, [0.62, 0.18, 0.06, 1.0], 0.6);
        }
        px = over(px, c);
    }
    let hs = ((x - cx).powi(2) + (y - (cy - b + 8.0)).powi(2)).sqrt();
    px = over(px, [1.0, 0.97, 0.9, (1.0 - hs / 28.0).max(0.0)]);

    // orange band at the bottom with three white "file" marks, so the type reads at small sizes
    if y >= 730.0 && y <= 850.0 {
        let band = mix([0.98, 0.50, 0.15, 1.0], [0.85, 0.30, 0.08, 1.0], (y - 730.0) / 120.0);
        px = over(px, band);
        for bx in [372.0, 512.0, 652.0] {
            let k = (x - bx).abs() / 34.0 + (y - 790.0).abs() / 40.0;
            if k <= 1.0 {
                px = over(px, [1.0, 1.0, 1.0, 0.92]);
            }
        }
    }
    px
}

fn main() {
    let s = 4;
    let mut img = RgbaImage::new(N, N);
    for py in 0..N {
        for pxl in 0..N {
            let mut acc = [0.0f32; 4];
            for j in 0..s {
                for i in 0..s {
                    let c = sample(pxl as f32 + (i as f32 + 0.5) / s as f32, py as f32 + (j as f32 + 0.5) / s as f32);
                    acc[0] += c[0] * c[3];
                    acc[1] += c[1] * c[3];
                    acc[2] += c[2] * c[3];
                    acc[3] += c[3];
                }
            }
            let n = (s * s) as f32;
            let a = acc[3] / n;
            let un = |v: f32| if a > 0.0 { (v / n / a * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            img.put_pixel(pxl, py, Rgba([un(acc[0]), un(acc[1]), un(acc[2]), (a * 255.0).round() as u8]));
        }
    }
    std::fs::create_dir_all("assets").expect("assets folder");
    img.save("assets/document.png").expect("write assets/document.png");

    let frames: Vec<image::codecs::ico::IcoFrame> = [256u32, 128, 64, 48, 32, 16]
        .iter()
        .map(|&size| {
            let small = image::imageops::resize(&img, size, size, FilterType::Lanczos3);
            image::codecs::ico::IcoFrame::as_png(small.as_raw(), size, size, image::ExtendedColorType::Rgba8).expect("ico frame")
        })
        .collect();
    let file = std::fs::File::create("assets/document.ico").expect("create assets/document.ico");
    image::codecs::ico::IcoEncoder::new(file).encode_images(&frames).expect("write assets/document.ico");
    println!("Wrote assets/document.png (1024×1024) and assets/document.ico (16–256 px)");
}
