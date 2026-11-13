//! Measures the running code against app/mechanics.json: each test reads a value from the sheet
//! and compares it with what the program actually computes or uses (`cargo test mechanics`).
//! This checks BeamForge against its own sheet; it can't compare with the original running.

use crate::model::{Geometry, Shape};

fn sheet() -> serde_json::Value {
    serde_json::from_str(include_str!("../mechanics.json")).expect("mechanics.json is valid JSON")
}

/// The number the sheet gives for `name`.
fn num(name: &str) -> f64 {
    let s = sheet();
    let v = s["values"].as_array().unwrap().iter().find(|v| v["name"] == name).unwrap_or_else(|| panic!("{name} missing from mechanics.json"));
    v["value"].as_f64().unwrap_or_else(|| panic!("{name} is not a number"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

#[test]
fn zoom_values_match_sheet() {
    assert!(close(crate::canvas::ZOOM_IN_FACTOR as f64, num("zoom in factor")));
    assert!(close(crate::canvas::ZOOM_OUT_FACTOR as f64, num("zoom out factor")));
    assert!(close(crate::canvas::ZOOM_MIN as f64, num("zoom min")));
    assert!(close(crate::canvas::ZOOM_MAX as f64, num("zoom max")));
}

/// Shift on the line tool: every snapped segment lies on a multiple of the sheet's step, and a
/// diagonal takes the longer side (measured over a fan of pointer positions).
#[test]
fn line_shift_snap_matches_sheet() {
    let step = num("line tool Shift snap angle step");
    let a = [10.0, 20.0];
    for i in 0..360 {
        let t = (i as f64).to_radians();
        let r = 30.0 + (i % 7) as f64 * 5.0;
        let b = [a[0] + r * t.cos(), a[1] + r * t.sin()];
        let s = crate::canvas::line_axis_snap(a, b);
        let (dx, dy) = (s[0] - a[0], s[1] - a[1]);
        let ang = dy.atan2(dx).to_degrees();
        let k = ang / step;
        assert!((k - k.round()).abs() < 1e-6, "pointer at {i}° snapped to {ang}°, not a multiple of {step}°");
        if dx.abs() > 1e-9 && dy.abs() > 1e-9 {
            let (ox, oy) = (b[0] - a[0], b[1] - a[1]);
            assert!(close(dx.abs(), ox.abs().max(oy.abs())), "diagonal length is not max(|dx|, |dy|)");
        }
    }
}

#[test]
fn rotate_handles_match_sheet() {
    let off = num("rotate handle offset from corner") as f32;
    let corner = eframe::egui::pos2(300.0, 100.0);
    let mid = eframe::egui::pos2(200.0, 200.0);
    let h = crate::canvas::rotate_handle_pos(corner, mid);
    assert_eq!((h.x - corner.x, h.y - corner.y), (off, -off));
    assert!(close(crate::canvas::ROTATE_GRAB_PX as f64, num("rotate arrow grab distance")));
}

/// Turning a shape keeps its size: a 120 × 70 rectangle turned 90° measures 70 × 120.
#[test]
fn rotation_keeps_size() {
    let mut s = Shape::new(0, Geometry::Rect { w: 120.0, h: 70.0, radius: 0.0 }, 0, 50.0, 50.0);
    let b0 = s.bbox();
    s.rotate_about(90.0, b0.center());
    let b = s.bbox();
    assert!(close(b.w(), b0.h()) && close(b.h(), b0.w()), "size changed: {} × {}", b.w(), b.h());
    s.rotate_about(-90.0, b0.center());
    let b = s.bbox();
    assert!(close(b.w(), b0.w()) && close(b.h(), b0.h()));
}

#[test]
fn window_and_update_values_match_sheet() {
    assert!(close(crate::panels::MAC_WINDOW_BUTTONS_WIDTH as f64, num("macOS window buttons room")));
    assert!(close(crate::panels::RESIZE_BAND_PX as f64, num("frameless resize band")));
    assert!(close(crate::update::AUTO_CHECK_DELAY_S, num("auto update check delay")));
    assert!(close(crate::update::DOWNLOAD_TIMEOUT_S.parse::<f64>().unwrap(), num("update download timeout")));
}

/// The version the sheet records is the one the program reports.
#[test]
fn version_matches_sheet() {
    let s = sheet();
    let v = s["values"].as_array().unwrap().iter().find(|v| v["name"] == "current version").unwrap();
    assert_eq!(v["value"].as_str().unwrap(), crate::update::current());
}
