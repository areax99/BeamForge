//! The app's general settings (Impostazioni), laid out like the original's Settings dialog with
//! its six tabs: Impostazioni editor, Unità e Griglie, Schermo, Importa/Esporta, Fotocamera and
//! Estensioni (the person's pictures 3-8). Defaults are the values on those pictures. They are
//! kept in prefs.json in the settings folder.
//!
//! Every option of the original is here and remembered. The ones BeamForge acts on are listed in
//! MECHANICS.md ("Impostazioni"); the others show a note when hovered.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GridContrast {
    #[default]
    Low,
    Medium,
    High,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AppTheme {
    /// light or dark as the system is
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DxfUnit {
    #[default]
    Millimeters,
    Centimeters,
    Meters,
    Kilometers,
    Microns,
    Inches,
    Feet,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    // --- Impostazioni editor ---
    pub autosave_minutes: u32,
    pub beginner_mode: bool,
    pub ignore_out_of_bounds: bool,
    pub invert_wheel_zoom: bool,
    pub save_cut_selected_in_project: bool,
    pub default_cut_selected: bool,
    pub default_use_selection_origin: bool,
    pub save_job_origin_in_project: bool,
    pub save_move_rotate_scale_switches: bool,
    pub load_default_layer_settings: bool,
    pub select_after_drawing: bool,
    pub show_page_tabs: bool,
    pub send_usage_data: bool,
    pub ask_crash_reports: bool,
    pub use_system_clipboard: bool,
    pub error_sound_on_send_fail: bool,
    pub ignore_start_when_monitor_off: bool,
    pub start_button_no_focus: bool,
    pub save_job_log: bool,
    /// true: BeamForge's own Load/Save windows; false: the system's
    pub internal_file_dialogs: bool,
    pub default_font: String,
    pub default_text_height: f64,
    pub right_to_left_text: bool,
    pub double_click_edits_text: bool,
    // --- Unità e Griglie ---
    pub inches: bool,
    pub snap_to_objects: bool,
    pub snap_to_grid: bool,
    pub grid_contrast: GridContrast,
    pub grid_spacing: f64,
    pub snap_grid_distance: f64,
    pub click_tolerance_px: f64,
    pub snap_object_distance_px: f64,
    /// arrow-key moves in mm: with ⌘ (Ctrl), plain, with ⇧
    pub nudge_fine: f64,
    pub nudge: f64,
    pub nudge_coarse: f64,
    // --- Schermo ---
    pub antialiasing: bool,
    pub filled_rendering: bool,
    pub reduce_motion: bool,
    pub dark_background: bool,
    pub palette_labels: bool,
    pub screen_size_checks: bool,
    pub fullscreen_line_cursor: bool,
    pub show_rotary_toggle: bool,
    pub show_cylinder_toggle: bool,
    pub show_center_cross: bool,
    pub high_dpi_workspace: bool,
    /// 0..=4, 1 = Normale
    pub handle_size: u8,
    /// 0..=4, 1 = Normale
    pub toolbar_icon_size: u8,
    /// 0..=4, 3 = Alta
    pub curve_quality: u8,
    /// 0..=4, 0 = Default
    pub font_size: u8,
    pub theme: AppTheme,
    // --- Importa/Esporta ---
    pub group_imported: bool,
    pub import_hidden_ai_layers: bool,
    pub select_imported: bool,
    pub center_imported: bool,
    pub import_on_tool_layers: bool,
    pub dxf_auto_units: bool,
    pub dxf_unit: DxfUnit,
    pub dxf_ignore_text: bool,
    pub dxf_close_tolerance: f64,
    /// true: export arcs; false: arcs become lines
    pub dxf_export_arcs: bool,
    /// 96 (Inkscape) or 72 (Illustrator)
    pub svg_dpi: u32,
    pub svg_auto_dpi: bool,
    /// 1000 or 1016 units per inch
    pub hpgl_units: u32,
    pub curve_tolerance: f64,
    pub print_line_width: f64,
    // --- Fotocamera ---
    pub camera_recommended: bool,
    pub overlay_max_mb: u32,
    pub camera_preview_quality: u8,
    pub camera_preview_fps: f64,
    pub camera_default_fps: f64,
    // --- Estensioni ---
    pub api_network_access: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            autosave_minutes: 2,
            beginner_mode: false,
            ignore_out_of_bounds: true,
            invert_wheel_zoom: true,
            save_cut_selected_in_project: false,
            default_cut_selected: false,
            default_use_selection_origin: false,
            save_job_origin_in_project: false,
            save_move_rotate_scale_switches: false,
            load_default_layer_settings: true,
            select_after_drawing: false,
            show_page_tabs: true,
            // BeamForge sends nothing anywhere: these two stay off
            send_usage_data: false,
            ask_crash_reports: false,
            use_system_clipboard: true,
            error_sound_on_send_fail: true,
            ignore_start_when_monitor_off: false,
            start_button_no_focus: true,
            save_job_log: false,
            internal_file_dialogs: true,
            default_font: String::new(),
            default_text_height: 25.0,
            right_to_left_text: false,
            double_click_edits_text: false,
            inches: false,
            snap_to_objects: true,
            snap_to_grid: true,
            grid_contrast: GridContrast::Low,
            grid_spacing: 10.0,
            snap_grid_distance: 1.0,
            click_tolerance_px: 3.0,
            snap_object_distance_px: 10.0,
            nudge_fine: 1.0,
            nudge: 5.0,
            nudge_coarse: 20.0,
            antialiasing: true,
            filled_rendering: false,
            reduce_motion: false,
            dark_background: false,
            palette_labels: true,
            screen_size_checks: true,
            fullscreen_line_cursor: false,
            show_rotary_toggle: false,
            show_cylinder_toggle: false,
            show_center_cross: false,
            high_dpi_workspace: false,
            handle_size: 0,
            toolbar_icon_size: 1,
            curve_quality: 3,
            font_size: 0,
            theme: AppTheme::System,
            group_imported: false,
            import_hidden_ai_layers: false,
            select_imported: true,
            center_imported: false,
            import_on_tool_layers: false,
            dxf_auto_units: true,
            dxf_unit: DxfUnit::Millimeters,
            dxf_ignore_text: false,
            dxf_close_tolerance: 0.01,
            dxf_export_arcs: true,
            svg_dpi: 96,
            svg_auto_dpi: true,
            hpgl_units: 1016,
            curve_tolerance: 0.05,
            print_line_width: 0.2,
            camera_recommended: true,
            overlay_max_mb: 200,
            camera_preview_quality: 0,
            camera_preview_fps: 2.5,
            camera_default_fps: 1.0,
            api_network_access: false,
        }
    }
}

const FILE: &str = "prefs.json";

impl Prefs {
    pub fn load() -> Self {
        crate::i18n::load_json(FILE).unwrap_or_default()
    }

    pub fn save(&self) {
        crate::i18n::save_json(FILE, self);
    }

    /// "Controlla automaticamente gli aggiornamenti" (update_auto.json, shared with Aggiornamenti).
    pub fn check_updates() -> bool {
        crate::i18n::load_json::<bool>("update_auto.json").unwrap_or(true)
    }

    pub fn set_check_updates(on: bool) {
        crate::i18n::save_json("update_auto.json", &on);
    }
}
