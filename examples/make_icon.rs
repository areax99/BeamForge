//! Draws BeamForge's own icon (no artwork from the original): a dark rounded square, a laser beam
//! and the orange diamond used in the menu. Writes assets/icon.png (1024×1024, for macOS and
//! Linux) and assets/icon.ico (16…256 px, for Windows).
//!
//! Run from the app folder:  cargo run --example make_icon

use image::{Rgba, RgbaImage, imageops::FilterType};

const N: u32 = 1024;

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

/// Over-compositing of `top` (straight alpha) onto `bottom`.
fn over(bottom: [f32; 4], top: [f32; 4]) -> [f32; 4] {
    let a = top[3] + bottom[3] * (1.0 - top[3]);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let c = |i: usize| (top[i] * top[3] + bottom[i] * bottom[3] * (1.0 - top[3])) / a;
    [c(0), c(1), c(2), a]
}

/// Colour of one sample point.
fn sample(x: f32, y: f32) -> [f32; 4] {
    let mut px = [0.0f32; 4];
    // macOS-style tile: 824×824 rounded square centred in 1024
    let (x0, y0, x1, y1, r) = (100.0, 100.0, 924.0, 924.0, 185.0);
    let qx = (x0 + r - x).max(x - (x1 - r)).max(0.0);
    let qy = (y0 + r - y).max(y - (y1 - r)).max(0.0);
    let inside = x >= x0 && x <= x1 && y >= y0 && y <= y1 && qx * qx + qy * qy <= r * r;
    if !inside {
        return px;
    }
    let t = (y - y0) / (y1 - y0);
    px = mix([0.17, 0.19, 0.23, 1.0], [0.08, 0.09, 0.11, 1.0], t);

    let (cx, cy) = (512.0, 600.0);
    // soft orange glow around the diamond
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    let glow = (1.0 - d / 380.0).max(0.0).powi(2) * 0.55;
    px = over(px, [1.0, 0.48, 0.14, glow]);

    // laser beam from the top of the tile down to the diamond: white core, orange halo
    let beam_top = 170.0;
    let diamond_top = cy - 270.0;
    if y >= beam_top && y <= diamond_top + 20.0 {
        let fade = ((y - beam_top) / 120.0).clamp(0.0, 1.0);
        let dx = (x - cx).abs();
        let halo = (1.0 - dx / 34.0).max(0.0).powi(2) * 0.8 * fade;
        px = over(px, [1.0, 0.45, 0.12, halo]);
        if dx <= 7.0 {
            px = over(px, [1.0, 0.95, 0.85, fade]);
        }
    }

    // the diamond (◆): orange gradient, lighter left facet
    let (a, b) = (215.0, 270.0);
    let k = (x - cx).abs() / a + (y - cy).abs() / b;
    if k <= 1.0 {
        let shade = (y - (cy - b)) / (2.0 * b);
        let mut c = mix([1.0, 0.62, 0.25, 1.0], [0.78, 0.22, 0.08, 1.0], shade);
        if x < cx {
            c = mix(c, [1.0, 0.85, 0.6, 1.0], 0.18);
        }
        // thin bright rim
        if k > 0.965 {
            c = mix(c, [1.0, 0.9, 0.75, 1.0], 0.6);
        }
        px = over(px, c);
    }
    // hot spot where the beam hits
    let hs = ((x - cx).powi(2) + (y - (cy - b + 10.0)).powi(2)).sqrt();
    px = over(px, [1.0, 0.97, 0.9, (1.0 - hs / 40.0).max(0.0)]);
    px
}

fn main() {
    // 4×4 samples per pixel for smooth edges
    let s = 4;
    let mut img = RgbaImage::new(N, N);
    for py in 0..N {
        for pxl in 0..N {
            let mut acc = [0.0f32; 4];
            for j in 0..s {
                for i in 0..s {
                    let c = sample(pxl as f32 + (i as f32 + 0.5) / s as f32, py as f32 + (j as f32 + 0.5) / s as f32);
                    // premultiply while averaging
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
    img.save("assets/icon.png").expect("write assets/icon.png");

    // Windows .ico with the usual sizes
    let frames: Vec<image::codecs::ico::IcoFrame> = [256u32, 128, 64, 48, 32, 16]
        .iter()
        .map(|&size| {
            let small = image::imageops::resize(&img, size, size, FilterType::Lanczos3);
            image::codecs::ico::IcoFrame::as_png(small.as_raw(), size, size, image::ExtendedColorType::Rgba8).expect("ico frame")
        })
        .collect();
    let file = std::fs::File::create("assets/icon.ico").expect("create assets/icon.ico");
    image::codecs::ico::IcoEncoder::new(file).encode_images(&frames).expect("write assets/icon.ico");
    println!("Wrote assets/icon.png (1024×1024) and assets/icon.ico (16–256 px)");
}
