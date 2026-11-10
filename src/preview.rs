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
}

impl Preview {
    pub fn new(job: Job, gcode: String, doc: Option<Document>, device: &Device) -> Self {
        let secs = job.move_seconds(device.rapid_speed, device.accel);
        let mut ends = Vec::with_capacity(secs.len());
        let (mut t, mut cut) = (0.0, 0.0);
        for (m, s) in job.moves.iter().zip(&secs) {
            t += s;
            if matches!(m.kind, MoveKind::Cut) {
                cut += s;
            }
            ends.push(t);
        }
        Self { job, gcode, progress: 1.0, doc, ends, total: t, cut_time: cut, playing: false, speed: PLAY_SPEED_DEFAULT, show_traversals: false, shade_by_power: false, invert: false, show_legend: true }
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

fn ink(pv: &Preview, power: f64) -> Color32 {
    let (bg, fg) = if pv.invert { (INK, PAGE) } else { (PAGE, INK) };
    if pv.shade_by_power {
        let k = (power / 100.0).clamp(0.0, 1.0) as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k) as u8;
        Color32::from_rgb(mix(bg.r(), fg.r()), mix(bg.g(), fg.g()), mix(bg.b(), fg.b()))
    } else {
        fg
    }
}

fn legend_row(ui: &mut egui::Ui, fill: Color32, text: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, fill);
        ui.painter().rect_stroke(r, 2.0, Stroke::new(1.5, Color32::BLACK), egui::StrokeKind::Inside);
        ui.add_space(8.0);
        ui.label(egui::RichText::new(text).size(16.0));
    });
}

pub fn window(app: &mut BeamForge, ctx: &egui::Context) {
    let Some(mut pv) = app.preview.take() else { return };
    let doc = pv.doc.clone().unwrap_or_else(|| app.doc.clone());
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
            // legend on the left (cbShowLegend); "Sposta" only while traversals are shown
            ui.allocate_ui(vec2(170.0, area_h), |ui| {
                ui.set_width(170.0);
                if pv.show_legend {
                    legend_row(ui, ink(&pv, 100.0), tr("Taglia"));
                    if pv.show_traversals {
                        legend_row(ui, TRAVEL, tr("Sposta"));
                    }
                }
            });
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), area_h), Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 0.0, if pv.invert { INK } else { PAGE });
            let Some(b) = pv.job.bounds else {
                p.text(rect.center(), egui::Align2::CENTER_CENTER, tr("Niente da incidere: aggiungi forme su un livello con Emissione attiva."), egui::FontId::proportional(14.0), TEXT);
                return;
            };
            // the view frames the job itself (the machine origin may be far away)
            let bb = b;
            let z =((rect.width() - 40.0) / bb.w().max(1.0) as f32).min((rect.height() - 40.0) / bb.h().max(1.0) as f32);
            let c = bb.center();
            let to = |q: P2| pos2(rect.center().x + (q[0] - c[0]) as f32 * z, rect.center().y - (q[1] - c[1]) as f32 * z);
            let n = pv.done();
            let mut last = pv.job.start;
            for m in pv.job.moves.iter().take(n) {
                match m.kind {
                    MoveKind::Cut => {
                        let w = burn_width(&doc, m.layer).map(|w| (w as f32 * z).max(1.0)).unwrap_or(1.2);
                        p.line_segment([to(last), to(m.to)], Stroke::new(w, ink(&pv, m.power)));
                    }
                    _ if pv.show_traversals => {
                        p.line_segment([to(last), to(m.to)], Stroke::new(0.8, TRAVEL));
                    }
                    _ => {}
                }
                last = m.to;
            }
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
            ui.checkbox(&mut pv.show_traversals, tr("Mostra movimenti trasversali"));
            ui.checkbox(&mut pv.shade_by_power, tr("Ombreggia in base alla potenza"));
            ui.checkbox(&mut pv.invert, tr("Inverti"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(clock(pv.progress as f64 * pv.total));
            });
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut pv.show_legend, tr("Mostra legenda"));
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
        save_png(&pv, &doc);
    }
    if let Some(start) = from_here {
        let part = pv.job.from_move(pv.done());
        let opts = crate::gcode::JobOptions { selected_only: false, ..app.job_opts.clone() };
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

/// "Salva immagine" (on_pbSaveImage_clicked 0x100da2df4): the burn as a PNG, 10 px per mm
/// (at most 4000 px a side; estimated resolution).
fn save_png(pv: &Preview, doc: &Document) {
    let Some(b) = pv.job.bounds else { return };
    let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("anteprima.png").save_file() else { return };
    let margin = 2.0;
    let scale = (10.0f64).min(4000.0 / (b.w() + 2.0 * margin)).min(4000.0 / (b.h() + 2.0 * margin));
    let (w, h) = (((b.w() + 2.0 * margin) * scale).ceil() as u32, ((b.h() + 2.0 * margin) * scale).ceil() as u32);
    let bg = if pv.invert { INK } else { PAGE };
    let mut img = image::RgbaImage::from_pixel(w.max(1), h.max(1), image::Rgba([bg.r(), bg.g(), bg.b(), 255]));
    let px = |q: P2| -> (f64, f64) { ((q[0] - b.min[0] + margin) * scale, (b.max[1] - q[1] + margin) * scale) };
    let n = pv.done();
    let mut last = pv.job.start;
    for m in pv.job.moves.iter().take(n) {
        let show = matches!(m.kind, MoveKind::Cut) || pv.show_traversals;
        if show {
            let col = if matches!(m.kind, MoveKind::Cut) { ink(pv, m.power) } else { TRAVEL };
            let width = if matches!(m.kind, MoveKind::Cut) { burn_width(doc, m.layer).map(|w| w * scale).unwrap_or(1.5).max(1.0) } else { 1.0 };
            stroke(&mut img, px(last), px(m.to), width, col);
        }
        last = m.to;
    }
    let _ = img.save(path);
}

fn stroke(img: &mut image::RgbaImage, a: (f64, f64), b: (f64, f64), width: f64, c: Color32) {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let steps = (len * 2.0).ceil().max(1.0) as usize;
    let r = (width / 2.0).max(0.5);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let (x0, x1) = ((x - r).floor().max(0.0) as u32, (x + r).ceil().min(img.width() as f64 - 1.0).max(0.0) as u32);
        let (y0, y1) = ((y - r).floor().max(0.0) as u32, (y + r).ceil().min(img.height() as f64 - 1.0).max(0.0) as u32);
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                img.put_pixel(xx, yy, image::Rgba([c.r(), c.g(), c.b(), 255]));
            }
        }
    }
}
