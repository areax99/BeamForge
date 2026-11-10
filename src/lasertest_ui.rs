//! The Test intervallo and Generatore test del materiale windows, laid out like the original's.

use crate::app::BeamForge;
use crate::i18n::{dv, tr, trf};
use crate::lasertest::*;
use crate::model::*;
use crate::theme;
use eframe::egui::{self, Color32, vec2};

/// Settings file for the interval test (the original keeps IntervalTest_* preferences).
const INTERVAL_FILE: &str = "interval_test.json";
const PRESETS_FILE: &str = "material_test_presets.json";
/// Names of the built-in presets, in the original's list order.
const BUILTIN_PRESETS: [&str; 4] = ["CO2 - Cut test", "CO2 - Engrave test", "Diode - Cut test", "Diode - Engrave test"];

impl BeamForge {
    /// MainWindow::on_actionInterval_Test_triggered 0x10068bbfc.
    pub fn open_interval_test(&mut self) {
        self.interval_test = Some(crate::i18n::load_json(INTERVAL_FILE).unwrap_or_default());
    }

    /// MainWindow::on_actionMaterial_Test_triggered 0x10068c29c.
    pub fn open_material_test(&mut self) {
        if self.material_test.is_none() {
            self.material_test = Some(MaterialTest::new(&self.doc.device));
        }
    }
}

fn big_icon(ui: &mut egui::Ui, ic: crate::icons::Ic, text: &str, w: f32, h: f32) -> bool {
    crate::icons::text_button(ui, ic, text, vec2(w, h), None, 14.0, "")
}

fn big(ui: &mut egui::Ui, text: &str, w: f32, h: f32) -> bool {
    ui.add_sized(vec2(w, h), egui::Button::new(text)).clicked()
}

/// Both test windows. Their buttons work on the test's own document, never on the project.
pub fn windows(app: &mut BeamForge, ctx: &egui::Context) {
    interval_window(app, ctx);
    material_window(app, ctx);
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Preview,
    Frame,
    Start,
    Pause,
    Stop,
    Save,
}

fn run(app: &mut BeamForge, doc: Document, act: Act, file: &str) {
    match act {
        Act::Preview => app.preview_test(doc),
        Act::Frame => app.frame_test(&doc),
        Act::Start => app.start_test(&doc),
        Act::Pause => app.pause_toggle(),
        Act::Stop => app.stop_job(),
        Act::Save => app.save_test_gcode(&doc, file),
    }
}

fn interval_window(app: &mut BeamForge, ctx: &egui::Context) {
    let Some(mut t) = app.interval_test.clone() else { return };
    let mut open = true;
    let mut act: Option<Act> = None;
    let mut close: Option<bool> = None;
    theme::centered(ctx, "interval_test", egui::Window::new(tr("Test di intervallo")).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        egui::Grid::new("it_grid").num_columns(6).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
            ui.label(tr("Velocità:"));
            ui.add_sized(vec2(110.0, 22.0), dv(&mut t.speed).speed(10.0).range(1.0..=100000.0).max_decimals(0));
            ui.label("mm/min");
            ui.label(tr("Intervallo min:"));
            ui.add_sized(vec2(80.0, 22.0), dv(&mut t.min_interval).speed(0.001).range(0.01..=5.0).min_decimals(2).max_decimals(3));
            ui.label("mm");
            ui.end_row();
            ui.label(tr("Potenza:"));
            ui.add_sized(vec2(110.0, 22.0), dv(&mut t.power).speed(0.5).range(0.0..=100.0).min_decimals(1).max_decimals(1));
            ui.label("%");
            ui.label(tr("Intervallo massimo:"));
            ui.add_sized(vec2(80.0, 22.0), dv(&mut t.max_interval).speed(0.001).range(0.01..=5.0).min_decimals(2).max_decimals(3));
            ui.label("mm");
            ui.end_row();
            ui.label(tr("Passi:"));
            ui.add_sized(vec2(110.0, 22.0), dv(&mut t.steps).range(1..=50));
            ui.label("");
            ui.label(tr("Dimensione:"));
            ui.add_sized(vec2(80.0, 22.0), dv(&mut t.size).speed(0.5).range(1.0..=100.0).max_decimals(1));
            ui.label("mm");
            ui.end_row();
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_space(90.0);
            ui.radio_value(&mut t.dithered, false, tr("Riempimento semplice"));
            ui.add_space(60.0);
            ui.radio_value(&mut t.dithered, true, tr("Immagine dithering"));
        });
        ui.add_space(6.0);
        let w = (ui.available_width() - 16.0) / 3.0;
        ui.horizontal(|ui| {
            if big(ui, tr("Anteprima"), w, 30.0) {
                act = Some(Act::Preview);
            }
            if big(ui, tr("Frame"), w, 30.0) {
                act = Some(Act::Frame);
            }
            if big_icon(ui, crate::icons::Ic::Play, tr("Avvio"), w, 30.0) {
                act = Some(Act::Start);
            }
        });
        let w2 = (ui.available_width() - 8.0) / 2.0;
        ui.horizontal(|ui| {
            // "Invia" uploads to the controller's memory; a GRBL laser has none, so it runs the job
            if crate::icons::text_button(ui, crate::icons::Ic::Upload, tr("Invia"), vec2(w2, 30.0), None, 14.0, tr("Invia il test al laser e avvialo")) {
                act = Some(Act::Start);
            }
            if big(ui, tr("Salva"), w2, 30.0) {
                act = Some(Act::Save);
            }
        });
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 28.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_sized(vec2(90.0, 26.0), egui::Button::new(egui::RichText::new("OK").strong())).clicked() {
                close = Some(true);
            }
            if ui.add_sized(vec2(90.0, 26.0), egui::Button::new(crate::i18n::cancel())).clicked() {
                close = Some(false);
            }
        });
    });
    if !open {
        close = Some(false);
    }
    if let Some(a) = act {
        let doc = t.build(&app.doc);
        run(app, doc, a, "interval-test.gc");
    }
    match close {
        // OK keeps the values for next time (on_IntervalTest_accepted 0x100d387cc)
        Some(true) => {
            crate::i18n::save_json(INTERVAL_FILE, &t);
            app.interval_test = None;
        }
        Some(false) => app.interval_test = None,
        None => app.interval_test = Some(t),
    }
}

fn axis_box(ui: &mut egui::Ui, id: &str, a: &mut Axis, columns: bool, center_enabled: bool) {
    egui::Frame::new().fill(crate::theme::p().field).corner_radius(4.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        egui::Grid::new(id).num_columns(2).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
            ui.label(tr("Cont.:"));
            ui.add_sized(vec2(150.0, 22.0), dv(&mut a.count).range(1..=20));
            ui.end_row();
            ui.label(tr("Param.:"));
            let before = a.param;
            egui::ComboBox::from_id_salt((id, "param")).width(140.0).selected_text(a.param.label()).show_ui(ui, |ui| {
                // a GRBL laser has no frequency setting, so it isn't offered (showEvent removes it)
                for p in [MtParam::Speed, MtParam::Power, MtParam::Interval, MtParam::Passes] {
                    ui.selectable_value(&mut a.param, p, p.label());
                }
            });
            if a.param != before {
                // new parameter: a sensible range to start from (estimated)
                (a.min, a.max) = match a.param {
                    MtParam::Speed => (600.0, 6000.0),
                    MtParam::Power => (10.0, 100.0),
                    MtParam::Interval => (0.05, 0.2),
                    MtParam::Passes => (1.0, 10.0),
                    MtParam::Frequency => (20.0, 60.0),
                };
            }
            ui.end_row();
            let (range, dec, step) = match a.param {
                MtParam::Speed => (1.0..=100000.0, 0, 10.0),
                MtParam::Power => (0.0..=100.0, 1, 0.5),
                MtParam::Interval => (0.01..=5.0, 3, 0.001),
                MtParam::Passes => (1.0..=100.0, 0, 1.0),
                MtParam::Frequency => (1.0..=1000.0, 1, 1.0),
            };
            let unit = a.param.unit();
            ui.label(tr("Min:"));
            ui.add_sized(vec2(150.0, 22.0), dv(&mut a.min).speed(step).range(range.clone()).max_decimals(dec).suffix(format!(" {unit}")));
            ui.end_row();
            ui.label(tr("Max:"));
            ui.add_sized(vec2(150.0, 22.0), dv(&mut a.max).speed(step).range(range).max_decimals(dec).suffix(format!(" {unit}")));
            ui.end_row();
            ui.label(if columns { tr("Larghezza:") } else { tr("Altezza:") });
            ui.add_sized(vec2(150.0, 22.0), dv(&mut a.size).speed(0.1).range(0.5..=200.0).min_decimals(2).max_decimals(2).suffix(" mm"));
            ui.end_row();
            ui.label(if columns { tr("Centro X:") } else { tr("Centro Y:") });
            let r = ui.add_enabled_ui(center_enabled, |ui| ui.add_sized(vec2(150.0, 22.0), dv(&mut a.center).speed(0.5).range(-10000.0..=10000.0).max_decimals(1).suffix(" mm"))).inner;
            if !center_enabled {
                r.on_disabled_hover_text(tr("Per usare questa impostazione, l'origine deve essere 'Coordinate assolute'."));
            }
            ui.end_row();
        });
    });
}

/// Small editor for one of the three settings (00 materiale, 01 testo, 02 bordo).
fn setting_editor(ui: &mut egui::Ui, id: &str, cs: &mut CutSetting) {
    egui::Grid::new(id).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
        ui.label(tr("Modalità"));
        egui::ComboBox::from_id_salt((id, "mode")).selected_text(crate::panels::mode_it(cs.mode)).show_ui(ui, |ui| {
            for m in [CutMode::Line, CutMode::Fill] {
                ui.selectable_value(&mut cs.mode, m, crate::panels::mode_it(m));
            }
        });
        ui.end_row();
        ui.label(tr("Velocità (mm/min)"));
        ui.add(dv(&mut cs.speed).speed(10.0).range(1.0..=100000.0));
        ui.end_row();
        ui.label(tr("Potenza massima (%)"));
        if ui.add(dv(&mut cs.max_power).speed(0.5).range(0.0..=100.0)).changed() {
            cs.min_power = cs.min_power.min(cs.max_power);
        }
        ui.end_row();
        ui.label(tr("Potenza min. (%)"));
        ui.add(dv(&mut cs.min_power).speed(0.5).range(0.0..=100.0));
        ui.end_row();
        ui.label(tr("Numero passaggi"));
        ui.add(dv(&mut cs.passes).range(1..=100));
        ui.end_row();
        ui.label(tr("Intervallo (mm)"));
        ui.add(dv(&mut cs.interval).speed(0.001).range(0.01..=5.0).max_decimals(3));
        ui.end_row();
    });
}

/// "00 Modifica impostazione materiale": the layer number on its palette colour, then the words.
fn layer_button(ui: &mut egui::Ui, layer: usize, text: &str) -> bool {
    let bg = if layer == 0 { Color32::BLACK } else { crate::canvas::layer_color(layer) };
    let mut job = egui::text::LayoutJob::default();
    egui::RichText::new(format!(" {:02} ", layer)).color(Color32::WHITE).background_color(bg).append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
    egui::RichText::new(format!("  {text}")).append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
    ui.add_sized(vec2(270.0, 30.0), egui::Button::new(job)).clicked()
}

fn material_window(app: &mut BeamForge, ctx: &egui::Context) {
    let Some(mut t) = app.material_test.clone() else { return };
    let mut open = true;
    let mut act: Option<Act> = None;
    let mut close = false;
    // Centro X / Y only mean something with absolute coordinates (MaterialTest::showEvent 0x100d3fc18)
    let center_enabled = app.job_opts.start_from == crate::gcode::StartFrom::AbsoluteCoords;
    let (ow, oh) = t.output_size(&app.doc);
    let editing_id = egui::Id::new("mt_editing");
    let mut editing: Option<usize> = ctx.data(|d| d.get_temp::<Option<usize>>(editing_id)).flatten();
    let mut sel = app.material_preset_sel.clone();
    let mut save_preset = false;
    let mut delete_preset = false;
    theme::centered(ctx, "material_test", egui::Window::new(tr("Generatore test del materiale")).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.horizontal(|ui| {
            ui.label(tr("Impostazioni predefinite"));
            egui::ComboBox::from_id_salt("mt_preset").width(260.0).selected_text(sel.clone()).show_ui(ui, |ui| {
                ui.add_enabled(false, egui::Label::new(tr("-----Impostazioni predefinite integrate-----")));
                for (i, name) in BUILTIN_PRESETS.iter().enumerate() {
                    if ui.selectable_label(sel == *name, *name).clicked() {
                        sel = name.to_string();
                        t.preset(i);
                    }
                }
                ui.add_enabled(false, egui::Label::new(tr("-------Impostazioni predefinite utente-------")));
                for (name, p) in &app.material_presets {
                    if ui.selectable_label(&sel == name, name).clicked() {
                        sel = name.clone();
                        let (cx, cy) = (t.cols.center, t.rows.center);
                        t = p.clone();
                        t.cols.center = cx;
                        t.rows.center = cy;
                    }
                }
            });
            if crate::icons::button(ui, crate::icons::Ic::Save, vec2(28.0, 22.0), 14.0, tr("Salva come impostazione predefinita utente (usa il Titolo come nome)")) {
                save_preset = true;
            }
            if crate::icons::button(ui, crate::icons::Ic::Trash, vec2(28.0, 22.0), 14.0, tr("Elimina l'impostazione predefinita utente scelta")) {
                delete_preset = true;
            }
            ui.label(tr("Titolo"));
            ui.add(egui::TextEdit::singleline(&mut t.title).desired_width(260.0));
        });
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(tr("Verticale / Righe")).strong());
                axis_box(ui, "mt_rows", &mut t.rows, false, center_enabled);
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(tr("Orizzontale / Colonne")).strong());
                axis_box(ui, "mt_cols", &mut t.cols, true, center_enabled);
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(tr("Laser")).strong());
                egui::Frame::new().fill(crate::theme::p().field).corner_radius(4.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    egui::Grid::new("mt_laser").num_columns(2).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                        if big(ui, tr("Anteprima"), 120.0, 54.0) {
                            act = Some(Act::Preview);
                        }
                        if big(ui, tr("Frame"), 120.0, 54.0) {
                            act = Some(Act::Frame);
                        }
                        ui.end_row();
                        if big_icon(ui, crate::icons::Ic::Play, tr("Avvio"), 120.0, 54.0) {
                            act = Some(Act::Start);
                        }
                        if big_icon(ui, if app.paused { crate::icons::Ic::Play } else { crate::icons::Ic::Pause }, if app.paused { tr("Riprendi") } else { tr("Pausa") }, 120.0, 54.0) {
                            act = Some(Act::Pause);
                        }
                        ui.end_row();
                        if big_icon(ui, crate::icons::Ic::Stop, tr("Stop"), 120.0, 54.0) {
                            act = Some(Act::Stop);
                        }
                        if big(ui, tr("Salva"), 120.0, 54.0) {
                            act = Some(Act::Save);
                        }
                        ui.end_row();
                    });
                });
            });
        });
        ui.add_space(6.0);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 20.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n = |v: f64| format!("{:.1}", v).replace('.', &crate::i18n::decimal_char().to_string());
            ui.label(trf("Dimensione uscita: {} x {} mm", &[&n(ow), &n(oh)]));
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                if layer_button(ui, 0, tr("Modifica impostazione materiale")) {
                    editing = if editing == Some(0) { None } else { Some(0) };
                }
            });
            ui.vertical(|ui| {
                if layer_button(ui, 1, tr("Modifica impostazione testo")) {
                    editing = if editing == Some(1) { None } else { Some(1) };
                }
                ui.checkbox(&mut t.enable_text, tr("Abilita testo"));
            });
            ui.vertical(|ui| {
                if layer_button(ui, 2, tr("Modifica impostazione bordo")) {
                    editing = if editing == Some(2) { None } else { Some(2) };
                }
                ui.checkbox(&mut t.enable_border, tr("Abilita bordo"));
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.add_space(18.0);
                if ui.add_sized(vec2(100.0, 28.0), egui::Button::new(tr("Chiudi"))).clicked() {
                    close = true;
                }
            });
        });
        if let Some(e) = editing {
            ui.separator();
            let (title, cs) = match e {
                0 => (tr("Impostazione materiale (le celle variano i parametri scelti sopra)"), &mut t.material),
                1 => (tr("Impostazione testo"), &mut t.text),
                _ => (tr("Impostazione bordo"), &mut t.border),
            };
            ui.label(egui::RichText::new(title).strong());
            setting_editor(ui, "mt_edit", cs);
        }
    });
    ctx.data_mut(|d| d.insert_temp(editing_id, editing));
    if save_preset {
        let name = if t.title.trim().is_empty() { trf("Impostazione {}", &[&(app.material_presets.len() + 1)]) } else { t.title.trim().to_string() };
        app.material_presets.retain(|(n, _)| *n != name);
        app.material_presets.push((name.clone(), t.clone()));
        crate::i18n::save_json(PRESETS_FILE, &app.material_presets);
        sel = name;
    }
    if delete_preset {
        let before = app.material_presets.len();
        app.material_presets.retain(|(n, _)| *n != sel);
        if app.material_presets.len() != before {
            crate::i18n::save_json(PRESETS_FILE, &app.material_presets);
            sel.clear();
        }
    }
    app.material_preset_sel = sel;
    if let Some(a) = act {
        let doc = t.build(&app.doc);
        run(app, doc, a, "material-test.gc");
    }
    app.material_test = if close || !open { None } else { Some(t) };
}
