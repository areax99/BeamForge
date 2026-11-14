//! The Anteprima window, like the original's PreviewDialog: the job drawn as it will burn on a
//! light page (fills as solid areas), a legend, cut / rapid distances with their times, playback
//! at 0.1×–40×, shading by power, invert, "Comincia qui" and saving the picture.
//! Rules: UpdateCutStats 0x100da2364, on_hsPlaySpeed_valueChanged 0x100da3878, timerEvent
//! 0x100da372c, _updateLegend 0x100da15a4. See MECHANICS.md, "Anteprima".

use crate::app::BeamForge;
use crate::gcode::{Job, MoveKind};
use crate::i18n::{tr, trf};
use crate::model::*;
use crate::theme;
use eframe::egui::{self, Color32, Sense, Stroke, pos2, vec2};

/// Playback speeds for the slider positions −3..=5 (on_hsPlaySpeed_valueChanged 0x100da3878).
pub const PLAY_SPEEDS: [f64; 9] = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 40.0];
/// Index of 1× in PLAY_SPEEDS (slider position 0).
pub const PLAY_SPEED_DEFAULT: usize = 3;

const PAGE: Color32 = Color32::from_rgb(236, 239, 241);
const INK: Color32 = Color32::from_rgb(20, 20, 22);
/// Traversal colour of the legend's "Sposta" swatch.
const TRAVEL: Color32 = Color32::from_rgb(244, 150, 150);

pub struct Preview {
    pub job: Job,
    pub gcode: String,
    /// 0..1 of the job's time
    pub progress: f32,
    /// A test's own document (Test intervallo / materiale); None previews the project.
    pub doc: Option<Document>,
    /// seconds at the end of each move
    ends: Vec<f64>,
    total: f64,
    cut_time: f64,
    playing: bool,
    speed: usize,
    show_traversals: bool,
    shade_by_power: bool,
    invert: bool,
    show_legend: bool,
    /// one legend row per kind of work in the job (Taglia, Riempimento...), in cut order: its
    /// colour and its words
    legend: Vec<(Vec<Color32>, String)>,
    /// burns drawn in their layers' colours (the project); false for a test's own document
    colored: bool,
    /// the burn drawn as a picture (see `Raster`)
    raster: Option<Raster>,
    /// 1 = the whole job fits the view
    view_zoom: f32,
    /// job point at the middle of the view (None: the job's centre)
    view_center: Option<P2>,
}

impl Preview {
    /// `project` is the open document; a test's own `doc` is used instead when given.
    pub fn new(job: Job, gcode: String, doc: Option<Document>, project: &Document) -> Self {
        let layers_doc = doc.as_ref().unwrap_or(project);
        // the kinds of work, each with the layers doing it, in the order they first burn
        let colored = doc.is_none();
        let mut kinds: Vec<(CutMode, Vec<usize>)> = Vec::new();
        for m in &job.moves {
            if !matches!(m.kind, MoveKind::Cut | MoveKind::Dot { .. }) {
                continue;
            }
            let mode = layers_doc.layers.get(m.layer).map(|cs| layers_doc.layer_art(m.layer).clamp(cs.mode)).unwrap_or(CutMode::Line);
            match kinds.iter_mut().find(|(k, _)| *k == mode) {
                Some((_, ls)) if !ls.contains(&m.layer) => ls.push(m.layer),
                Some(_) => {}
                None => kinds.push((mode, vec![m.layer])),
            }
        }
        let legend: Vec<(Vec<Color32>, String)> = kinds
            .iter()
            .map(|(mode, ls)| {
                // the project: the swatch split in its layers' colours, their names after the
                // words (at most four; more are counted); a test: the ink and the words only
                let mut inks: Vec<Color32> = Vec::new();
                for &l in ls {
                    let c = layer_ink(l, false, colored);
                    if !inks.contains(&c) {
                        inks.push(c);
                    }
                }
                let text = if colored {
                    let mut names: Vec<String> = ls.iter().take(4).map(|&l| crate::model::layer_name(l)).collect();
                    if ls.len() > 4 {
                        names.push(format!("+{}", ls.len() - 4));
                    }
                    format!("{} ({})", kind_it(*mode), names.join(", "))
                } else {
                    kind_it(*mode).to_string()
                };
                (inks, text)
            })
            .collect();
        let secs = job.move_seconds_sim(&crate::gcode::Job::sim_for(layers_doc));
        let mut ends = Vec::with_capacity(secs.len());
        let (mut t, mut cut) = (0.0, 0.0);
        for (m, s) in job.moves.iter().zip(&secs) {
            t += s;
            if matches!(m.kind, MoveKind::Cut | MoveKind::Dot { .. }) {
                cut += s;
            }
            ends.push(t);
        }
        Self { job, gcode, progress: 1.0, doc, ends, total: t, cut_time: cut, playing: false, speed: PLAY_SPEED_DEFAULT, show_traversals: false, shade_by_power: false, invert: false, show_legend: true, legend, colored, raster: None, view_zoom: 1.0, view_center: None }
    }

    /// Zoom the view in on the job's middle (1 = whole job).
    pub fn zoom_to(&mut self, zoom: f32) {
        self.view_zoom = zoom.clamp(1.0, PREVIEW_MAX_ZOOM);
        self.view_center = None;
    }

    /// Moves done at the current progress.
    fn done(&self) -> usize {
        if self.progress >= 1.0 {
            return self.job.moves.len();
        }
        let t = self.progress as f64 * self.total;
        self.ends.partition_point(|e| *e <= t)
    }
}

/// "m:ss" or "h:mm:ss", as the original's preview writes times.
fn clock(secs: f64) -> String {
    let s = secs.round().max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// Width in mm a burning move leaves: the scan interval for fills and images, else a hairline.
fn burn_width(doc: &Document, layer: usize) -> Option<f64> {
    doc.layers.get(layer).and_then(|cs| match cs.mode {
        CutMode::Fill | CutMode::OffsetFill | CutMode::Image => Some(cs.interval.max(0.01)),
        CutMode::Line => None,
    })
}

/// The colour a layer burns in on the preview: the layer's own colour (the person's request, so
/// a black cut layer and a red fill layer can be told apart), with very dark colours drawn in
/// the preview's ink. With Inverti, dark colours turn light so they show on the dark page.
/// Test documents (Test materiale / intervallo) burn in the ink only, as in the original: their
/// many layers are not the project's coloured layers.
fn layer_ink(layer: usize, invert: bool, colored: bool) -> Color32 {
    if !colored || layer >= crate::model::CUT_LAYER_COUNT {
        return if invert { PAGE } else { INK };
    }
    let c = crate::canvas::layer_color(layer);
    // only (near) black counts as dark: by brightness a pure blue (0, 0, 255) is dark too, and
    // the blue layer C01 came out black
    if c.r().max(c.g()).max(c.b()) < 60 {
        if invert { PAGE } else { INK }
    } else {
        c
    }
}

/// What a layer does, as the legend says it.
fn kind_it(mode: CutMode) -> &'static str {
    match mode {
        CutMode::Line => tr("Taglia"),
        CutMode::Fill => tr("Riempimento"),
        CutMode::OffsetFill => tr("Riempimento offset"),
        CutMode::Image => tr("Immagine"),
    }
}

/// A legend entry: a square swatch (split side by side when several colours share the entry)
/// and its words.
fn legend_row(ui: &mut egui::Ui, fills: &[Color32], text: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
        let n = fills.len().max(1) as f32;
        for (i, c) in fills.iter().enumerate() {
            let part = egui::Rect::from_min_max(pos2(r.left() + r.width() * i as f32 / n, r.top()), pos2(r.left() + r.width() * (i + 1) as f32 / n, r.bottom()));
            ui.painter().rect_filled(part, 0.0, *c);
        }
        ui.painter().rect_stroke(r, 2.0, Stroke::new(1.5, Color32::BLACK), egui::StrokeKind::Inside);
        ui.add_space(8.0);
        ui.label(egui::RichText::new(text).size(16.0));
    });
}

pub fn window(app: &mut BeamForge, ctx: &egui::Context) {
    let Some(mut pv) = app.preview.take() else { return };
    // only the burn widths are needed while drawing; the document itself (with its pictures) is
    // no longer copied on every frame
    let widths: Vec<Option<f64>> = {
        let d = pv.doc.as_ref().unwrap_or(&app.doc);
        (0..d.layers.len()).map(|l| burn_width(d, l)).collect()
    };
    let mut open = true;
    let mut close = false;
    let mut from_here: Option<bool> = None; // Some(true) start, Some(false) save G-code
    let mut save_image = false;
    // playback: the slider moves through the job's time at the chosen speed (timerEvent 0x100da372c)
    if pv.playing {
        let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
        if pv.total > 0.0 {
            pv.progress = (pv.progress as f64 + dt * PLAY_SPEEDS[pv.speed] / pv.total).min(1.0) as f32;
        }
        if pv.progress >= 1.0 {
            pv.playing = false;
        }
        ctx.request_repaint();
    }
    theme::centered(ctx, "preview", egui::Window::new(tr("Anteprima")).open(&mut open).default_size(vec2(1100.0, 760.0)).resizable(true).collapsible(false)).show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let footer = 110.0;
        let area_h = (ui.available_height() - footer).max(260.0);
        ui.horizontal_top(|ui| {
            // legend on the left (cbShowLegend)
            // one entry under the other, in a column wide enough for "Riempimento offset (C29)"
            ui.allocate_ui_with_layout(vec2(LEGEND_W, area_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(LEGEND_W);
                if pv.show_legend {
                    // one row per layer that burns, in its colour, saying what it does
                    for (cols, text) in &pv.legend {
                        let cols: Vec<Color32> = cols.iter().map(|c| if pv.invert && *c == INK { PAGE } else { *c }).collect();
                        legend_row(ui, &cols, text);
                    }
                    // the original's legend always has Sposta, in the travel colour
                    legend_row(ui, &[TRAVEL], tr("Sposta"));
                }
            });
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), area_h), Sense::click_and_drag());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 0.0, if pv.invert { INK } else { PAGE });
            let Some(b) = pv.job.bounds else {
                p.text(rect.center(), egui::Align2::CENTER_CENTER, tr("Niente da incidere: aggiungi forme su un livello con Emissione attiva."), egui::FontId::proportional(14.0), TEXT);
                return;
            };
            // the view frames the job itself (the machine origin may be far away); the wheel zooms
            // in on the pointer, dragging moves the view, a double click frames the job again
            let bb = b;
            let z_fit = ((rect.width() - 40.0) / bb.w().max(1.0) as f32).min((rect.height() - 40.0) / bb.h().max(1.0) as f32);
            let mut c = pv.view_center.unwrap_or(bb.center());
            if let Some(hp) = resp.hover_pos() {
                let (scroll, zd) = ui.ctx().input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                let factor = zd * (scroll * 0.004).exp();
                if (factor - 1.0).abs() > 1e-4 {
                    let z_old = z_fit * pv.view_zoom;
                    let new_zoom = (pv.view_zoom * factor).clamp(1.0, PREVIEW_MAX_ZOOM);
                    let z_new = z_fit * new_zoom;
                    // keep the job point under the pointer where it is
                    let d = hp - rect.center();
                    let q = [c[0] + (d.x / z_old) as f64, c[1] - (d.y / z_old) as f64];
                    c = [q[0] - (d.x / z_new) as f64, q[1] + (d.y / z_new) as f64];
                    pv.view_zoom = new_zoom;
                }
            }
            if resp.dragged() {
                let d = resp.drag_delta();
                let zz = z_fit * pv.view_zoom;
                c = [c[0] - (d.x / zz) as f64, c[1] + (d.y / zz) as f64];
            }
            if resp.double_clicked() || pv.view_zoom <= 1.0 {
                pv.view_zoom = 1.0;
                c = bb.center();
            }
            pv.view_center = Some(c);
            let z = z_fit * pv.view_zoom;
            let to = |q: P2| pos2(rect.center().x + (q[0] - c[0]) as f32 * z, rect.center().y - (q[1] - c[1]) as f32 * z);
            let n = pv.done();
            // The burn is drawn into one picture, not as one shape per move: a photo engraving has
            // about a million moves, and drawing them as shapes every frame made the preview crawl
            // and, with big pictures, run out of graphics memory and close the app. The picture
            // has one pixel per scan line (the engraving's own resolution), so the pattern of the
            // chosen image mode (dithering dots, halftone, newsprint, dots of Dot mode...) is
            // really in it and shows when zooming in. It is only added to while playing, and
            // redrawn when a switch changes.
            let view = raster_view(&bb, &widths, &pv.job);
            // a cut line is drawn about 1.5 screen pixels wide when the whole job is in view
            let raster_per_screen = view.scale / (z_fit as f64 * ui.ctx().pixels_per_point() as f64).max(1e-6);
            let hair_r = (0.75 * raster_per_screen).max(0.5);
            update_raster(&mut pv, &widths, view, hair_r, n, ui.ctx());
            if let Some(tex) = pv.raster.as_ref().and_then(|r| r.tex.as_ref()) {
                let (hw, hh) = (view.w as f64 / view.scale * 0.5, view.h as f64 / view.scale * 0.5);
                let img_rect = egui::Rect::from_two_pos(to([view.cx - hw, view.cy + hh]), to([view.cx + hw, view.cy - hh]));
                p.image(tex.id(), img_rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if pv.view_zoom > 1.0 {
                p.text(rect.right_top() + vec2(-8.0, 8.0), egui::Align2::RIGHT_TOP, format!("{:.0}×", pv.view_zoom), egui::FontId::proportional(13.0), TEXT);
            }
            let last = if n == 0 { pv.job.start } else { pv.job.moves[n - 1].to };
            // where the head is: a red cross
            let h = to(last);
            let red = Stroke::new(2.0, Color32::from_rgb(230, 20, 20));
            p.line_segment([h - vec2(12.0, 0.0), h + vec2(12.0, 0.0)], red);
            p.line_segment([h - vec2(0.0, 12.0), h + vec2(0.0, 12.0)], red);
        });
        let w = ui.available_width();
        let slider = egui::Slider::new(&mut pv.progress, 0.0..=1.0).show_value(false);
        ui.spacing_mut().slider_width = w - 10.0;
        if ui.add(slider).changed() {
            pv.playing = false;
        }
        // UpdateCutStats 0x100da2364: distances rounded to whole mm, times in brackets
        let rapid_time = (pv.total - pv.cut_time).max(0.0);
        ui.vertical_centered(|ui| {
            ui.label(trf("Distanza taglio: {} mm (~{})   Spostamenti rapidi: {} mm (~{})   Tempo totale stimato: {}", &[
                &format!("{:.0}", pv.job.cut_length()),
                &clock(pv.cut_time),
                &format!("{:.0}", pv.job.rapid_length()),
                &clock(rapid_time),
                &clock(pv.total),
            ]));
        });
        ui.horizontal(|ui| {
            ui.label(tr("Velocità di riproduzione"));
            let mut s = pv.speed as i32 - PLAY_SPEED_DEFAULT as i32;
            ui.spacing_mut().slider_width = 60.0;
            if ui.add(egui::Slider::new(&mut s, -3..=5).show_value(false)).changed() {
                pv.speed = (s + PLAY_SPEED_DEFAULT as i32) as usize;
            }
            let sp = PLAY_SPEEDS[pv.speed];
            ui.label(if sp >= 1.0 { format!("{:.1}x", sp) } else { format!("{}x", sp) });
            ui.add_space(16.0);
            crate::panels::switch(ui, &mut pv.show_traversals, tr("Mostra movimenti trasversali"));
            crate::panels::switch(ui, &mut pv.shade_by_power, tr("Ombreggia in base alla potenza"));
            crate::panels::switch(ui, &mut pv.invert, tr("Inverti"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(clock(pv.progress as f64 * pv.total));
            });
        });
        ui.horizontal(|ui| {
            crate::panels::switch(ui, &mut pv.show_legend, tr("Mostra legenda"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized(vec2(110.0, 28.0), egui::Button::new("Ok")).clicked() {
                    close = true;
                }
                let play = if pv.playing { tr("Pausa") } else { tr("Riproduci") };
                if ui.add_sized(vec2(110.0, 28.0), egui::Button::new(play)).clicked() {
                    if !pv.playing && pv.progress >= 1.0 {
                        pv.progress = 0.0;
                    }
                    pv.playing = !pv.playing;
                }
                if ui.add_sized(vec2(130.0, 28.0), egui::Button::new(tr("Salva immagine"))).clicked() {
                    save_image = true;
                }
                // start, or save, the job from the slider's point on (disabled at the end)
                let can = pv.done() < pv.job.moves.len();
                ui.add_enabled_ui(can, |ui| {
                    ui.menu_button(tr("Comincia qui"), |ui| {
                        if ui.button(tr("Avvia da qui")).clicked() {
                            from_here = Some(true);
                            ui.close();
                        }
                        if ui.button(tr("Salva G-code da qui")).clicked() {
                            from_here = Some(false);
                            ui.close();
                        }
                    });
                });
            });
        });
    });
    if save_image {
        save_png(&pv, &widths);
    }
    if let Some(start) = from_here {
        let part = pv.job.from_move(pv.done());
        let opts = crate::gcode::JobOptions { selected_only: false, ..app.job_opts.clone() };
        let doc = pv.doc.clone().unwrap_or_else(|| app.doc.clone());
        let g = crate::gcode::write_gcode(&doc, &part, &opts);
        if start {
            app.send_job(part, g);
        } else {
            app.write_gcode_file(part, g, "da-qui.gc");
        }
    }
    if open && !close {
        app.preview = Some(pv);
    }
}

const TEXT: Color32 = Color32::from_rgb(90, 90, 96);
/// Width of the legend column, points.
const LEGEND_W: f32 = 250.0;

/// "Salva immagine" (on_pbSaveImage_clicked 0x100da2df4): the burn as a PNG, 10 px per mm
/// (at most 4000 px a side; estimated resolution).
fn save_png(pv: &Preview, widths: &[Option<f64>]) {
    let Some(b) = pv.job.bounds else { return };
    let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("anteprima.png").save_file() else { return };
    // the same picture as on screen: one pixel per scan line, so the image mode's pattern is in it
    let view = raster_view(&b, widths, &pv.job);
    let mut r = Raster::new(view, pv, 0.75);
    r.add(pv, widths, 0, pv.done());
    let px = r.compose(pv);
    let mut img = image::RgbaImage::new(view.w as u32, view.h as u32);
    for (i, p) in px.iter().enumerate() {
        img.put_pixel((i % view.w) as u32, (i / view.w) as u32, image::Rgba([p.r(), p.g(), p.b(), 255]));
    }
    let _ = img.save(path);
}

/// Longest side of the preview picture, in pixels (a 400 mm job at 0.1 mm fits whole).
const PREVIEW_MAX_PX: f64 = 4096.0;
/// Fewest pixels on the long side, so line jobs stay sharp enough.
const PREVIEW_MIN_PX: f64 = 1600.0;
/// How far the preview zooms in.
const PREVIEW_MAX_ZOOM: f32 = 40.0;
/// Room around the job in the picture, mm.
const PREVIEW_MARGIN_MM: f64 = 2.0;

/// Up close the picture's own pixels stay square (each dot of the pattern visible); from afar
/// they are averaged, as the eye averages the dots on the material.
const BURN_TEXTURE: egui::TextureOptions = egui::TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Linear,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
    mipmap_mode: Some(egui::TextureFilter::Linear),
};

/// The preview picture's size and scale: one pixel per scan line of the finest image or fill
/// layer in the job (their interval), within PREVIEW_MIN_PX..PREVIEW_MAX_PX on the long side.
fn raster_view(b: &BBox, widths: &[Option<f64>], job: &Job) -> RasterView {
    let finest = job
        .moves
        .iter()
        .filter(|m| !matches!(m.kind, MoveKind::Rapid | MoveKind::Travel))
        .filter_map(|m| widths.get(m.layer).copied().flatten())
        .fold(f64::INFINITY, f64::min);
    let (wmm, hmm) = (b.w() + 2.0 * PREVIEW_MARGIN_MM, b.h() + 2.0 * PREVIEW_MARGIN_MM);
    let long = wmm.max(hmm).max(1.0);
    let wanted = if finest.is_finite() && finest > 0.0 { 1.0 / finest } else { 0.0 };
    let scale = wanted.max(PREVIEW_MIN_PX / long).min(PREVIEW_MAX_PX / long);
    // a whole number of pixels before the job's top-left corner, so each scan line falls on
    // the middle of one row of pixels (no beating between lines and rows)
    let margin_px = (PREVIEW_MARGIN_MM * scale).round();
    let (w, h) = ((b.w() * scale + 2.0 * margin_px).ceil().max(1.0), (b.h() * scale + 2.0 * margin_px).ceil().max(1.0));
    let (left, top) = (b.min[0] - margin_px / scale, b.max[1] + margin_px / scale);
    RasterView { w: w as usize, h: h as usize, scale, cx: left + w / (2.0 * scale), cy: top - h / (2.0 * scale) }
}

/// Where the job sits in a preview picture: its size in pixels, pixels per mm, and the job point
/// at its centre.
#[derive(Clone, Copy, PartialEq, Debug)]
struct RasterView {
    w: usize,
    h: usize,
    scale: f64,
    cx: f64,
    cy: f64,
}

/// The burn as a picture. Each pixel keeps how much of it is burned (0..1): a scan line thinner
/// than a pixel adds its share, so dithered pictures look grey from afar as they will on the
/// material, and a wider one fills the pixels under it.
pub struct Raster {
    view: RasterView,
    shade: bool,
    traversals: bool,
    invert: bool,
    /// moves already drawn
    done: usize,
    dark: Vec<f32>,
    travel: Vec<bool>,
    /// longest dot stop in the job, for scaling dots by their time
    max_dot_us: u32,
    /// which layer burned each pixel last, for its colour
    layer: Vec<u8>,
    /// layer of the move being drawn
    cur_layer: u8,
    /// half the width of a cut line (no scan interval) in pixels, so it stays visible on screen
    hair_r: f64,
    tex: Option<egui::TextureHandle>,
    /// the texture shows `dark` as of this many moves
    shown: Option<usize>,
}

impl Raster {
    fn new(view: RasterView, pv: &Preview, hair_r: f64) -> Self {
        let max_dot_us = pv.job.moves.iter().filter_map(|m| if let MoveKind::Dot { us } = m.kind { Some(us) } else { None }).max().unwrap_or(1).max(1);
        Self {
            view,
            shade: pv.shade_by_power,
            traversals: pv.show_traversals,
            invert: pv.invert,
            done: 0,
            dark: vec![0.0; view.w * view.h],
            travel: vec![false; view.w * view.h],
            max_dot_us,
            layer: vec![0; view.w * view.h],
            cur_layer: 0,
            hair_r,
            tex: None,
            shown: None,
        }
    }

    fn px(&self, q: P2) -> (f64, f64) {
        let v = &self.view;
        (v.w as f64 * 0.5 + (q[0] - v.cx) * v.scale, v.h as f64 * 0.5 - (q[1] - v.cy) * v.scale)
    }

    /// Draw moves `from..to` on top of what is there.
    fn add(&mut self, pv: &Preview, widths: &[Option<f64>], from: usize, to: usize) {
        let mut last = if from == 0 { pv.job.start } else { pv.job.moves[from - 1].to };
        for m in &pv.job.moves[from..to] {
            let k = if self.shade { (m.power / 100.0).clamp(0.0, 1.0) as f32 } else { 1.0 };
            let w_px = widths.get(m.layer).copied().flatten().map(|w| w * self.view.scale);
            self.cur_layer = m.layer.min(255) as u8;
            match m.kind {
                MoveKind::Cut => self.burn_line(self.px(last), self.px(m.to), w_px, k),
                MoveKind::Dot { us } => {
                    if self.traversals {
                        self.travel_line(self.px(last), self.px(m.to));
                    }
                    let t = us as f32 / self.max_dot_us as f32;
                    let d = w_px.unwrap_or(1.0);
                    let (x, y) = self.px(m.to);
                    if d >= 1.0 {
                        self.stamp(x, y, d * 0.5, k * t);
                    } else {
                        self.accumulate(x, y, k * t * (d * d) as f32);
                    }
                }
                _ if self.traversals => self.travel_line(self.px(last), self.px(m.to)),
                _ => {}
            }
            last = m.to;
        }
        self.done = to;
    }

    fn idx(&self, x: f64, y: f64) -> Option<usize> {
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let (xi, yi) = (x as usize, y as usize);
        (xi < self.view.w && yi < self.view.h).then(|| yi * self.view.w + xi)
    }

    fn accumulate(&mut self, x: f64, y: f64, a: f32) {
        if let Some(i) = self.idx(x, y) {
            self.dark[i] = (self.dark[i] + a).min(1.0);
            self.layer[i] = self.cur_layer;
        }
    }

    fn stamp(&mut self, x: f64, y: f64, r: f64, k: f32) {
        // the pixels whose centres are within r (at least the one under the point), so a line
        // one pixel wide fills one row, not two
        let r = r.max(0.5);
        let span = |c: f64| {
            let (lo, hi) = ((c - r - 0.5).ceil(), (c + r - 0.5).floor());
            if hi < lo { (c.floor(), c.floor()) } else { (lo, hi) }
        };
        let ((x0, x1), (y0, y1)) = (span(x), span(y));
        let (x0, x1) = (x0.max(0.0), x1.min(self.view.w as f64 - 1.0));
        let (y0, y1) = (y0.max(0.0), y1.min(self.view.h as f64 - 1.0));
        if x1 < x0 || y1 < y0 {
            return;
        }
        for yy in y0 as usize..=y1 as usize {
            for xx in x0 as usize..=x1 as usize {
                let i = yy * self.view.w + xx;
                if k >= self.dark[i] {
                    self.dark[i] = k;
                    self.layer[i] = self.cur_layer;
                }
            }
        }
    }

    /// A burning move: `w_px` is the burn width in pixels (None for a hairline cut line).
    fn burn_line(&mut self, a: (f64, f64), b: (f64, f64), w_px: Option<f64>, k: f32) {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let steps = (len * 2.0).ceil().max(1.0) as usize;
        let step = len / steps as f64;
        match w_px {
            Some(w) if w < 1.0 => {
                // thinner than a pixel: each half-pixel step adds the area it burns
                let a_step = (w * step) as f32 * k;
                for i in 0..steps {
                    let t = (i as f64 + 0.5) / steps as f64;
                    self.accumulate(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a_step);
                }
            }
            w => {
                let r = w.map(|w| w * 0.5).unwrap_or(self.hair_r);
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    self.stamp(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, r, k);
                }
            }
        }
    }

    fn travel_line(&mut self, a: (f64, f64), b: (f64, f64)) {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let steps = (len * 2.0).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            if let Some(j) = self.idx(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t) {
                self.travel[j] = true;
            }
        }
    }

    fn compose(&self, pv: &Preview) -> Vec<Color32> {
        let bg = if pv.invert { INK } else { PAGE };
        let inks: Vec<Color32> = (0..256).map(|l| layer_ink(l, pv.invert, pv.colored)).collect();
        let mix = |a: u8, b: u8, k: f32| (a as f32 + (b as f32 - a as f32) * k) as u8;
        self.dark
            .iter()
            .zip(&self.travel)
            .zip(&self.layer)
            .map(|((&d, &t), &l)| {
                if t && d < 0.5 {
                    TRAVEL
                } else {
                    let fg = inks[l as usize];
                    Color32::from_rgb(mix(bg.r(), fg.r(), d), mix(bg.g(), fg.g(), d), mix(bg.b(), fg.b(), d))
                }
            })
            .collect()
    }
}

/// Bring the preview picture up to `n` moves for this view, redrawing from scratch only when the
/// view or a switch changed (or the slider went back), and upload it when it changed.
fn update_raster(pv: &mut Preview, widths: &[Option<f64>], view: RasterView, hair_r: f64, n: usize, ctx: &egui::Context) {
    let fresh = match &pv.raster {
        Some(r) => r.view != view || (r.hair_r - hair_r).abs() > 0.25 || r.shade != pv.shade_by_power || r.traversals != pv.show_traversals || r.invert != pv.invert || n < r.done,
        None => true,
    };
    if fresh {
        let tex = pv.raster.take().and_then(|r| r.tex);
        let mut r = Raster::new(view, pv, hair_r);
        r.tex = tex;
        pv.raster = Some(r);
    }
    let mut r = pv.raster.take().unwrap();
    if n > r.done {
        r.add(pv, widths, r.done, n);
    }
    if r.shown != Some(r.done) {
        let img = egui::ColorImage { size: [view.w, view.h], source_size: vec2(view.w as f32, view.h as f32), pixels: r.compose(pv) };
        match r.tex.as_mut() {
            Some(t) if t.size() == [view.w, view.h] => t.set(img, BURN_TEXTURE),
            _ => r.tex = Some(ctx.load_texture("bf_preview_burn", img, BURN_TEXTURE)),
        }
        r.shown = Some(r.done);
    }
    pv.raster = Some(r);
}
