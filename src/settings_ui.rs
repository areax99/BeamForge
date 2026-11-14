//! The Impostazioni window, with the original's six tabs (the person's pictures 3-8). It edits a
//! copy of the settings; OK keeps it, Cancel drops it.

use crate::app::BeamForge;
use crate::i18n::{dv, tr};
use crate::panels::switch;
use crate::prefs::{AppTheme, DxfUnit, GridContrast, Prefs};
use crate::theme;
use eframe::egui::{self, vec2};

/// The window's fixed size, so it doesn't change between tabs (the person's rule).
const SIZE: egui::Vec2 = egui::Vec2::new(940.0, 600.0);
const TABS: [&str; 6] = ["Impostazioni editor", "Unità e Griglie", "Schermo", "Importa/Esporta", "Fotocamera", "Estensioni"];
/// Shown on the options that need a whole feature BeamForge doesn't have yet (pages, rotary,
/// inches, its own Load/Save windows, SHX fonts, DXF export, printing, camera, API): the person
/// chose to add those later.
fn not_yet() -> &'static str {
    tr("In arrivo: questa funzione sarà aggiunta in un secondo momento. Per ora l'impostazione viene solo ricordata.")
}

/// A switch; `active` false adds the "not used yet" note.
fn opt(ui: &mut egui::Ui, on: &mut bool, label: &str, active: bool) {
    let r = switch(ui, on, label);
    if !active {
        r.on_hover_text(not_yet());
    }
}

/// A titled box, like the original's group boxes.
fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(egui::RichText::new(title).strong());
    egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
    ui.add_space(6.0);
}

/// A row: label on the left, a number on the right.
fn number_row(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>, decimals: usize, active: bool) {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        if !active {
            l.on_hover_text(not_yet());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_sized(vec2(140.0, 20.0), dv(value).speed(0.1).range(range).min_decimals(decimals).max_decimals(decimals));
        });
    });
}

fn level_name(v: u8, names: &[&'static str]) -> &'static str {
    tr(names[(v as usize).min(names.len() - 1)])
}

fn editor_tab(ui: &mut egui::Ui, p: &mut Prefs, updates: &mut bool) {
    group(ui, tr("Impostazioni editor"), |ui| {
        ui.columns(2, |c| {
            let ui = &mut c[0];
            ui.horizontal(|ui| {
                ui.label(tr("Intervallo di salvataggio automatico (minuti):"));
                ui.add(egui::DragValue::new(&mut p.autosave_minutes).range(0..=120))
                    .on_hover_text(tr("Ogni tanti minuti, se ci sono modifiche, BeamForge ne salva una copia di sicurezza (0 = mai)"));
            });
            opt(ui, &mut p.beginner_mode, tr("Modalità principiante (interfaccia più semplice)"), true);
            opt(ui, &mut p.ignore_out_of_bounds, tr("Ignora le forme fuori limite se possibile"), true);
            opt(ui, &mut p.invert_wheel_zoom, tr("Inverti il senso dello zoom della rotellina del mouse"), true);
            opt(ui, &mut p.save_cut_selected_in_project, tr("Salva Taglia selezionata / Origine selezione nel progetto"), true);
            opt(ui, &mut p.default_cut_selected, tr("Stato predefinito \"Taglia selezionato\""), true);
            opt(ui, &mut p.default_use_selection_origin, tr("Stato predefinito \"Usa origine selezione\""), true);
            opt(ui, &mut p.save_job_origin_in_project, tr("Salva origine lavoro nel progetto"), true);
            opt(ui, &mut p.save_move_rotate_scale_switches, tr("Salva lo stato degli interruttori Sposta / Ruota / Scala"), true);
            opt(ui, &mut p.load_default_layer_settings, tr("Carica le impostazioni di livello predefinite su nuovo o riavvia"), true);
            opt(ui, &mut p.select_after_drawing, tr("Passa alla modalità Seleziona dopo aver disegnato una forma"), true);
            opt(ui, &mut p.show_page_tabs, tr("Mostra le linguette di selezione delle pagine"), false);
            // BeamForge sends nothing: shown off and greyed
            ui.add_enabled_ui(false, |ui| {
                switch(ui, &mut p.send_usage_data, tr("Invia dati di utilizzo anonimi"))
                    .on_disabled_hover_text(tr("BeamForge non invia dati a nessuno"));
                switch(ui, &mut p.ask_crash_reports, tr("Chiedi di segnalare i crash"))
                    .on_disabled_hover_text(tr("BeamForge non invia dati a nessuno"));
            });
            let ui = &mut c[1];
            opt(ui, updates, tr("Controlla automaticamente gli aggiornamenti"), true);
            opt(ui, &mut p.use_system_clipboard, tr("Usa appunti di sistema"), true);
            ui.add_space(20.0);
            opt(ui, &mut p.error_sound_on_send_fail, tr("Riproduci il suono dell'errore se l'invio del file fallisce"), true);
            opt(ui, &mut p.ignore_start_when_monitor_off, tr("Ignora il pulsante \"Avvio\" se il monitor è spento / inattivo"), true);
            opt(ui, &mut p.start_button_no_focus, tr("Non lasciare che il pulsante \"Start\" mantenga il focus sulla tastiera"), true);
            opt(ui, &mut p.save_job_log, tr("Salva registro lavori"), true);
            ui.add_space(8.0);
            ui.radio_value(&mut p.internal_file_dialogs, true, tr("Utilizza le finestre di dialogo interne Carica / Salva")).on_hover_text(not_yet());
            ui.add_space(8.0);
            ui.radio_value(&mut p.internal_file_dialogs, false, tr("Ignora le finestre di dialogo Carica / Salva del sistema")).on_hover_text(not_yet());
        });
    });
    group(ui, tr("Impostazioni testo"), |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Carattere predefinito"));
            let shown = if p.default_font.is_empty() { crate::text::fonts()[0].name.clone() } else { p.default_font.clone() };
            egui::ComboBox::from_id_salt("pref_font").width(220.0).selected_text(shown).show_ui(ui, |ui| {
                for f in crate::text::fonts() {
                    ui.selectable_value(&mut p.default_font, f.name.clone(), &f.name);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_sized(vec2(140.0, 20.0), dv(&mut p.default_text_height).speed(0.5).range(0.5..=500.0).min_decimals(2).max_decimals(2));
                ui.label(tr("Altezza predefinita"));
            });
        });
        ui.horizontal(|ui| {
            ui.add_enabled(false, egui::Button::new(tr("Imposta percorso Font SHX"))).on_disabled_hover_text(not_yet());
            ui.label(tr("Nessun percorso font SHX configurato"));
        });
        ui.horizontal(|ui| {
            opt(ui, &mut p.right_to_left_text, tr("Abilita il supporto del testo da destra a sinistra"), true);
            ui.add_space(30.0);
            opt(ui, &mut p.double_click_edits_text, tr("Fare doppio clic sulla selezione per visualizzare la finestra di dialogo Modifica testo"), true);
        });
    });
}

fn units_tab(ui: &mut egui::Ui, p: &mut Prefs) {
    ui.columns(2, |c| {
        let ui = &mut c[0];
        ui.add_space(26.0);
        group(ui, tr("Unità di progettazione"), |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut p.inches, false, tr("Millimetri"));
                ui.radio_value(&mut p.inches, true, tr("Pollici")).on_hover_text(not_yet());
            });
        });
        ui.add_space(20.0);
        group(ui, tr("Aggancio"), |ui| {
            ui.horizontal(|ui| {
                opt(ui, &mut p.snap_to_objects, tr("Aggancia agli oggetti"), true);
                ui.add_space(40.0);
                opt(ui, &mut p.snap_to_grid, tr("Aggancia alla griglia"), true);
            });
        });
        let ui = &mut c[1];
        group(ui, tr("Griglie"), |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Contrasto griglia"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let name = |g: GridContrast| match g {
                        GridContrast::Low => tr("Contrasto basso"),
                        GridContrast::Medium => tr("Contrasto medio"),
                        GridContrast::High => tr("Contrasto alto"),
                    };
                    egui::ComboBox::from_id_salt("pref_grid").width(140.0).selected_text(name(p.grid_contrast)).show_ui(ui, |ui| {
                        for g in [GridContrast::Low, GridContrast::Medium, GridContrast::High] {
                            ui.selectable_value(&mut p.grid_contrast, g, name(g));
                        }
                    });
                });
            });
            number_row(ui, tr("Visualizza spaziatura griglia"), &mut p.grid_spacing, 0.1..=1000.0, 2, true);
            number_row(ui, tr("Distanza griglia magnetica"), &mut p.snap_grid_distance, 0.01..=100.0, 2, true);
            number_row(ui, tr("Tolleranza selezione clic (pixel)"), &mut p.click_tolerance_px, 0.5..=50.0, 1, true);
            number_row(ui, tr("Distanza snap ad oggetto (Pixel)"), &mut p.snap_object_distance_px, 0.5..=100.0, 1, true);
        });
    });
    group(ui, tr("Incrementi spostamento forma"), |ui| {
        let cmd = if cfg!(target_os = "macos") { "⌘ + " } else { "Ctrl + " };
        ui.horizontal(|ui| {
            for (label, v) in [(format!("{cmd}{}", tr("Freccia:")), &mut p.nudge_fine), (tr("Freccia:").to_string(), &mut p.nudge), (format!("⇧ + {}", tr("Freccia:")), &mut p.nudge_coarse)] {
                ui.label(label);
                ui.add_sized(vec2(150.0, 20.0), dv(v).speed(0.1).range(0.01..=1000.0).min_decimals(2).max_decimals(2));
                ui.add_space(30.0);
            }
        });
    });
}

fn screen_tab(ui: &mut egui::Ui, p: &mut Prefs, lang: &mut Option<Option<crate::i18n::Lang>>) {
    group(ui, tr("Schermo / Grafica"), |ui| {
        ui.columns(2, |c| {
            let ui = &mut c[0];
            opt(ui, &mut p.antialiasing, tr("Abilita antialiasing (più lento, ma migliore)"), true);
            opt(ui, &mut p.filled_rendering, tr("Rendering riempito (più lento)"), true);
            opt(ui, &mut p.reduce_motion, tr("Riduci il movimento (più lento)"), true);
            opt(ui, &mut p.dark_background, tr("Usa sfondo scuro"), true);
            opt(ui, &mut p.palette_labels, tr("Mostra etichette pulsanti tavolozza"), true);
            opt(ui, &mut p.screen_size_checks, tr("Esegui controlli delle dimensioni dello schermo all'avvio"), true);
            let ui = &mut c[1];
            opt(ui, &mut p.fullscreen_line_cursor, tr("Mostra cursore linea a schermo intero"), true);
            opt(ui, &mut p.show_rotary_toggle, tr("Mostra abilitazione unità rotante nella finestra principale"), false);
            opt(ui, &mut p.show_cylinder_toggle, tr("Mostra abilitazione correzione cilindro sulla finestra principale"), false);
            opt(ui, &mut p.show_center_cross, tr("Mostra la croce al centro dell'area di lavoro"), true);
            opt(ui, &mut p.high_dpi_workspace, tr("Disegno a DPI elevato dell'area di lavoro (più lento e dettaglio più preciso)"), true);
        });
        let sizes = ["Normale", "Grande", "Più grande", "Molto grande", "Enorme"];
        let icon_sizes = ["Piccola", "Normale", "Grande", "Più grande", "Enorme"];
        let quality = ["Bassa", "Media", "Normale", "Alta", "Massima"];
        let fonts = ["Default", "Grande", "Più grande", "Molto grande", "Enorme"];
        // two sliders a row, each with its level after it, as on the original
        ui.spacing_mut().slider_width = 110.0;
        egui::Grid::new("pref_sliders").num_columns(6).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            let slider = |ui: &mut egui::Ui, label: &str, v: &mut u8, names: &[&'static str]| {
                ui.label(label);
                ui.add(egui::Slider::new(v, 0..=4).show_value(false));
                ui.label(level_name(*v, names));
            };
            slider(ui, tr("Cursori/Nodi"), &mut p.handle_size, &sizes);
            slider(ui, tr("Qualità della curva"), &mut p.curve_quality, &quality);
            ui.end_row();
            slider(ui, tr("Dimensione icone nella barra degli strumenti"), &mut p.toolbar_icon_size, &icon_sizes);
            slider(ui, tr("Dimensione del font"), &mut p.font_size, &fonts);
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.label(tr("Tema dell'applicazione"));
            let name = |t: AppTheme| match t {
                AppTheme::System => tr("Tema di sistema"),
                AppTheme::Light => tr("Chiaro"),
                AppTheme::Dark => tr("Scuro"),
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("pref_theme").width(400.0).selected_text(name(p.theme)).show_ui(ui, |ui| {
                    for t in [AppTheme::System, AppTheme::Light, AppTheme::Dark] {
                        if ui.selectable_value(&mut p.theme, t, name(t)).changed() {
                            p.dark_background = t == AppTheme::Dark;
                        }
                    }
                });
            });
        });
        // BeamForge's own: the interface language
        ui.horizontal(|ui| {
            ui.label(tr("Lingua"));
            let current = crate::i18n::load_choice();
            let shown = match current {
                None => tr("Automatica (lingua di sistema)").to_string(),
                Some(l) => l.native_name().to_string(),
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("language").width(400.0).selected_text(shown).show_ui(ui, |ui| {
                    if ui.selectable_label(current.is_none(), tr("Automatica (lingua di sistema)")).clicked() {
                        *lang = Some(None);
                    }
                    for l in crate::i18n::ALL {
                        if ui.selectable_label(current == Some(l), l.native_name()).clicked() {
                            *lang = Some(Some(l));
                        }
                    }
                });
            });
        });
    });
}

fn import_tab(ui: &mut egui::Ui, p: &mut Prefs) {
    group(ui, tr("Impostazioni di importazione generali"), |ui| {
        ui.columns(2, |c| {
            opt(&mut c[0], &mut p.group_imported, tr("Raggruppa le forme importate"), true);
            opt(&mut c[0], &mut p.import_hidden_ai_layers, tr("Importa livelli nascosti da file IA"), true);
            opt(&mut c[1], &mut p.select_imported, tr("Seleziona le forme importate"), true);
            opt(&mut c[1], &mut p.center_imported, tr("Centra le forme importate nella finestra Modifica"), true);
            opt(&mut c[1], &mut p.import_on_tool_layers, tr("Consenti l'importazione sui livelli degli strumenti"), true);
        });
    });
    ui.columns(3, |c| {
        group(&mut c[0], tr("Impostazioni di importazione DXF"), |ui| {
            opt(ui, &mut p.dxf_auto_units, tr("Rileva automaticamente le unità, se possibile"), true);
            egui::Grid::new("pref_dxf_units").num_columns(2).show(ui, |ui| {
                let units = [
                    (DxfUnit::Millimeters, "Millimetri", DxfUnit::Microns, "Micron"),
                    (DxfUnit::Centimeters, "Centimetri", DxfUnit::Inches, "Pollici"),
                    (DxfUnit::Meters, "Metri", DxfUnit::Feet, "Piedi"),
                ];
                for (a, al, b, bl) in units {
                    ui.radio_value(&mut p.dxf_unit, a, tr(al));
                    ui.radio_value(&mut p.dxf_unit, b, tr(bl));
                    ui.end_row();
                }
                ui.radio_value(&mut p.dxf_unit, DxfUnit::Kilometers, tr("Chilometri"));
                ui.end_row();
            });
            opt(ui, &mut p.dxf_ignore_text, tr("Ignora oggetti di testo"), true);
            ui.horizontal(|ui| {
                ui.label(tr("Chiudi automaticamente tolleranza (mm)"));
                ui.add(dv(&mut p.dxf_close_tolerance).speed(0.001).range(0.0..=10.0).min_decimals(3).max_decimals(3));
            });
        });
        group(&mut c[1], tr("Impostazioni di esportazione DXF"), |ui| {
            ui.radio_value(&mut p.dxf_export_arcs, true, tr("Esporta archi")).on_hover_text(not_yet());
            ui.radio_value(&mut p.dxf_export_arcs, false, tr("Converti archi in linee")).on_hover_text(not_yet());
        });
        group(&mut c[2], tr("Impostazioni di importazione SVG"), |ui| {
            ui.radio_value(&mut p.svg_dpi, 96, tr("96 DPI - InkScape (predefinito)"));
            ui.radio_value(&mut p.svg_dpi, 72, tr("72 DPI - Illustrator"));
            opt(ui, &mut p.svg_auto_dpi, tr("Rilevazione automatica, se possibile"), true);
        });
        group(&mut c[2], tr("Precisione importazione da formato HPGL/PLT"), |ui| {
            ui.radio_value(&mut p.hpgl_units, 1000, "1\"/1000");
            ui.radio_value(&mut p.hpgl_units, 1016, "1\"/1016");
        });
    });
    ui.columns(2, |c| {
        group(&mut c[0], tr("Impostazioni di output"), |ui| {
            number_row(ui, tr("Tolleranza curva (mm)"), &mut p.curve_tolerance, 0.001..=5.0, 3, true);
        });
        group(&mut c[1], tr("Impostazioni di stampa"), |ui| {
            number_row(ui, tr("Larghezza linea (mm)"), &mut p.print_line_width, 0.01..=10.0, 3, false);
        });
    });
}

fn camera_tab(ui: &mut egui::Ui, p: &mut Prefs) {
    ui.label(egui::RichText::new(tr("BeamForge non gestisce ancora le fotocamere: queste impostazioni vengono solo ricordate.")).color(theme::p().text_dim));
    ui.add_space(6.0);
    ui.columns(2, |c| {
        group(&mut c[0], tr("Sistema di acquisizione della fotocamera"), |ui| {
            ui.radio_value(&mut p.camera_recommended, false, tr("Fallback"));
            ui.radio_value(&mut p.camera_recommended, true, tr("Consigliato"));
        });
        group(&mut c[1], tr("Sovrapposizione telecamera"), |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Dimensione massima sovrapposizione:"));
                ui.add(egui::DragValue::new(&mut p.overlay_max_mb).range(10..=4000).suffix(" MB"));
            });
        });
    });
    group(ui, tr("Anteprima impostazioni"), |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Anteprima qualità:"));
            ui.add(egui::Slider::new(&mut p.camera_preview_quality, 0..=4).show_value(false));
            ui.label(if p.camera_preview_quality == 0 { tr("Veloce") } else { tr("Migliore") });
        });
        ui.horizontal(|ui| {
            ui.label(tr("Limita la frequenza di fotogrammi di anteprima:"));
            ui.add(dv(&mut p.camera_preview_fps).speed(0.1).range(0.5..=30.0).min_decimals(1).max_decimals(1).suffix("fps"));
        });
        ui.label(egui::RichText::new(tr("Imposta la frequenza di fotogrammi della finestra di anteprima per limitare la CPU")).color(theme::p().text_dim));
    });
    group(ui, tr("Impostazioni predefinite della telecamera"), |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("Modalità fotocamera predefinita:"));
            ui.add(dv(&mut p.camera_default_fps).speed(0.1).range(0.5..=30.0).min_decimals(1).max_decimals(1).suffix("fps"));
        });
        ui.label(egui::RichText::new(tr("Scegli una frequenza di fotogrammi preferita della telecamera per controllare l'esposizione")).color(theme::p().text_dim));
    });
}

fn extensions_tab(ui: &mut egui::Ui, p: &mut Prefs) {
    ui.horizontal(|ui| {
        ui.add_enabled(false, egui::Button::new(tr("Gestisci autorizzazioni API"))).on_disabled_hover_text(not_yet());
        opt(ui, &mut p.api_network_access, tr("Consenti l'accesso API dalla rete"), false);
    });
    ui.add_enabled(false, egui::Button::new(tr("Visualizza documentazione API"))).on_disabled_hover_text(not_yet());
    ui.add_space(200.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(tr("BeamForge non ha ancora un'interfaccia API: questa impostazione viene solo ricordata.")).strong());
    });
}

/// "Modifica testo": opened by a double click on a text when the setting asks for it.
pub fn text_edit_window(app: &mut BeamForge, ctx: &egui::Context) {
    let Some((id, mut text)) = app.text_edit.take() else { return };
    let mut open = true;
    let mut done: Option<bool> = None;
    theme::centered(ctx, "text_edit", egui::Window::new(tr("Modifica testo")).open(&mut open).resizable(false).collapsible(false).fixed_size(vec2(420.0, 200.0))).show(ctx, |ui| {
        let r = ui.add_sized(vec2(400.0, 130.0), egui::TextEdit::multiline(&mut text));
        if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("OK").min_size(vec2(90.0, 26.0))).clicked() {
                    done = Some(true);
                }
                if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                    done = Some(false);
                }
            });
        });
    });
    match done {
        Some(true) => {
            app.checkpoint_now();
            if let Some(s) = app.doc.shapes.iter_mut().find(|s| s.id == id) {
                if let crate::model::Geometry::Text { text: t, size, font, outline } = &mut s.geom {
                    *t = text;
                    *outline = crate::text::text_outline(t, *size, font);
                }
            }
        }
        Some(false) => {}
        None if open => app.text_edit = Some((id, text)),
        None => {}
    }
}

pub fn window(app: &mut BeamForge, ctx: &egui::Context) {
    if !app.show_settings {
        app.prefs_draft = None;
        return;
    }
    let mut draft = app.prefs_draft.take().unwrap_or_else(|| app.prefs.clone());
    let mut updates = app.prefs_updates.unwrap_or_else(Prefs::check_updates);
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    let mut lang: Option<Option<crate::i18n::Lang>> = None;
    let mut tab = app.settings_tab.min(TABS.len() - 1);
    theme::centered(ctx, "settings", egui::Window::new(tr("Impostazioni")).open(&mut open).resizable(false).collapsible(false).fixed_size(SIZE)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (i, t) in TABS.iter().enumerate() {
                let sel = tab == i;
                let b = egui::Button::new(egui::RichText::new(tr(t)).size(14.0)).selected(sel).min_size(vec2(0.0, 30.0));
                if ui.add(b).clicked() {
                    tab = i;
                }
            }
        });
        ui.separator();
        // the page fills a fixed area, so the buttons below never move between tabs
        let page_h = SIZE.y - 110.0;
        let page_w = SIZE.x - 16.0;
        egui::ScrollArea::vertical().max_height(page_h).min_scrolled_height(page_h).max_width(page_w).auto_shrink([false, false]).show(ui, |ui| {
            // every tab gets the same width: a wide row wraps or is cut, the window doesn't grow
            ui.set_max_width(page_w - 12.0);
            match tab {
                0 => editor_tab(ui, &mut draft, &mut updates),
                1 => units_tab(ui, &mut draft),
                2 => screen_tab(ui, &mut draft, &mut lang),
                3 => import_tab(ui, &mut draft),
                4 => camera_tab(ui, &mut draft),
                _ => extensions_tab(ui, &mut draft),
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tr("Ripristina alle impostazioni predefinite...")).clicked() {
                draft = Prefs::default();
                updates = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("OK").min_size(vec2(90.0, 26.0))).clicked() {
                    ok = true;
                }
                if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                    cancel = true;
                }
            });
        });
    });
    app.settings_tab = tab;
    if let Some(choice) = lang {
        app.set_language(choice, ctx);
    }
    if ok {
        Prefs::set_check_updates(updates);
        app.apply_prefs(draft, ctx);
        app.show_settings = false;
        app.prefs_updates = None;
    } else if cancel || !open {
        app.show_settings = false;
        app.prefs_updates = None;
    } else {
        app.prefs_draft = Some(draft);
        app.prefs_updates = Some(updates);
    }
}
