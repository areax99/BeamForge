//! "Impostazioni della periferica": the laser's settings window with the original's four tabs
//! (Dimensioni / Unità, Impostazioni di base, Altre impostazioni, Estensioni; the person's pictures
//! 9-12). It works on a copy: OK applies it, Cancel or ✕ throws it away
//! (MainWindow::on_actionDevice_Settings_triggered 0x100688400: QDialog::exec, applied on accept).
//! RayEtch's own GRBL settings sit in a "Controller GRBL" box on the second tab.

use crate::app::RayEtch;
use crate::i18n::{dv, tr, trf};
use crate::model::{Device, OriginCorner, ScanOffset};
use crate::panels::switch;
use crate::theme;
use eframe::egui::{self, Color32, vec2};

/// Fixed size, the same on every tab (the person's rule).
const SIZE: egui::Vec2 = egui::Vec2::new(900.0, 600.0);
const TABS: [&str; 4] = ["Dimensioni / Unità", "Impostazioni di base", "Altre impostazioni", "Estensioni"];

/// Shown on what needs a feature RayEtch doesn't have yet.
fn later() -> &'static str {
    tr("In arrivo: questa funzione sarà aggiunta in un secondo momento. Per ora l'impostazione viene solo ricordata.")
}

fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(egui::RichText::new(title).strong());
    egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        add(ui);
    });
    ui.add_space(6.0);
}

fn mm(ui: &mut egui::Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, decimals: usize, enabled: bool) {
    ui.add_enabled(enabled, dv(v).speed(0.1).range(range).min_decimals(decimals).max_decimals(decimals).suffix("mm"));
}

fn size_tab(ui: &mut egui::Ui, d: &mut Device) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            group(ui, tr("Area di lavoro"), |ui| {
                egui::Grid::new("dev_area").num_columns(2).show(ui, |ui| {
                    ui.label(tr("Larghezza"));
                    mm(ui, &mut d.width, 10.0..=15000.0, 1, true);
                    ui.end_row();
                    ui.label(tr("Altezza"));
                    mm(ui, &mut d.height, 10.0..=15000.0, 1, true);
                    ui.end_row();
                });
                switch(ui, &mut d.machine_coords, tr("Coordinate della macchina"))
                    .on_hover_text(tr("Acceso: la posizione del laser è mostrata nelle coordinate della macchina (MPos); spento: in quelle di lavoro (WPos)"));
            });
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            group(ui, tr("Offset laser"), |ui| {
                switch(ui, &mut d.pointer_offset_on, tr("Abilita offset puntatore"))
                    .on_hover_text(tr("Distanza del puntatore rosso dal raggio: Inquadra senza laser fa seguire il contorno al puntatore"));
                ui.horizontal(|ui| {
                    let on = d.pointer_offset_on;
                    ui.label("X");
                    mm(ui, &mut d.pointer_offset[0], -1000.0..=1000.0, 2, on);
                    ui.label("Y");
                    mm(ui, &mut d.pointer_offset[1], -1000.0..=1000.0, 2, on);
                });
            });
            group(ui, tr("Unità di controllo"), |ui| {
                let name = |s: bool| if s { "mm/sec" } else { "mm/min" };
                egui::ComboBox::from_id_salt("dev_units").width(260.0).selected_text(name(d.speed_mm_s)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.speed_mm_s, false, "mm/min");
                    ui.selectable_value(&mut d.speed_mm_s, true, "mm/sec");
                });
                switch(ui, &mut d.link_units, tr("Collega le unità di controllo e di progettazione")).on_hover_text(later());
            });
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            group(ui, tr("Origine"), |ui| {
                // the machine's home corner, laid out as the corners of the bed
                egui::Grid::new("dev_origin").num_columns(2).spacing(vec2(6.0, 10.0)).show(ui, |ui| {
                    ui.radio_value(&mut d.origin, OriginCorner::RearLeft, "").on_hover_text(tr("Posteriore sinistra"));
                    ui.radio_value(&mut d.origin, OriginCorner::RearRight, "").on_hover_text(tr("Posteriore destra"));
                    ui.end_row();
                    ui.radio_value(&mut d.origin, OriginCorner::FrontLeft, "").on_hover_text(tr("Anteriore sinistra"));
                    ui.radio_value(&mut d.origin, OriginCorner::FrontRight, "").on_hover_text(tr("Anteriore destra"));
                    ui.end_row();
                });
            });
        });
    });
}

fn basic_tab(ui: &mut egui::Ui, d: &mut Device, edit_checklist: &mut bool, sel_row: &mut Option<usize>) {
    ui.columns(2, |c| {
        let ui = &mut c[0];
        group(ui, tr("Opzioni generali"), |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Larghezza impulso linguetta (mm)")).on_hover_text(later());
                ui.add(dv(&mut d.tab_pulse_width).speed(0.005).range(0.0..=10.0).min_decimals(3).max_decimals(3));
            });
            ui.horizontal(|ui| {
                switch(ui, &mut d.job_checklist_on, tr("Abilita la lista di controllo lavori"))
                    .on_hover_text(tr("Prima di ogni Avvia, chiede di spuntare ogni voce della lista"));
                if ui.button(tr("Modifica")).clicked() {
                    *edit_checklist = true;
                }
            });
            // RayEtch has no rotary yet: shown off and greyed
            let mut no_rotary = false;
            ui.add_enabled_ui(false, |ui| {
                switch(ui, &mut no_rotary, tr("Salva/carica le impostazioni dell'unità rotante con i file di progetto")).on_disabled_hover_text(later());
            });
        });
        group(ui, tr("Regolazione offset di scansione"), |ui| {
            switch(ui, &mut d.scan_offset_on, tr("Abilita regolazione offset di scansione"))
                .on_hover_text(tr("Alla velocità indicata, le righe incise da destra a sinistra vengono spostate di questa distanza, così combaciano con quelle da sinistra a destra"));
            let mut laser2 = false;
            ui.horizontal(|ui| {
                ui.radio_value(&mut laser2, false, tr("Laser 1"));
                ui.add_enabled_ui(d.laser2_on, |ui| {
                    ui.radio_value(&mut laser2, true, tr("Laser 2")).on_disabled_hover_text(later());
                });
            });
            egui::Frame::new().fill(theme::p().field).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_min_height(170.0);
                ui.set_width(ui.available_width());
                egui::Grid::new("scan_rows").num_columns(3).striped(true).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                    ui.label(egui::RichText::new(tr("Velocità (mm/s)")).strong());
                    ui.label(egui::RichText::new(tr("Spostamento linea")).strong());
                    ui.label(egui::RichText::new(tr("Offset iniziale")).strong());
                    ui.end_row();
                    for (i, r) in d.scan_offsets.iter_mut().enumerate() {
                        let picked = *sel_row == Some(i);
                        let resp = ui.add(dv(&mut r.speed).speed(1.0).range(0.0..=5000.0).max_decimals(1));
                        ui.add(dv(&mut r.shift).speed(0.005).range(-10.0..=10.0).min_decimals(3).max_decimals(3).suffix(" mm"));
                        let r3 = ui.add(dv(&mut r.start).speed(0.005).range(-10.0..=10.0).min_decimals(3).max_decimals(3).suffix(" mm"));
                        if resp.has_focus() || r3.has_focus() || resp.clicked() {
                            *sel_row = Some(i);
                        }
                        if picked {
                            ui.label("◀");
                        }
                        ui.end_row();
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button(tr("Aggiungi")).clicked() {
                    let speed = d.scan_offsets.iter().map(|r| r.speed).fold(0.0, f64::max) + 50.0;
                    d.scan_offsets.push(ScanOffset { speed, shift: 0.0, start: 0.0, laser2: false });
                    *sel_row = Some(d.scan_offsets.len() - 1);
                }
                if ui.add_enabled(sel_row.is_some() || !d.scan_offsets.is_empty(), egui::Button::new(tr("Elimina"))).clicked() {
                    let i = sel_row.unwrap_or(d.scan_offsets.len() - 1).min(d.scan_offsets.len() - 1);
                    d.scan_offsets.remove(i);
                    *sel_row = None;
                }
                if ui.button(tr("Importa")).clicked() {
                    if let Some(rows) = import_scan_offsets() {
                        d.scan_offsets = rows;
                    }
                }
                if ui.button(tr("Esporta")).clicked() {
                    export_scan_offsets(&d.scan_offsets);
                }
            });
        });
        let ui = &mut c[1];
        group(ui, tr("Altre opzioni"), |ui| {
            ui.label(egui::RichText::new(tr("Opzioni Laser 1")).strong());
            ui.horizontal(|ui| {
                ui.label(tr("Nome Laser 1"));
                ui.text_edit_singleline(&mut d.laser1_name).on_hover_text(tr("Mostrato nel titolo della finestra di RayEtch"));
            });
            switch(ui, &mut d.laser1_rf, tr("Laser 1 è un tubo RF")).on_hover_text(later());
            switch(ui, &mut d.laser1_fiber, tr("Laser 1 è a fibra")).on_hover_text(later());
            ui.add_space(6.0);
            switch(ui, &mut d.laser2_on, tr("Abilita comandi Laser 2")).on_hover_text(later());
            ui.add_enabled_ui(d.laser2_on, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Nome Laser 2"));
                    ui.text_edit_singleline(&mut d.laser2_name);
                });
                switch(ui, &mut d.one_laser_at_once, tr("Consenti solo un laser alla volta"));
                switch(ui, &mut d.laser2_rf, tr("Laser 2 è un tubo RF"));
                switch(ui, &mut d.laser2_fiber, tr("Laser 2 è a fibra"));
                switch(ui, &mut d.laser2_offset_on, tr("Abilita offset laser 2"));
                ui.horizontal(|ui| {
                    let on = d.laser2_offset_on;
                    ui.label("X:");
                    mm(ui, &mut d.laser2_offset[0], -1000.0..=1000.0, 2, on);
                    ui.label("Y:");
                    mm(ui, &mut d.laser2_offset[1], -1000.0..=1000.0, 2, on);
                });
            });
        });
        // RayEtch's own settings for GRBL controllers
        group(ui, tr("Controller GRBL"), |ui| {
            egui::Grid::new("dev_grbl").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
                ui.label(tr("Nome"));
                ui.text_edit_singleline(&mut d.name);
                ui.end_row();
                ui.label(tr("Valore S massimo ($30)"));
                ui.add(dv(&mut d.s_max).speed(10.0).range(1.0..=100000.0));
                ui.end_row();
                ui.label(tr("Potenza dinamica"));
                switch(ui, &mut d.dynamic_power, tr("M4 (spenta: M3 potenza costante)"));
                ui.end_row();
                ui.label(tr("Aria compressa"));
                switch(ui, &mut d.air_assist_m7, tr("Usa M7 invece di M8"));
                ui.end_row();
                ui.label(tr("Potenza Fuoco (%)"));
                ui.add(dv(&mut d.fire_power).speed(0.1).range(0.0..=100.0));
                ui.end_row();
                ui.label(tr("Inquadra con laser acceso"));
                switch(ui, &mut d.laser_frame, tr("Raggio continuo all'1%"));
                ui.end_row();
                ui.label(tr("Baud"));
                egui::ComboBox::from_id_salt("baud").selected_text(d.baud.to_string()).show_ui(ui, |ui| {
                    for b in [9600u32, 57600, 115200, 230400, 250000] {
                        ui.selectable_value(&mut d.baud, b, b.to_string());
                    }
                });
                ui.end_row();
            });
        });
    });
}

fn sim_tab(ui: &mut egui::Ui, d: &mut Device) {
    group(ui, tr("Impostazioni della simulazione"), |ui| {
        ui.label(egui::RichText::new(tr("Queste impostazioni servono per regolare i tempi di anteprima / simulazione.\nNon influiscono sul controller.")).color(Color32::from_rgb(230, 70, 60)));
        ui.separator();
        let pair = |ui: &mut egui::Ui, label: &str, v: &mut [f64; 2], unit: &str, max: f64| {
            ui.label(label);
            ui.add_sized(vec2(110.0, 20.0), dv(&mut v[0]).speed(5.0).range(1.0..=max).min_decimals(1).max_decimals(1));
            ui.label("Y:");
            ui.add_sized(vec2(110.0, 20.0), dv(&mut v[1]).speed(5.0).range(1.0..=max).min_decimals(1).max_decimals(1));
            ui.label(unit);
            ui.end_row();
        };
        egui::Grid::new("dev_sim").num_columns(5).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            pair(ui, tr("Velocità massima X:"), &mut d.max_speed, "mm/sec", 5000.0);
            pair(ui, tr("Accelerazione taglio X:"), &mut d.cut_accel, "mm/sec^2", 100000.0);
            pair(ui, tr("Accelerazione incisione X:"), &mut d.engrave_accel, "mm/sec^2", 100000.0);
            let one = |ui: &mut egui::Ui, label: &str, v: &mut f64, unit: &str, range: std::ops::RangeInclusive<f64>, dec: usize| {
                ui.label(label);
                ui.add_sized(vec2(110.0, 20.0), dv(v).speed(1.0).range(range).min_decimals(dec).max_decimals(dec));
                ui.label(unit);
                ui.end_row();
            };
            one(ui, tr("Velocità rapida:"), &mut d.rapid_speed, "mm/sec", 1.0..=5000.0, 1);
            one(ui, tr("Precisione angolo:"), &mut d.corner_precision, "mm", 0.0..=10.0, 3);
            one(ui, tr("Velocità d'angolo minima:"), &mut d.min_corner_speed, "mm/sec", 0.0..=1000.0, 2);
            one(ui, tr("Scala velocità taglio:"), &mut d.cut_speed_scale, "%", 1.0..=1000.0, 1);
            one(ui, tr("Scala della velocità globale:"), &mut d.global_speed_scale, "%", 1.0..=1000.0, 1);
        });
    });
}

fn ext_tab(ui: &mut egui::Ui, d: &mut Device) {
    group(ui, "API", |ui| {
        switch(ui, &mut d.api_port_on, tr("Abilita porta di accesso API permanente")).on_hover_text(later());
        ui.horizontal(|ui| {
            ui.add_enabled(d.api_port_on, egui::DragValue::new(&mut d.api_port).range(1024..=65535));
            ui.label(tr("Numero di porta fisso"));
        });
    });
}

/// The scan offset table as CSV: speed (mm/s), line shift (mm), start offset (mm).
fn export_scan_offsets(rows: &[ScanOffset]) {
    let Some(p) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("offset-scansione.csv").save_file() else { return };
    let mut s = String::from("velocita_mm_s;spostamento_mm;offset_iniziale_mm\n");
    for r in rows {
        s.push_str(&format!("{};{};{}\n", r.speed, r.shift, r.start));
    }
    let _ = std::fs::write(p, s);
}

fn import_scan_offsets() -> Option<Vec<ScanOffset>> {
    let p = rfd::FileDialog::new().add_filter("CSV", &["csv", "txt"]).pick_file()?;
    let text = std::fs::read_to_string(p).ok()?;
    let rows: Vec<ScanOffset> = text
        .lines()
        .filter_map(|l| {
            let n: Vec<f64> = l.split([';', ',', '\t']).filter_map(|x| x.trim().replace(',', ".").parse().ok()).collect();
            (n.len() >= 2).then(|| ScanOffset { speed: n[0], shift: n[1], start: n.get(2).copied().unwrap_or(0.0), laser2: false })
        })
        .collect();
    (!rows.is_empty()).then_some(rows)
}

pub fn window(app: &mut RayEtch, ctx: &egui::Context) {
    checklist_window(app, ctx);
    if !app.show_device {
        return;
    }
    let mut open = true;
    let mut d = app.device_draft.take().unwrap_or_else(|| app.doc.device.clone());
    let mut ok = false;
    let mut cancel = false;
    let mut tab = app.device_tab.min(TABS.len() - 1);
    let mut edit_checklist = false;
    let title = trf("Impostazioni della periferica per {}", &[&d.name]);
    let mut sel_row = app.scan_row;
    theme::centered(ctx, "device", egui::Window::new(title).id(egui::Id::new("device_win")).open(&mut open).resizable(false).collapsible(false).fixed_size(SIZE)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (i, t) in TABS.iter().enumerate() {
                if ui.add(egui::Button::new(egui::RichText::new(tr(t)).size(14.0)).selected(tab == i).min_size(vec2(0.0, 30.0))).clicked() {
                    tab = i;
                }
            }
        });
        ui.separator();
        let page_h = SIZE.y - 100.0;
        egui::ScrollArea::vertical().max_height(page_h).min_scrolled_height(page_h).max_width(SIZE.x - 16.0).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(SIZE.x - 28.0);
            match tab {
                0 => size_tab(ui, &mut d),
                1 => basic_tab(ui, &mut d, &mut edit_checklist, &mut sel_row),
                2 => sim_tab(ui, &mut d),
                _ => ext_tab(ui, &mut d),
            }
        });
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("OK").min_size(vec2(90.0, 26.0))).clicked() {
                ok = true;
            }
            if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                cancel = true;
            }
        });
    });
    app.device_tab = tab;
    app.scan_row = sel_row;
    if edit_checklist {
        app.checklist_text = Some(d.job_checklist.join("\n"));
    }
    // the checklist editor writes into the copy
    if let Some(t) = checklist_editor(app, ctx) {
        d.job_checklist = t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    }
    if ok {
        if d.width != app.doc.device.width || d.height != app.doc.device.height {
            app.view.fit_pending = true;
        }
        app.checkpoint_now();
        // keep the saved laser in Periferiche in step with its settings
        if let Some(slot) = app.device_sel.and_then(|i| app.device_list.devices.get_mut(i)) {
            *slot = d.clone();
            app.device_list.save();
        }
        app.doc.device = d;
        app.show_device = false;
    } else if cancel || !open {
        app.show_device = false;
        app.checklist_text = None;
    } else {
        app.device_draft = Some(d);
    }
}

/// "Modifica" of the job checklist: one item per line. Returns the text on OK.
fn checklist_editor(app: &mut RayEtch, ctx: &egui::Context) -> Option<String> {
    let mut text = app.checklist_text.take()?;
    let mut done: Option<bool> = None;
    theme::centered(ctx, "checklist_edit", egui::Window::new(tr("Lista di controllo lavori")).collapsible(false).resizable(false).fixed_size(vec2(440.0, 260.0))).show(ctx, |ui| {
        ui.label(tr("Una voce per riga:"));
        ui.add_sized(vec2(420.0, 180.0), egui::TextEdit::multiline(&mut text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("OK").min_size(vec2(90.0, 26.0))).clicked() {
                done = Some(true);
            }
            if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                done = Some(false);
            }
        });
    });
    match done {
        Some(true) => Some(text),
        Some(false) => None,
        None => {
            app.checklist_text = Some(text);
            None
        }
    }
}

/// Before a start with the checklist on: every item must be ticked, then Avvia starts the job.
fn checklist_window(app: &mut RayEtch, ctx: &egui::Context) {
    let Some(mut ticks) = app.checklist.take() else { return };
    let items = app.doc.device.job_checklist.clone();
    ticks.resize(items.len(), false);
    let mut go = false;
    let mut stop = false;
    theme::centered(ctx, "checklist", egui::Window::new(tr("Prima di avviare")).collapsible(false).resizable(false).fixed_size(vec2(420.0, 60.0 + 30.0 * items.len() as f32))).show(ctx, |ui| {
        for (t, item) in ticks.iter_mut().zip(&items) {
            ui.checkbox(t, item.as_str());
        }
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let all = ticks.iter().all(|t| *t);
            if ui.add_enabled(all, egui::Button::new(tr("Avvia")).min_size(vec2(90.0, 26.0))).on_disabled_hover_text(tr("Spunta ogni voce per avviare")).clicked() {
                go = true;
            }
            if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                stop = true;
            }
        });
    });
    if go {
        app.start_job_now();
    } else if !stop {
        app.checklist = Some(ticks);
    }
}
