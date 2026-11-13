//! macOS system menu bar (the original uses the system menu, not one inside the window).

use crate::i18n::tr;
use crate::app::{BeamForge, RightTab};
use eframe::egui;
use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, accelerator::{Accelerator, Code, Modifiers}};
use std::sync::{Mutex, Once, OnceLock};

/// Menu clicks waiting for the next frame. muda accepts its event handler only once per run
/// (a OnceLock), so one permanent handler feeds this queue for every menu ever installed.
static QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());
static REPAINT: OnceLock<egui::Context> = OnceLock::new();
static HANDLER: Once = Once::new();

pub struct SystemMenu {
    _menu: Menu,
}

thread_local! {
    /// Every menu-bar item with its last enabled state, so it can be greyed out when it can't run.
    static ITEMS: std::cell::RefCell<Vec<(MenuItem, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn item(id: &str, text: &str, acc: Option<Accelerator>) -> MenuItem {
    let m = MenuItem::with_id(id, text, true, acc);
    ITEMS.with(|v| v.borrow_mut().push((m.clone(), true)));
    m
}

impl BeamForge {
    /// Whether an action can run now; when it can't, its menu item and button are greyed out.
    /// Like the original (MainWindow::UpdateActionsStatus 0x100c39980 enables each action from
    /// Core_Checks::UpdateSelectionStatus): selection actions need a selection, arranging needs
    /// 2 objects (align, group, same size, move together, combine: exactly 2), distribute 3
    /// (Core_Arrange rules), paste a copied shape, laser commands a connected laser.
    pub fn can(&self, id: &str) -> bool {
        let n = self.selection.len();
        let items = || self.selection_items().len();
        let has_shapes = !self.doc.shapes.is_empty();
        // UpdateActionsStatus 0x100c39980: every selection/clipboard action is also off while the current
        // tool is busy (virtual call +0x90 on the active tool): a drag, a shape being drawn, an open polyline.
        let busy = !matches!(self.drag, crate::app::Drag::None) || !self.line_pts.is_empty();
        let edit = matches!(
            id,
            "cut" | "copy" | "paste" | "delete" | "duplicate" | "mirror_h" | "mirror_v" | "rot_cw" | "rot_ccw" | "center_page" | "offset"
                | "group" | "ungroup" | "same_w" | "same_h" | "together_h" | "together_v" | "combine" | "combine_assist"
                | "dist_h_spaced" | "dist_v_spaced" | "dist_h_centered" | "dist_v_centered" | "undo" | "redo"
        ) || id.starts_with("align_");
        if busy && edit {
            return false;
        }
        match id {
            "cut" | "copy" | "delete" | "duplicate" | "mirror_h" | "mirror_v" | "rot_cw" | "rot_ccw" | "center_page" | "offset" => n > 0,
            "paste" => !self.clipboard.is_empty(),
            "undo" => !self.undo.is_empty(),
            "redo" => !self.redo.is_empty(),
            "select_all" | "preview" | "export_gcode" | "export_svg" => has_shapes,
            "frame" | "frame_outline" | "start" => has_shapes && self.connected.is_some(),
            "pause" | "stop" => self.job_progress.is_some() || self.paused,
            "group" | "same_w" | "same_h" | "together_h" | "together_v" => items() >= 2,
            "combine_assist" | "combine" => items() == 2,
            "dist_h_spaced" | "dist_v_spaced" | "dist_h_centered" | "dist_v_centered" => items() >= 3,
            "ungroup" => self.doc.shapes.iter().any(|s| self.selection.contains(&s.id) && !s.groups.is_empty()),
            "home" | "unlock" | "fire_on" | "fire_off" | "run_gcode" | "goto_origin" => self.connected.is_some(),
            other if other.starts_with("align_") => items() >= 2,
            _ => true,
        }
    }
}

/// Grey out the menu-bar items that can't run now (only the ones whose state changed).
fn refresh_enabled(app: &BeamForge) {
    ITEMS.with(|v| {
        for (m, last) in v.borrow_mut().iter_mut() {
            let on = app.can(&m.id().0);
            if on != *last {
                m.set_enabled(on);
                *last = on;
            }
        }
    });
}

fn cmd(code: Code) -> Option<Accelerator> {
    Some(Accelerator::new(Modifiers::META, code))
}

/// A submenu from shared menu rows (the same rows the window's own menu draws).
fn rows_submenu(title: &str, rows: Vec<crate::panels::MenuRow>) -> Option<Submenu> {
    let sub = Submenu::new(title, true);
    for row in rows {
        match row {
            None => sub.append(&PredefinedMenuItem::separator()).ok()?,
            Some((id, label)) => {
                let acc = (id == "preview").then(|| Accelerator::new(Modifiers::ALT, Code::KeyP));
                sub.append(&item(&id, label, acc)).ok()?
            }
        }
    }
    Some(sub)
}

/// Builds the menu and installs it in the macOS menu bar. Returns None on other systems.
pub fn install(ctx: &egui::Context) -> Option<SystemMenu> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    ITEMS.with(|v| v.borrow_mut().clear());
    let menu = Menu::new();
    let sep = PredefinedMenuItem::separator;

    let app_menu = Submenu::with_items(
        "BeamForge",
        true,
        &[
            &PredefinedMenuItem::about(
                Some(tr("Informazioni su BeamForge")),
                Some(AboutMetadata {
                    name: Some("BeamForge".into()),
                    version: Some("1.0".into()),
                    comments: Some(tr("Software libero per laser, licenza Apache 2.0. Nessuna attivazione richiesta.").into()),
                    ..Default::default()
                }),
            ),
            &sep(),
            &PredefinedMenuItem::services(Some(tr("Servizi"))),
            &sep(),
            &PredefinedMenuItem::hide(Some(tr("Nascondi BeamForge"))),
            &PredefinedMenuItem::hide_others(Some(tr("Nascondi altre"))),
            &PredefinedMenuItem::show_all(Some(tr("Mostra tutte"))),
            &sep(),
            &item("quit", tr("Esci da BeamForge"), cmd(Code::KeyQ)),
        ],
    )
    .ok()?;

    let file = Submenu::with_items(
        tr("File"),
        true,
        &[
            &item("new", tr("Nuovo"), cmd(Code::KeyN)),
            &item("open", tr("Apri…"), cmd(Code::KeyO)),
            &sep(),
            &item("save", tr("Salva"), cmd(Code::KeyS)),
            &item("save_as", tr("Salva con nome…"), Some(Accelerator::new(Modifiers::META | Modifiers::SHIFT, Code::KeyS))),
            &sep(),
            &item("import", tr("Importa…"), cmd(Code::KeyI)),
            &item("export_gcode", tr("Esporta G-code…"), None),
            &item("export_svg", tr("Esporta SVG…"), None),
        ],
    )
    .ok()?;

    // Editing keys (⌘Z, ⌘C, ⌘V…) stay with the window, so they keep working inside text fields.
    let edit = Submenu::with_items(
        tr("Modifica"),
        true,
        &[
            &item("undo", tr("Annulla (⌘Z)"), None),
            &item("redo", tr("Ripeti (⇧⌘Z)"), None),
            &sep(),
            &item("cut", tr("Taglia (⌘X)"), None),
            &item("copy", tr("Copia (⌘C)"), None),
            &item("paste", tr("Incolla (⌘V)"), None),
            &item("duplicate", tr("Duplica (⌘D)"), None),
            &item("delete", tr("Elimina (⌫)"), None),
            &sep(),
            &item("select_all", tr("Seleziona tutto (⌘A)"), None),
            &sep(),
            // NoRole in the original, so it stays here and not in the app menu (MECHANICS.md, "Menu BeamForge").
            &item("device", tr("Impostazioni dispositivo…"), None),
            // the running original has "Impostazioni" at the end of its Edit menu and none in the
            // app menu (measured, MECHANICS.md round 65)
            &item("settings", tr("Impostazioni…"), Some(Accelerator::new(Modifiers::META, Code::Comma))),
        ],
    )
    .ok()?;

    let tools = rows_submenu(tr("Strumenti"), crate::panels::tools_menu_rows())?;

    let arrange = Submenu::with_items(
        tr("Organizza"),
        true,
        &[
            &item("group", tr("Raggruppa"), cmd(Code::KeyG)),
            &item("ungroup", tr("Separa"), Some(Accelerator::new(Modifiers::META | Modifiers::SHIFT, Code::KeyG))),
            &sep(),
            &Submenu::with_items(
                tr("Allinea (all'ultimo selezionato)"),
                true,
                &[
                    &item("align_0", tr("Allinea a sinistra"), None),
                    &item("align_4", tr("Allinea centri orizzontali"), None),
                    &item("align_1", tr("Allinea a destra"), None),
                    &sep(),
                    &item("align_2", tr("Allinea in alto"), None),
                    &item("align_5", tr("Allinea centri verticali"), None),
                    &item("align_3", tr("Allinea in basso"), None),
                    &sep(),
                    &item("align_6", tr("Allinea centri"), None),
                ],
            )
            .ok()?,
            &Submenu::with_items(
                tr("Distribuisci"),
                true,
                &[
                    &item("dist_h_spaced", tr("Distribuisci spaziatura orizzontale"), None),
                    &item("dist_v_spaced", tr("Distribuisci spaziatura verticale"), None),
                    &item("dist_h_centered", tr("Distribuisci centri orizzontali"), None),
                    &item("dist_v_centered", tr("Distribuisci centri verticali"), None),
                ],
            )
            .ok()?,
            &Submenu::with_items(
                tr("Avvicina"),
                true,
                &[
                    &item("together_h", tr("Avvicina orizzontalmente"), None),
                    &item("together_v", tr("Avvicina verticalmente"), None),
                ],
            )
            .ok()?,
            &item("same_w", tr("Stessa larghezza"), None),
            &item("same_h", tr("Stessa altezza"), None),
            &sep(),
            &item("offset", tr("Offset…"), None),
            &item("mirror_h", tr("Specchia orizzontalmente"), None),
            &item("mirror_v", tr("Specchia verticalmente"), None),
            &sep(),
            &item("rot_cw", tr("Ruota 90° in senso orario"), None),
            &item("rot_ccw", tr("Ruota 90° in senso antiorario"), None),
            &sep(),
            &item("center_page", tr("Sposta al centro della pagina"), None),
        ],
    )
    .ok()?;

    let laser = rows_submenu(tr("Strumenti laser"), crate::panels::laser_menu_rows())?;

    let window = Submenu::with_items(
        tr("Finestra"),
        true,
        &[
            &item("fit", tr("Adatta alla pagina"), None),
            &item("zoom_in", tr("Ingrandisci"), None),
            &item("zoom_out", tr("Riduci"), None),
            &sep(),
            &item("tab_laser", tr("Laser"), None),
            &item("tab_move", tr("Sposta"), None),
            &item("tab_console", tr("Console"), None),
            &item("tab_shape", tr("Proprietà forma"), None),
            &sep(),
            &item("reset_layout", tr("Reimposta disposizione predefinita"), None),
            &sep(),
            &PredefinedMenuItem::minimize(Some(tr("Riduci a icona"))),
            &PredefinedMenuItem::fullscreen(Some(tr("Schermo intero"))),
        ],
    )
    .ok()?;

    let help = Submenu::with_items(tr("Guida"), true, &[&item("updates", tr("Aggiornamenti…"), None), &item("about", tr("Informazioni su BeamForge"), None)]).ok()?;

    menu.append_items(&[&app_menu, &file, &edit, &tools, &arrange, &laser, &window, &help]).ok()?;
    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
        window.set_as_windows_menu_for_nsapp();
    }

    let _ = REPAINT.set(ctx.clone());
    HANDLER.call_once(|| {
        MenuEvent::set_event_handler(Some(|e: MenuEvent| {
            if let Ok(mut q) = QUEUE.lock() {
                q.push(e.id.0.clone());
            }
            if let Some(c) = REPAINT.get() {
                c.request_repaint();
            }
        }));
    });
    Some(SystemMenu { _menu: menu })
}

/// Run the actions chosen in the system menu since the last frame.
pub fn handle(app: &mut BeamForge, ctx: &egui::Context) {
    if app.system_menu.is_none() {
        return;
    }
    let ids: Vec<String> = QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    for id in ids {
        if app.can(&id) {
            run(app, ctx, &id);
        }
    }
    refresh_enabled(app);
}

/// Run one menu action by its id (from the menu bar or the window's menu).
pub fn run(app: &mut BeamForge, ctx: &egui::Context, id: &str) {
    {
        if let Some(t) = id.strip_prefix("tool_").and_then(|n| n.parse::<usize>().ok()).and_then(|n| crate::panels::TOOLBAR_TOOLS.get(n)) {
            app.tool = *t;
            app.line_pts.clear();
            if crate::app::SHAPE_TOOLS.contains(t) {
                app.shape_tool = *t;
            }
            ctx.request_repaint();
            return;
        }
        match id {
            "new" => app.new_doc(ctx),
            "quit" => app.request_close(crate::app::PendingClose::Quit, ctx),
            "open" => app.open_dialog(ctx),
            "save" => app.save(false),
            "save_as" => app.save(true),
            "import" => app.import_dialog(),
            "export_gcode" => app.save_gcode(),
            "export_svg" => app.export_svg(),
            "device" => app.show_device = true,
            "undo" => app.do_undo(),
            "redo" => app.do_redo(),
            "cut" => app.cut(),
            "copy" => app.copy(),
            "paste" => app.paste(5.0),
            "duplicate" => app.duplicate(),
            "delete" => app.delete_selection(),
            "select_all" => app.select_all(),
            "combine_assist" => app.open_combine_assistant(),
            "fire_on" => {
                let d = &app.doc.device;
                let fire = crate::gcode::fire_on_lines(d.fire_power, d.s_max, d.dynamic_power);
                app.send_lines(fire);
            }
            "fire_off" => app.send_lines(crate::gcode::fire_off_lines()),
            "mirror_h" => app.mirror(true),
            "mirror_v" => app.mirror(false),
            "rot_cw" => app.rotate_sel(-90.0),
            "rot_ccw" => app.rotate_sel(90.0),
            "center_page" => app.center_on_page(),
            "group" => app.group(),
            "ungroup" => app.ungroup(),
            "dist_h_spaced" => app.distribute(0, true),
            "dist_v_spaced" => app.distribute(1, true),
            "dist_h_centered" => app.distribute(0, false),
            "dist_v_centered" => app.distribute(1, false),
            "together_h" => app.move_together(0),
            "together_v" => app.move_together(1),
            "offset" => app.open_offset(),
            "interval_test" => app.open_interval_test(),
            "material_test" => app.open_material_test(),
            "same_w" => app.make_same_size(0, ctx.input(|i| i.modifiers.shift)),
            "same_h" => app.make_same_size(1, ctx.input(|i| i.modifiers.shift)),
            "preview" => app.open_preview(),
            "frame" => app.frame_job(),
            "frame_outline" => app.frame_outline_job(),
            "home" => app.send_lines(vec![crate::gcode::HOME_CMD.into()]),
            "unlock" => app.send_lines(vec!["$X".into()]),
            "fit" => app.view.fit_pending = true,
            "zoom_in" => crate::canvas::zoom_relative(app, crate::canvas::ZOOM_IN_FACTOR),
            "zoom_out" => crate::canvas::zoom_relative(app, crate::canvas::ZOOM_OUT_FACTOR),
            "tab_laser" => app.right_tab = RightTab::Laser,
            "tab_move" => app.right_tab = RightTab::Move,
            "tab_console" => app.right_tab = RightTab::Console,
            "tab_shape" => app.right_tab = RightTab::Shape,
            "reset_layout" => app.reset_layout = true,
            "about" => app.show_about = true,
            "updates" => {
                app.show_updates = true;
                app.updates.start(ctx, false);
            }
            "settings" => app.show_settings = true,
            "lang_auto" => app.set_language(None, ctx),
            other if other.starts_with("lang_") => {
                app.set_language(crate::i18n::Lang::from_code(&other[5..]), ctx);
            }
            other => {
                if let Some(n) = other.strip_prefix("align_").and_then(|n| n.parse::<u8>().ok()) {
                    app.align(n);
                }
            }
        }
        ctx.request_repaint();
    }
}
