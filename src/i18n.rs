//! Translations. The Italian text is the key; English, Spanish, German and French sit beside it.
//! The language follows the system unless the person picks one in Impostazioni (saved on disk).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    It = 0,
    En = 1,
    Es = 2,
    De = 3,
    Fr = 4,
}

pub const ALL: [Lang; 5] = [Lang::It, Lang::En, Lang::Es, Lang::De, Lang::Fr];

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::It => "it",
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::Fr => "fr",
        }
    }
    /// The language's own name, shown the same in every language.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::It => "Italiano",
            Lang::En => "English",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
        }
    }
    pub fn from_code(c: &str) -> Option<Lang> {
        let c = c.to_lowercase();
        let p = c.split(['-', '_']).next().unwrap_or("");
        ALL.into_iter().find(|l| l.code() == p)
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    ALL[CURRENT.load(Ordering::Relaxed) as usize]
}

pub fn set(l: Lang) {
    CURRENT.store(l as u8, Ordering::Relaxed);
}

/// The system's language when it is one BeamForge speaks, otherwise English.
pub fn system_lang() -> Lang {
    sys_locale::get_locales().find_map(|l| Lang::from_code(&l)).unwrap_or(Lang::En)
}

/// Saved choice: None = follow the system.
pub fn load_choice() -> Option<Lang> {
    let s = std::fs::read_to_string(settings_path()?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    v.get("language").and_then(|l| l.as_str()).and_then(Lang::from_code)
}

pub fn save_choice(choice: Option<Lang>) {
    let Some(p) = settings_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let v = serde_json::json!({ "language": choice.map(|l| l.code()).unwrap_or("auto") });
    let _ = std::fs::write(p, v.to_string());
}

/// Defaults saved with Rendi predefinito, per layer index, beside the settings file.
/// Read a JSON file kept beside the settings (laser test settings and presets).
pub fn load_json<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    settings_path().and_then(|p| std::fs::read_to_string(p.with_file_name(name)).ok()).and_then(|s| serde_json::from_str(&s).ok())
}

pub fn save_json<T: serde::Serialize>(name: &str, value: &T) {
    let Some(p) = settings_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(p.with_file_name(name), s);
    }
}

pub fn load_cut_defaults() -> std::collections::HashMap<usize, crate::model::CutSetting> {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p.with_file_name("cut_defaults.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_cut_defaults(d: &std::collections::HashMap<usize, crate::model::CutSetting>) {
    let Some(p) = settings_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(d) {
        let _ = std::fs::write(p.with_file_name("cut_defaults.json"), s);
    }
}

/// The system's app-data folder, like the original's settings folder (_GetSettingsFolder
/// 0x1009444e4: QStandardPaths::writableLocation(AppDataLocation)):
/// macOS ~/Library/Application Support/BeamForge, Windows %APPDATA%\BeamForge,
/// Linux $XDG_DATA_HOME/BeamForge (default ~/.local/share/BeamForge).
pub fn settings_folder() -> Option<std::path::PathBuf> {
    settings_path().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn settings_path() -> Option<std::path::PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let mut p = if cfg!(target_os = "macos") {
        let mut h = var("HOME")?;
        h.push("Library/Application Support");
        h
    } else if cfg!(windows) {
        var("APPDATA").or_else(|| var("USERPROFILE").map(|mut u| {
            u.push("AppData\\Roaming");
            u
        }))?
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|mut h| {
            h.push(".local/share");
            h
        }))?
    };
    p.push("BeamForge");
    p.push("settings.json");
    Some(p)
}

/// Decimal separator taken from the system locale, as the original does when it loads a
/// language (MainWindow::loadLanguage 0x100c40810: QLocale::system().decimalPoint()).
/// English, Chinese, Japanese and Korean locales use '.', the others ','.
pub fn decimal_char() -> char {
    static C: OnceLock<char> = OnceLock::new();
    *C.get_or_init(|| {
        let loc = sys_locale::get_locale().unwrap_or_default().to_lowercase();
        let lang = loc.split(['-', '_']).next().unwrap_or("");
        if ["en", "zh", "ja", "ko", "he", "th", ""].contains(&lang) { '.' } else { ',' }
    })
}

/// A number field that shows and accepts the system's decimal separator.
pub fn dv<'a, N: eframe::egui::emath::Numeric>(value: &'a mut N) -> eframe::egui::DragValue<'a> {
    let sep = decimal_char();
    eframe::egui::DragValue::new(value)
        .custom_formatter(move |v, range| {
            let s = eframe::egui::emath::format_with_decimals_in_range(v, range);
            if sep == '.' { s } else { s.replace('.', &sep.to_string()) }
        })
        .custom_parser(|s| s.trim().replace(',', ".").parse::<f64>().ok())
}

/// "Annulla" as the Cancel button of a dialog (the same Italian word as Undo).
pub fn cancel() -> &'static str {
    match current() {
        Lang::It => "Annulla",
        Lang::En => "Cancel",
        Lang::Es => "Cancelar",
        Lang::De => "Abbrechen",
        Lang::Fr => "Annuler",
    }
}

/// Translate a fixed Italian text.
pub fn tr(it: &'static str) -> &'static str {
    let l = current();
    if l == Lang::It {
        return it;
    }
    table().get(it).map(|t| t[l as usize - 1]).unwrap_or(it)
}

/// Translate a text with `{}` placeholders, filled in order.
pub fn trf(it: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut parts = tr(it).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        if let Some(a) = args.get(i) {
            out.push_str(&a.to_string());
        }
        out.push_str(part);
    }
    out
}

fn table() -> &'static HashMap<&'static str, [&'static str; 4]> {
    static T: OnceLock<HashMap<&'static str, [&'static str; 4]>> = OnceLock::new();
    T.get_or_init(|| ROWS.iter().map(|(k, v)| (*k, *v)).collect())
}

// (italiano, [english, español, deutsch, français])
static ROWS: &[(&str, [&str; 4])] = &[
    ("Clicca per scegliere la forma", ["Click to choose the shape", "Haz clic para elegir la forma", "Klicken, um die Form zu wählen", "Cliquez pour choisir la forme"]),
    ("Combina forme (2 oggetti selezionati): clicca per scegliere l'operazione", ["Combine shapes (2 selected objects): click to choose the operation", "Combinar formas (2 objetos seleccionados): haz clic para elegir la operación", "Formen kombinieren (2 ausgewählte Objekte): klicken, um die Operation zu wählen", "Combiner les formes (2 objets sélectionnés) : cliquez pour choisir l'opération"]),
    ("Combina forme", ["Combine shapes", "Combinar formas", "Formen kombinieren", "Combiner les formes"]),
    ("Combina forme (2 oggetti selezionati)", ["Combine shapes (2 selected objects)", "Combinar formas (2 objetos seleccionados)", "Formen kombinieren (2 ausgewählte Objekte)", "Combiner les formes (2 objets sélectionnés)"]),
    ("Unisci forme", ["Join shapes", "Unir formas", "Formen vereinen", "Réunir les formes"]),
    ("Sottrai forme", ["Cut out shape", "Restar formas", "Form ausschneiden", "Soustraire les formes"]),
    ("Interseca forme", ["Keep overlap", "Intersecar formas", "Überlappung behalten", "Garder le chevauchement"]),
    ("Sottrai al contrario", ["Cut out the other way", "Restar al revés", "Umgekehrt ausschneiden", "Soustraire à l'inverse"]),
    ("Assistente combinazione", ["Combine assistant", "Asistente de combinación", "Kombinations-Assistent", "Assistant de combinaison"]),
    ("Assistente combinazione…", ["Combine assistant…", "Asistente de combinación…", "Kombinations-Assistent…", "Assistant de combinaison…"]),
    ("Seleziona esattamente 2 oggetti (il primo scelto è quello da cui si sottrae).", ["Select exactly 2 objects (the first one picked is the one cut from).", "Selecciona exactamente 2 objetos (al primero elegido se le resta).", "Genau 2 Objekte auswählen (vom zuerst gewählten wird abgezogen).", "Sélectionnez exactement 2 objets (on soustrait du premier choisi)."]),
    ("Il risultato è vuoto: le forme non cambiano.", ["The result is empty: the shapes stay as they are.", "El resultado está vacío: las formas no cambian.", "Das Ergebnis ist leer: die Formen bleiben unverändert.", "Le résultat est vide : les formes ne changent pas."]),
    ("Scegli come combinare i 2 oggetti selezionati: il risultato compare subito sul disegno.", ["Choose how to combine the 2 selected objects: the result shows on the drawing right away.", "Elige cómo combinar los 2 objetos seleccionados: el resultado aparece enseguida en el dibujo.", "Wähle, wie die 2 ausgewählten Objekte kombiniert werden: das Ergebnis erscheint sofort in der Zeichnung.", "Choisissez comment combiner les 2 objets sélectionnés : le résultat s'affiche aussitôt sur le dessin."]),
    ("Unisci", ["Join", "Unir", "Vereinen", "Réunir"]),
    ("Interseca", ["Overlap", "Intersecar", "Überlappung", "Chevauchement"]),
    ("Primo − secondo", ["First − second", "Primero − segundo", "Erstes − zweites", "Premier − second"]),
    ("Secondo − primo", ["Second − first", "Segundo − primero", "Zweites − erstes", "Second − premier"]),
    ("Originali", ["Originals", "Originales", "Originale", "Originaux"]),
    ("Forme molto complesse: il risultato si vede al clic, non al passaggio del mouse.", ["Very complex shapes: the result shows on click, not on hover.", "Formas muy complejas: el resultado se ve al hacer clic, no al pasar el ratón.", "Sehr komplexe Formen: das Ergebnis erscheint beim Klicken, nicht beim Überfahren.", "Formes très complexes : le résultat s'affiche au clic, pas au survol."]),
    ("Triangolo", ["Triangle", "Triángulo", "Dreieck", "Triangle"]),
    ("Pentagono", ["Pentagon", "Pentágono", "Fünfeck", "Pentagone"]),
    ("Ottagono", ["Octagon", "Octógono", "Achteck", "Octogone"]),
    ("Stella", ["Star", "Estrella", "Stern", "Étoile"]),
    ("Stella doppia", ["Dual Star", "Estrella doble", "Doppelstern", "Étoile double"]),
    ("Punte", ["Points", "Puntas", "Zacken", "Branches"]),
    ("Raggio interno (%)", ["Inner radius (%)", "Radio interior (%)", "Innenradius (%)", "Rayon intérieur (%)"]),
    ("Raggio medio (%)", ["Middle radius (%)", "Radio medio (%)", "Mittlerer Radius (%)", "Rayon moyen (%)"]),
    ("Periferiche…", ["Devices…", "Dispositivos…", "Geräte…", "Périphériques…"]),
    ("Elenco periferiche personali", ["Personal device list", "Lista de dispositivos personales", "Persönliche Geräteliste", "Liste des périphériques personnels"]),
    ("Nessuna macchina", ["No Machine", "Ninguna máquina", "Keine Maschine", "Aucune machine"]),
    ("Usa «Trova il mio laser» o «Crea manualmente»", ["Use “Find My Laser” or “Create Manually”", "Usa «Buscar mi láser» o «Crear manualmente»", "„Meinen Laser finden“ oder „Manuell erstellen“ verwenden", "Utilisez « Trouver mon laser » ou « Créer manuellement »"]),
    ("Trova il mio laser", ["Find My Laser", "Buscar mi láser", "Meinen Laser finden", "Trouver mon laser"]),
    ("Crea manualmente", ["Create Manually", "Crear manualmente", "Manuell erstellen", "Créer manuellement"]),
    ("Importa", ["Import", "Importar", "Importieren", "Importer"]),
    ("Esporta", ["Export", "Exportar", "Exportieren", "Exporter"]),
    ("Rimuovi", ["Remove", "Quitar", "Entfernen", "Supprimer"]),
    ("File del dispositivo non valido", ["Invalid device file", "Archivo de dispositivo no válido", "Ungültige Gerätedatei", "Fichier de périphérique non valide"]),
    ("Il mio laser", ["My laser", "Mi láser", "Mein Laser", "Mon laser"]),
    ("porta automatica", ["automatic port", "puerto automático", "automatischer Port", "port automatique"]),
    ("Cerco i dispositivi collegati…", ["Scanning for connected devices…", "Buscando dispositivos conectados…", "Suche nach verbundenen Geräten…", "Recherche des périphériques connectés…"]),
    ("Provo {} a {} baud…", ["Trying {} at {} baud…", "Probando {} a {} baudios…", "Teste {} mit {} Baud…", "Essai de {} à {} bauds…"]),
    ("Nessun laser trovato.", ["No lasers found.", "No se encontró ningún láser.", "Kein Laser gefunden.", "Aucun laser trouvé."]),
    ("Trovati {} dispositivi.", ["Found {} devices.", "Se encontraron {} dispositivos.", "{} Geräte gefunden.", "{} périphériques trouvés."]),
    ("Collega il laser con il cavo USB e accendilo. BeamForge prova ogni porta USB e ascolta la risposta del controller.", ["Connect the laser with its USB cable and switch it on. BeamForge tries every USB port and listens for the controller's answer.", "Conecta el láser con el cable USB y enciéndelo. BeamForge prueba cada puerto USB y escucha la respuesta del controlador.", "Laser per USB-Kabel verbinden und einschalten. BeamForge prüft jeden USB-Port und wartet auf die Antwort des Controllers.", "Branchez le laser avec son câble USB et allumez-le. BeamForge essaie chaque port USB et écoute la réponse du contrôleur."]),
    ("Nessun controller ha risposto. Controlla cavo, alimentazione e driver USB, poi riprova, oppure crea il dispositivo a mano.", ["No controller answered. Check the cable, power and USB driver and try again, or create the device manually.", "Ningún controlador respondió. Revisa cable, alimentación y driver USB y vuelve a intentarlo, o crea el dispositivo manualmente.", "Kein Controller hat geantwortet. Kabel, Strom und USB-Treiber prüfen und erneut versuchen oder das Gerät manuell anlegen.", "Aucun contrôleur n'a répondu. Vérifiez le câble, l'alimentation et le pilote USB puis réessayez, ou créez le périphérique manuellement."]),
    ("In attesa di risposte…", ["Waiting for answers…", "Esperando respuestas…", "Warte auf Antworten…", "En attente de réponses…"]),
    ("area sconosciuta", ["unknown area", "área desconocida", "Bereich unbekannt", "zone inconnue"]),
    ("modalità laser attiva", ["laser mode on", "modo láser activo", "Lasermodus an", "mode laser activé"]),
    ("modalità laser spenta ($32=0)", ["laser mode off ($32=0)", "modo láser desactivado ($32=0)", "Lasermodus aus ($32=0)", "mode laser désactivé ($32=0)"]),
    ("Cerca di nuovo", ["Search again", "Buscar de nuevo", "Erneut suchen", "Rechercher à nouveau"]),
    ("Avanti", ["Next", "Siguiente", "Weiter", "Suivant"]),
    ("Modifica dispositivo", ["Edit device", "Editar dispositivo", "Gerät bearbeiten", "Modifier le périphérique"]),
    ("Nuovo dispositivo", ["New device", "Nuevo dispositivo", "Neues Gerät", "Nouveau périphérique"]),
    ("Controller", ["Controller", "Controlador", "Controller", "Contrôleur"]),
    ("Porta", ["Port", "Puerto", "Port", "Port"]),
    ("Automatica (la prima trovata)", ["Automatic (first one found)", "Automático (el primero encontrado)", "Automatisch (erster gefundener)", "Automatique (le premier trouvé)"]),
    ("Nessuna periferica: apri Periferiche", ["No devices: open Devices", "Sin dispositivos: abre Dispositivos", "Keine Geräte: Geräte öffnen", "Aucun périphérique : ouvrez Périphériques"]),
    ("CH340: probabilmente GRBL", ["CH340: probably GRBL", "CH340: probablemente GRBL", "CH340: wahrscheinlich GRBL", "CH340 : probablement GRBL"]),
    ("STM32: probabilmente Gerbil-STM", ["STM32: probably Gerbil-STM", "STM32: probablemente Gerbil-STM", "STM32: wahrscheinlich Gerbil-STM", "STM32 : probablement Gerbil-STM"]),
    ("Atmega: probabilmente GRBL, forse LPC", ["Atmega: probably GRBL, could be LPC", "Atmega: probablemente GRBL, quizá LPC", "Atmega: wahrscheinlich GRBL, evtl. LPC", "Atmega : probablement GRBL, peut-être LPC"]),
    ("Arduino Uno: probabilmente GRBL", ["Arduino Uno: probably GRBL", "Arduino Uno: probablemente GRBL", "Arduino Uno: wahrscheinlich GRBL", "Arduino Uno : probablement GRBL"]),
    ("FTDI: probabile Ruida / TopWisdom (forse GRBL)", ["FTDI: probably Ruida / TopWisdom (possibly GRBL)", "FTDI: probablemente Ruida / TopWisdom (quizá GRBL)", "FTDI: wahrscheinlich Ruida / TopWisdom (evtl. GRBL)", "FTDI : probablement Ruida / TopWisdom (peut-être GRBL)"]),
    // cut modes and options
    ("Linea", ["Line", "Línea", "Linie", "Ligne"]),
    ("Riempi", ["Fill", "Relleno", "Füllen", "Remplissage"]),
    ("Immagine", ["Image", "Imagen", "Bild", "Image"]),
    ("Soglia", ["Threshold", "Umbral", "Schwellwert", "Seuil"]),
    ("Retinato (Floyd-Steinberg)", ["Dithered (Floyd-Steinberg)", "Tramado (Floyd-Steinberg)", "Gerastert (Floyd-Steinberg)", "Tramé (Floyd-Steinberg)"]),
    ("Scala di grigi", ["Grayscale", "Escala de grises", "Graustufen", "Niveaux de gris"]),
    ("Coordinate assolute", ["Absolute Coords", "Coordenadas absolutas", "Absolute Koordinaten", "Coordonnées absolues"]),
    ("Origine utente", ["User Origin", "Origen de usuario", "Benutzerursprung", "Origine utilisateur"]),
    ("Posizione corrente", ["Current Position", "Posición actual", "Aktuelle Position", "Position actuelle"]),
    // tools
    ("Seleziona", ["Select", "Seleccionar", "Auswählen", "Sélectionner"]),
    ("Disegna linee", ["Draw Lines", "Dibujar líneas", "Linien zeichnen", "Dessiner des lignes"]),
    ("Rettangolo", ["Rectangle", "Rectángulo", "Rechteck", "Rectangle"]),
    ("Ellisse", ["Ellipse", "Elipse", "Ellipse", "Ellipse"]),
    ("Poligono", ["Polygon", "Polígono", "Polygon", "Polygone"]),
    ("Testo", ["Text", "Texto", "Text", "Texte"]),
    ("Posiziona laser", ["Position Laser", "Posicionar láser", "Laser positionieren", "Positionner le laser"]),
    // menus
    ("File", ["File", "Archivo", "Datei", "Fichier"]),
    ("Nuovo", ["New", "Nuevo", "Neu", "Nouveau"]),
    ("Apri…", ["Open…", "Abrir…", "Öffnen…", "Ouvrir…"]),
    ("Salva", ["Save", "Guardar", "Speichern", "Enregistrer"]),
    ("Salva con nome…", ["Save As…", "Guardar como…", "Speichern unter…", "Enregistrer sous…"]),
    ("Importa…", ["Import…", "Importar…", "Importieren…", "Importer…"]),
    ("Esporta G-code…", ["Export G-code…", "Exportar G-code…", "G-Code exportieren…", "Exporter le G-code…"]),
    ("Esporta SVG…", ["Export SVG…", "Exportar SVG…", "SVG exportieren…", "Exporter en SVG…"]),
    ("Esci", ["Quit", "Salir", "Beenden", "Quitter"]),
    ("Modifica", ["Edit", "Editar", "Bearbeiten", "Édition"]),
    ("Annulla", ["Undo", "Deshacer", "Rückgängig", "Annuler"]),
    ("Ripeti", ["Redo", "Rehacer", "Wiederholen", "Rétablir"]),
    ("Taglia", ["Cut", "Cortar", "Ausschneiden", "Couper"]),
    ("Copia", ["Copy", "Copiar", "Kopieren", "Copier"]),
    ("Incolla", ["Paste", "Pegar", "Einfügen", "Coller"]),
    ("Duplica", ["Duplicate", "Duplicar", "Duplizieren", "Dupliquer"]),
    ("Elimina", ["Delete", "Eliminar", "Löschen", "Supprimer"]),
    ("Seleziona tutto", ["Select All", "Seleccionar todo", "Alles auswählen", "Tout sélectionner"]),
    ("Impostazioni dispositivo…", ["Device Settings…", "Ajustes del dispositivo…", "Geräteeinstellungen…", "Réglages de l'appareil…"]),
    ("Impostazioni dispositivo", ["Device Settings", "Ajustes del dispositivo", "Geräteeinstellungen", "Réglages de l'appareil"]),
    ("Strumenti", ["Tools", "Herramientas", "Werkzeuge", "Outils"]),
    ("Organizza", ["Arrange", "Organizar", "Anordnen", "Organiser"]),
    ("Raggruppa (Ctrl+G)", ["Group (Ctrl+G)", "Agrupar (Ctrl+G)", "Gruppieren (Strg+G)", "Grouper (Ctrl+G)"]),
    ("Separa (Ctrl+Shift+G)", ["Ungroup (Ctrl+Shift+G)", "Desagrupar (Ctrl+Mayús+G)", "Gruppierung aufheben (Strg+Umschalt+G)", "Dissocier (Ctrl+Maj+G)"]),
    ("Raggruppa (⌘G)", ["Group (⌘G)", "Agrupar (⌘G)", "Gruppieren (⌘G)", "Grouper (⌘G)"]),
    ("Separa (⇧⌘G)", ["Ungroup (⇧⌘G)", "Desagrupar (⇧⌘G)", "Gruppierung aufheben (⇧⌘G)", "Dissocier (⇧⌘G)"]),
    ("Raggruppa", ["Group", "Agrupar", "Gruppieren", "Grouper"]),
    ("Separa", ["Ungroup", "Desagrupar", "Gruppierung aufheben", "Dissocier"]),
    ("Allinea", ["Align", "Alinear", "Ausrichten", "Aligner"]),
    ("Allinea (all'ultimo selezionato)", ["Align (to the last selected)", "Alinear (al último seleccionado)", "Ausrichten (am zuletzt gewählten)", "Aligner (sur le dernier sélectionné)"]),
    ("Allinea a sinistra", ["Align Left", "Alinear a la izquierda", "Links ausrichten", "Aligner à gauche"]),
    ("Allinea a sinistra (all'ultimo selezionato)", ["Align Left (to the last selected)", "Alinear a la izquierda (al último seleccionado)", "Links ausrichten (am zuletzt gewählten)", "Aligner à gauche (sur le dernier sélectionné)"]),
    ("Allinea a destra", ["Align Right", "Alinear a la derecha", "Rechts ausrichten", "Aligner à droite"]),
    ("Allinea in alto", ["Align Top", "Alinear arriba", "Oben ausrichten", "Aligner en haut"]),
    ("Allinea in basso", ["Align Bottom", "Alinear abajo", "Unten ausrichten", "Aligner en bas"]),
    ("Allinea centri orizzontali", ["Align Horizontal Centers", "Alinear centros horizontales", "Horizontale Mitten ausrichten", "Aligner les centres horizontaux"]),
    ("Allinea centri verticali", ["Align Vertical Centers", "Alinear centros verticales", "Vertikale Mitten ausrichten", "Aligner les centres verticaux"]),
    ("Allinea centri", ["Align Centers", "Alinear centros", "Mitten ausrichten", "Aligner les centres"]),
    ("Distribuisci", ["Distribute", "Distribuir", "Verteilen", "Répartir"]),
    ("Spaziatura orizzontale", ["Horizontal Spacing", "Espaciado horizontal", "Horizontaler Abstand", "Espacement horizontal"]),
    ("Spaziatura verticale", ["Vertical Spacing", "Espaciado vertical", "Vertikaler Abstand", "Espacement vertical"]),
    ("Centri orizzontali", ["Horizontal Centers", "Centros horizontales", "Horizontale Mitten", "Centres horizontaux"]),
    ("Centri verticali", ["Vertical Centers", "Centros verticales", "Vertikale Mitten", "Centres verticaux"]),
    ("Distribuisci spaziatura orizzontale", ["Distribute Horizontal Spacing", "Distribuir espaciado horizontal", "Horizontal gleichmäßig verteilen", "Répartir l'espacement horizontal"]),
    ("Distribuisci spaziatura verticale", ["Distribute Vertical Spacing", "Distribuir espaciado vertical", "Vertikal gleichmäßig verteilen", "Répartir l'espacement vertical"]),
    ("Distribuisci centri orizzontali", ["Distribute Horizontal Centers", "Distribuir centros horizontales", "Horizontale Mitten verteilen", "Répartir les centres horizontaux"]),
    ("Distribuisci centri verticali", ["Distribute Vertical Centers", "Distribuir centros verticales", "Vertikale Mitten verteilen", "Répartir les centres verticaux"]),
    ("Avvicina", ["Move Together", "Juntar", "Zusammenschieben", "Rapprocher"]),
    ("Orizzontalmente", ["Horizontally", "Horizontalmente", "Horizontal", "Horizontalement"]),
    ("Verticalmente", ["Vertically", "Verticalmente", "Vertikal", "Verticalement"]),
    ("Avvicina orizzontalmente", ["Move Together Horizontally", "Juntar horizontalmente", "Horizontal zusammenschieben", "Rapprocher horizontalement"]),
    ("Avvicina verticalmente", ["Move Together Vertically", "Juntar verticalmente", "Vertikal zusammenschieben", "Rapprocher verticalement"]),
    ("Specchia orizzontalmente", ["Mirror Horizontally", "Reflejar horizontalmente", "Horizontal spiegeln", "Miroir horizontal"]),
    ("Specchia verticalmente", ["Mirror Vertically", "Reflejar verticalmente", "Vertikal spiegeln", "Miroir vertical"]),
    ("Ruota 90° in senso orario", ["Rotate 90° Clockwise", "Girar 90° en sentido horario", "90° im Uhrzeigersinn drehen", "Rotation 90° horaire"]),
    ("Ruota 90° in senso antiorario", ["Rotate 90° Counter-clockwise", "Girar 90° en sentido antihorario", "90° gegen den Uhrzeigersinn drehen", "Rotation 90° antihoraire"]),
    ("Ruota 90° orario", ["Rotate 90° clockwise", "Girar 90° horario", "90° im Uhrzeigersinn", "Rotation 90° horaire"]),
    ("Centra orizzontalmente", ["Center Horizontally", "Centrar horizontalmente", "Horizontal zentrieren", "Centrer horizontalement"]),
    ("Centra verticalmente", ["Center Vertically", "Centrar verticalmente", "Vertikal zentrieren", "Centrer verticalement"]),
    ("Sposta al centro della pagina", ["Move to Page Center", "Mover al centro de la página", "In die Seitenmitte verschieben", "Déplacer au centre de la page"]),
    ("Centra nella pagina", ["Center on page", "Centrar en la página", "Auf der Seite zentrieren", "Centrer sur la page"]),
    ("Strumenti laser", ["Laser Tools", "Herramientas láser", "Laser-Werkzeuge", "Outils laser"]),
    ("Anteprima", ["Preview", "Vista previa", "Vorschau", "Aperçu"]),
    ("Inquadra", ["Frame", "Encuadrar", "Umriss fahren", "Cadrer"]),
    ("Inquadra forme", ["Rubber-band frame", "Encuadrar formas", "Formumriss fahren", "Cadrer les formes"]),
    ("Percorre col laser spento il contorno che avvolge le forme, alla velocità del pannello Sposta.", ["Traces the boundary around the shapes with the beam off, at the Move panel speed.", "Recorre con el láser apagado el contorno que envuelve las formas, a la velocidad del panel Mover.", "Fährt bei ausgeschaltetem Laser die Hülle um die Formen ab, mit der Geschwindigkeit des Bewegen-Felds.", "Parcourt laser éteint le contour qui entoure les formes, à la vitesse du panneau Déplacer."]),
    ("In cerca del laser…", ["Looking for the laser…", "Buscando el láser…", "Suche Laser…", "Recherche du laser…"]),
    ("Nessun laser trovato: collegalo con il cavo USB oppure usa Salva G-code.", ["No laser found: plug it in with the USB cable or use Save G-code.", "No se encontró ningún láser: conéctalo con el cable USB o usa Guardar G-code.", "Kein Laser gefunden: per USB-Kabel anschließen oder G-Code speichern verwenden.", "Aucun laser trouvé : branchez-le avec le câble USB ou utilisez Enregistrer le G-code."]),
    ("Nessun laser trovato: collegalo con il cavo USB.", ["No laser found: plug it in with the USB cable.", "No se encontró ningún láser: conéctalo con el cable USB.", "Kein Laser gefunden: per USB-Kabel anschließen.", "Aucun laser trouvé : branchez-le avec le câble USB."]),
    ("Home ($H)", ["Home ($H)", "Inicio ($H)", "Referenzfahrt ($H)", "Origine machine ($H)"]),
    ("Sblocca ($X)", ["Unlock ($X)", "Desbloquear ($X)", "Entsperren ($X)", "Déverrouiller ($X)"]),
    ("Finestra", ["Window", "Ventana", "Fenster", "Fenêtre"]),
    ("Adatta alla pagina", ["Zoom to Page", "Ajustar a la página", "An Seite anpassen", "Ajuster à la page"]),
    ("Ingrandisci", ["Zoom In", "Acercar", "Vergrößern", "Zoom avant"]),
    ("Riduci", ["Zoom Out", "Alejar", "Verkleinern", "Zoom arrière"]),
    ("Laser", ["Laser", "Láser", "Laser", "Laser"]),
    ("Sposta", ["Move", "Mover", "Bewegen", "Déplacer"]),
    ("Console", ["Console", "Consola", "Konsole", "Console"]),
    ("Proprietà forma", ["Shape Properties", "Propiedades de forma", "Formeigenschaften", "Propriétés de la forme"]),
    ("Forma", ["Shape", "Forma", "Form", "Forme"]),
    ("Aiuto", ["Help", "Ayuda", "Hilfe", "Aide"]),
    ("Guida", ["Help", "Ayuda", "Hilfe", "Aide"]),
    ("Informazioni su BeamForge", ["About BeamForge", "Acerca de BeamForge", "Über BeamForge", "À propos de BeamForge"]),
    ("Servizi", ["Services", "Servicios", "Dienste", "Services"]),
    ("Nascondi BeamForge", ["Hide BeamForge", "Ocultar BeamForge", "BeamForge ausblenden", "Masquer BeamForge"]),
    ("Nascondi altre", ["Hide Others", "Ocultar otros", "Andere ausblenden", "Masquer les autres"]),
    ("Mostra tutte", ["Show All", "Mostrar todo", "Alle einblenden", "Tout afficher"]),
    ("Punto di riferimento: posizione, dimensioni e rotazione partono da qui", ["Reference point: position, size and rotation work from here", "Punto de referencia: posición, tamaño y rotación parten de aquí", "Bezugspunkt: Position, Größe und Drehung gehen von hier aus", "Point de référence : position, taille et rotation partent d'ici"]),
    ("Seleziona almeno 2 oggetti: prendono la misura dell'ultimo selezionato.", ["Select at least 2 objects: they take the size of the last one selected.", "Selecciona al menos 2 objetos: toman la medida del último seleccionado.", "Wähle mindestens 2 Objekte: sie übernehmen die Größe des zuletzt gewählten.", "Sélectionnez au moins 2 objets : ils prennent la taille du dernier sélectionné."]),
    ("Stessa larghezza", ["Same width", "Mismo ancho", "Gleiche Breite", "Même largeur"]),
    ("Stessa altezza", ["Same height", "Misma altura", "Gleiche Höhe", "Même hauteur"]),
    ("Stessa larghezza (dell'ultimo selezionato; Maiusc: in proporzione)", ["Make same width (as the last selected; Shift: keep proportions)", "Mismo ancho (del último seleccionado; Mayús: proporcional)", "Gleiche Breite (wie das zuletzt gewählte; Umschalt: proportional)", "Même largeur (que le dernier sélectionné ; Maj : proportionnel)"]),
    ("Stessa altezza (dell'ultimo selezionato; Maiusc: in proporzione)", ["Make same height (as the last selected; Shift: keep proportions)", "Misma altura (del último seleccionado; Mayús: proporcional)", "Gleiche Höhe (wie das zuletzt gewählte; Umschalt: proportional)", "Même hauteur (que le dernier sélectionné ; Maj : proportionnel)"]),
    ("Offset", ["Offset", "Desplazamiento", "Versatz", "Décalage"]),
    ("Offset…", ["Offset…", "Desplazamiento…", "Versatz…", "Décalage…"]),
    ("Offset (crea un contorno a distanza dalle forme selezionate)", ["Offset (make an outline at a distance from the selected shapes)", "Desplazamiento (crea un contorno a distancia de las formas seleccionadas)", "Versatz (erzeugt eine Kontur im Abstand zu den gewählten Formen)", "Décalage (crée un contour à distance des formes sélectionnées)"]),
    ("Distanza offset", ["Offset distance", "Distancia", "Versatzabstand", "Distance de décalage"]),
    ("Direzione", ["Direction", "Dirección", "Richtung", "Direction"]),
    ("Esterno", ["Outward", "Exterior", "Nach außen", "Extérieur"]),
    ("Interno", ["Inward", "Interior", "Nach innen", "Intérieur"]),
    ("Entrambe", ["Both", "Ambos", "Beide", "Les deux"]),
    ("Stile angolo", ["Corner style", "Estilo de esquina", "Eckenstil", "Style d'angle"]),
    ("Arrotondato", ["Rounded", "Redondeado", "Abgerundet", "Arrondi"]),
    ("Smussatura", ["Square", "Biselado", "Abgeschrägt", "Biseau"]),
    ("Angolo vivo", ["Mitered", "Inglete", "Spitz", "Pointu"]),
    ("Solo forme esterne", ["Outer shapes only", "Solo formas exteriores", "Nur äußere Formen", "Formes extérieures seulement"]),
    ("Seleziona oggetti risultanti", ["Select resulting objects", "Seleccionar objetos resultantes", "Ergebnisobjekte auswählen", "Sélectionner les objets obtenus"]),
    ("Elimina oggetti originali", ["Delete original objects", "Eliminar objetos originales", "Originalobjekte löschen", "Supprimer les objets d'origine"]),
    ("Ottimizza / Semplifica risultati", ["Optimize / Simplify results", "Optimizar / Simplificar resultados", "Ergebnisse optimieren / vereinfachen", "Optimiser / Simplifier les résultats"]),
    ("Alcune forme selezionate si sovrappongono: l'offset viene applicato a ciascuna separatamente.", ["Some of the selected shapes overlap: the offset is applied to each one separately.", "Algunas formas seleccionadas se superponen: el desplazamiento se aplica a cada una por separado.", "Einige gewählte Formen überlappen: der Versatz wird auf jede einzeln angewendet.", "Certaines formes sélectionnées se chevauchent : le décalage est appliqué à chacune séparément."]),
    ("Seleziona una o più forme da cui creare l'offset.", ["Select one or more shapes to offset.", "Selecciona una o más formas para desplazar.", "Wähle eine oder mehrere Formen für den Versatz.", "Sélectionnez une ou plusieurs formes à décaler."]),
    ("L'offset non ha prodotto forme.", ["The offset produced no shapes.", "El desplazamiento no produjo formas.", "Der Versatz hat keine Formen erzeugt.", "Le décalage n'a produit aucune forme."]),
    ("Offset: create {} forme", ["Offset: {} shapes created", "Desplazamiento: {} formas creadas", "Versatz: {} Formen erstellt", "Décalage : {} formes créées"]),
    ("Test intervallo", ["Interval Test", "Prueba de intervalo", "Intervalltest", "Test d'intervalle"]),
    ("Test materiale", ["Material Test", "Prueba de material", "Materialtest", "Test de matériau"]),
    ("Test di intervallo", ["Interval Test", "Prueba de intervalo", "Intervalltest", "Test d'intervalle"]),
    ("Generatore test del materiale", ["Material Test Generator", "Generador de prueba de material", "Materialtest-Generator", "Générateur de test de matériau"]),
    ("Potenza", ["Power", "Potencia", "Leistung", "Puissance"]),
    ("Intervallo", ["Interval", "Intervalo", "Intervall", "Intervalle"]),
    ("Passate", ["Passes", "Pasadas", "Durchgänge", "Passes"]),
    ("Frequenza", ["Frequency", "Frecuencia", "Frequenz", "Fréquence"]),
    ("Velocità:", ["Speed:", "Velocidad:", "Geschwindigkeit:", "Vitesse :"]),
    ("Potenza:", ["Power:", "Potencia:", "Leistung:", "Puissance :"]),
    ("Passi:", ["Steps:", "Pasos:", "Schritte:", "Étapes :"]),
    ("Dimensione:", ["Size:", "Tamaño:", "Größe:", "Taille :"]),
    ("Intervallo min:", ["Min interval:", "Intervalo mín.:", "Min. Intervall:", "Intervalle min :"]),
    ("Intervallo massimo:", ["Max interval:", "Intervalo máx.:", "Max. Intervall:", "Intervalle max :"]),
    ("Riempimento semplice", ["Simple fill", "Relleno simple", "Einfache Füllung", "Remplissage simple"]),
    ("Immagine dithering", ["Dithered image", "Imagen con tramado", "Gerastertes Bild", "Image tramée"]),
    ("Frame", ["Frame", "Encuadrar", "Rahmen", "Cadrer"]),
    ("Avvio", ["Start", "Iniciar", "Start", "Démarrer"]),
    ("Riprendi", ["Resume", "Reanudar", "Fortsetzen", "Reprendre"]),
    ("Pausa", ["Pause", "Pausa", "Pause", "Pause"]),
    ("Stop", ["Stop", "Detener", "Stopp", "Arrêt"]),
    ("Invia", ["Send", "Enviar", "Senden", "Envoyer"]),
    ("Invia il test al laser e avvialo", ["Send the test to the laser and start it", "Enviar la prueba al láser e iniciarla", "Test an den Laser senden und starten", "Envoyer le test au laser et le lancer"]),
    ("Impostazioni predefinite", ["Presets", "Ajustes predefinidos", "Voreinstellungen", "Préréglages"]),
    ("-----Impostazioni predefinite integrate-----", ["-----Built-in presets-----", "-----Ajustes integrados-----", "-----Integrierte Voreinstellungen-----", "-----Préréglages intégrés-----"]),
    ("-------Impostazioni predefinite utente-------", ["-------User presets-------", "-------Ajustes de usuario-------", "-------Benutzer-Voreinstellungen-------", "-------Préréglages utilisateur-------"]),
    ("Salva come impostazione predefinita utente (usa il Titolo come nome)", ["Save as user preset (the Title is its name)", "Guardar como ajuste de usuario (el Título es el nombre)", "Als Benutzer-Voreinstellung speichern (Titel als Name)", "Enregistrer comme préréglage (le Titre est le nom)"]),
    ("Elimina l'impostazione predefinita utente scelta", ["Delete the chosen user preset", "Eliminar el ajuste de usuario elegido", "Gewählte Benutzer-Voreinstellung löschen", "Supprimer le préréglage choisi"]),
    ("Impostazione {}", ["Preset {}", "Ajuste {}", "Voreinstellung {}", "Préréglage {}"]),
    ("Titolo", ["Title", "Título", "Titel", "Titre"]),
    ("Verticale / Righe", ["Vertical / Rows", "Vertical / Filas", "Vertikal / Zeilen", "Vertical / Lignes"]),
    ("Orizzontale / Colonne", ["Horizontal / Columns", "Horizontal / Columnas", "Horizontal / Spalten", "Horizontal / Colonnes"]),
    ("Cont.:", ["Count:", "Cant.:", "Anzahl:", "Nombre :"]),
    ("Param.:", ["Param:", "Parám.:", "Param.:", "Param. :"]),
    ("Min:", ["Min:", "Mín:", "Min:", "Min :"]),
    ("Max:", ["Max:", "Máx:", "Max:", "Max :"]),
    ("Larghezza:", ["Width:", "Ancho:", "Breite:", "Largeur :"]),
    ("Altezza:", ["Height:", "Alto:", "Höhe:", "Hauteur :"]),
    ("Centro X:", ["Center X:", "Centro X:", "Mitte X:", "Centre X :"]),
    ("Centro Y:", ["Center Y:", "Centro Y:", "Mitte Y:", "Centre Y :"]),
    ("Per usare questa impostazione, l'origine deve essere 'Coordinate assolute'.", ["Origin mode must be 'Absolute Coords' to enable this setting.", "El origen debe ser 'Coordenadas absolutas' para usar este ajuste.", "Der Ursprung muss 'Absolute Koordinaten' sein, um dies zu nutzen.", "L'origine doit être 'Coordonnées absolues' pour ce réglage."]),
    ("Dimensione uscita: {} x {} mm", ["Output size: {} x {} mm", "Tamaño de salida: {} x {} mm", "Ausgabegröße: {} x {} mm", "Taille de sortie : {} x {} mm"]),
    ("Modifica impostazione materiale", ["Edit material setting", "Editar ajuste de material", "Materialeinstellung bearbeiten", "Modifier le réglage matériau"]),
    ("Modifica impostazione testo", ["Edit text setting", "Editar ajuste de texto", "Texteinstellung bearbeiten", "Modifier le réglage texte"]),
    ("Modifica impostazione bordo", ["Edit border setting", "Editar ajuste de borde", "Rahmeneinstellung bearbeiten", "Modifier le réglage bordure"]),
    ("Impostazione materiale (le celle variano i parametri scelti sopra)", ["Material setting (cells vary the parameters chosen above)", "Ajuste de material (las celdas varían los parámetros elegidos)", "Materialeinstellung (Zellen variieren die gewählten Parameter)", "Réglage matériau (les cellules varient les paramètres choisis)"]),
    ("Impostazione testo", ["Text setting", "Ajuste de texto", "Texteinstellung", "Réglage texte"]),
    ("Impostazione bordo", ["Border setting", "Ajuste de borde", "Rahmeneinstellung", "Réglage bordure"]),
    ("Abilita testo", ["Enable text", "Activar texto", "Text aktivieren", "Activer le texte"]),
    ("Abilita bordo", ["Enable border", "Activar borde", "Rahmen aktivieren", "Activer la bordure"]),
    ("Distanza taglio: {} mm (~{})   Spostamenti rapidi: {} mm (~{})   Tempo totale stimato: {}", ["Cut distance: {} mm (~{})   Rapid moves: {} mm (~{})   Total time estimated: {}", "Distancia de corte: {} mm (~{})   Movimientos rápidos: {} mm (~{})   Tiempo total estimado: {}", "Schnittlänge: {} mm (~{})   Eilgänge: {} mm (~{})   Geschätzte Gesamtzeit: {}", "Distance de coupe : {} mm (~{})   Déplacements rapides : {} mm (~{})   Durée totale estimée : {}"]),
    ("Velocità di riproduzione", ["Playback speed", "Velocidad de reproducción", "Wiedergabegeschwindigkeit", "Vitesse de lecture"]),
    ("Mostra movimenti trasversali", ["Show traversal moves", "Mostrar movimientos de desplazamiento", "Leerfahrten anzeigen", "Afficher les déplacements"]),
    ("Ombreggia in base alla potenza", ["Shade according to power", "Sombrear según la potencia", "Nach Leistung schattieren", "Ombrer selon la puissance"]),
    ("Inverti", ["Invert", "Invertir", "Invertieren", "Inverser"]),
    ("Mostra legenda", ["Show legend", "Mostrar leyenda", "Legende anzeigen", "Afficher la légende"]),
    ("Riproduci", ["Play", "Reproducir", "Abspielen", "Lire"]),
    ("Salva immagine", ["Save image", "Guardar imagen", "Bild speichern", "Enregistrer l'image"]),
    ("Comincia qui", ["Start here", "Empezar aquí", "Hier beginnen", "Commencer ici"]),
    ("Avvia da qui", ["Start from here", "Iniciar desde aquí", "Ab hier starten", "Démarrer d'ici"]),
    ("Salva G-code da qui", ["Save G-code from here", "Guardar G-code desde aquí", "G-Code ab hier speichern", "Enregistrer le G-code d'ici"]),
    ("Salvare il progetto?", ["Save Project?", "¿Guardar el proyecto?", "Projekt speichern?", "Enregistrer le projet ?"]),
    ("Il progetto è stato modificato.\n\nVuoi salvarlo prima di chiuderlo?", ["Your project has been modified.\n\nWould you like to save it before closing?", "El proyecto ha sido modificado.\n\n¿Quieres guardarlo antes de cerrarlo?", "Das Projekt wurde geändert.\n\nMöchtest du es vor dem Schließen speichern?", "Le projet a été modifié.\n\nVoulez-vous l'enregistrer avant de le fermer ?"]),
    ("Non salvare", ["Don't Save", "No guardar", "Nicht speichern", "Ne pas enregistrer"]),
    ("Esci da BeamForge", ["Quit BeamForge", "Salir de BeamForge", "BeamForge beenden", "Quitter BeamForge"]),
    ("Annulla (⌘Z)", ["Undo (⌘Z)", "Deshacer (⌘Z)", "Rückgängig (⌘Z)", "Annuler (⌘Z)"]),
    ("Ripeti (⇧⌘Z)", ["Redo (⇧⌘Z)", "Rehacer (⇧⌘Z)", "Wiederholen (⇧⌘Z)", "Rétablir (⇧⌘Z)"]),
    ("Taglia (⌘X)", ["Cut (⌘X)", "Cortar (⌘X)", "Ausschneiden (⌘X)", "Couper (⌘X)"]),
    ("Copia (⌘C)", ["Copy (⌘C)", "Copiar (⌘C)", "Kopieren (⌘C)", "Copier (⌘C)"]),
    ("Incolla (⌘V)", ["Paste (⌘V)", "Pegar (⌘V)", "Einfügen (⌘V)", "Coller (⌘V)"]),
    ("Duplica (⌘D)", ["Duplicate (⌘D)", "Duplicar (⌘D)", "Duplizieren (⌘D)", "Dupliquer (⌘D)"]),
    ("Elimina (⌫)", ["Delete (⌫)", "Eliminar (⌫)", "Löschen (⌫)", "Supprimer (⌫)"]),
    ("Seleziona tutto (⌘A)", ["Select All (⌘A)", "Seleccionar todo (⌘A)", "Alles auswählen (⌘A)", "Tout sélectionner (⌘A)"]),
    ("Riduci a icona", ["Minimize", "Minimizar", "Im Dock ablegen", "Placer dans le Dock"]),
    ("Schermo intero", ["Full Screen", "Pantalla completa", "Vollbild", "Plein écran"]),
    ("Impostazioni…", ["Settings…", "Ajustes…", "Einstellungen…", "Réglages…"]),
    ("Impostazioni", ["Settings", "Ajustes", "Einstellungen", "Réglages"]),
    ("Lingua", ["Language", "Idioma", "Sprache", "Langue"]),
    ("Automatica (lingua di sistema)", ["Automatic (system language)", "Automático (idioma del sistema)", "Automatisch (Systemsprache)", "Automatique (langue du système)"]),
    ("La lingua cambia subito in tutta l'app e viene ricordata.", ["The language changes right away everywhere and is remembered.", "El idioma cambia al instante en toda la app y se recuerda.", "Die Sprache ändert sich sofort überall und wird gespeichert.", "La langue change aussitôt partout et est mémorisée."]),
    ("Lingua di sistema rilevata: {}", ["Detected system language: {}", "Idioma del sistema detectado: {}", "Erkannte Systemsprache: {}", "Langue du système détectée : {}"]),
    // toolbar
    ("Nuovo progetto", ["New project", "Nuevo proyecto", "Neues Projekt", "Nouveau projet"]),
    ("Apri progetto", ["Open project", "Abrir proyecto", "Projekt öffnen", "Ouvrir un projet"]),
    ("Importa SVG o immagine", ["Import SVG or image", "Importar SVG o imagen", "SVG oder Bild importieren", "Importer un SVG ou une image"]),
    ("Esporta G-code", ["Export G-code", "Exportar G-code", "G-Code exportieren", "Exporter le G-code"]),
    ("Anteprima del lavoro (Alt+P)", ["Job preview (Alt+P)", "Vista previa del trabajo (Alt+P)", "Auftragsvorschau (Alt+P)", "Aperçu du travail (Alt+P)"]),
    ("○ Nessun laser collegato", ["○ No laser connected", "○ Ningún láser conectado", "○ Kein Laser verbunden", "○ Aucun laser connecté"]),
    // numeric bar
    ("Blocca proporzioni", ["Lock aspect ratio", "Bloquear proporciones", "Seitenverhältnis sperren", "Verrouiller les proportions"]),
    ("Larghezza", ["Width", "Ancho", "Breite", "Largeur"]),
    ("Altezza", ["Height", "Alto", "Höhe", "Hauteur"]),
    ("Ruota", ["Rotate", "Girar", "Drehen", "Rotation"]),
    ("Carattere", ["Font", "Fuente", "Schrift", "Police"]),
    ("Lati", ["Sides", "Lados", "Seiten", "Côtés"]),
    ("Lati del poligono", ["Polygon sides", "Lados del polígono", "Polygonseiten", "Côtés du polygone"]),
    // status and palette
    ("{} forme · {} selezionate", ["{} shapes · {} selected", "{} formas · {} seleccionadas", "{} Formen · {} ausgewählt", "{} formes · {} sélectionnées"]),
    ("BeamForge 1.0 · software libero, nessuna licenza", ["BeamForge 1.0 · free software, no licence", "BeamForge 1.0 · software libre, sin licencia", "BeamForge 1.0 · freie Software, keine Lizenz", "BeamForge 1.0 · logiciel libre, sans licence"]),
    ("Livello strumento (non viene inciso)", ["Tool layer (not burned)", "Capa de herramienta (no se graba)", "Werkzeugebene (wird nicht gelasert)", "Calque outil (non gravé)"]),
    ("Assegna il livello alle forme selezionate", ["Assign this layer to the selected shapes", "Asignar la capa a las formas seleccionadas", "Ebene den ausgewählten Formen zuweisen", "Attribuer le calque aux formes sélectionnées"]),
    // cuts / layers
    ("Tagli / Livelli", ["Cuts / Layers", "Cortes / Capas", "Schnitte / Ebenen", "Découpes / Calques"]),
    ("Livello", ["Layer", "Capa", "Ebene", "Calque"]),
    ("Modalità", ["Mode", "Modo", "Modus", "Mode"]),
    ("Vel./Pot. (mm/min / %)", ["Spd (mm/min) / Pwr (%)", "Vel. (mm/min) / Pot. (%)", "Geschw. (mm/min) / Leist. (%)", "Vit. (mm/min) / Puis. (%)"]),
    ("Risultato", ["Output", "Salida", "Ausgabe", "Sortie"]),
    ("Mostra", ["Show", "Mostrar", "Anzeigen", "Afficher"]),
    ("Aria", ["Air", "Aire", "Luft", "Air"]),
    ("Disegna o importa qualcosa:\ni livelli compaiono qui.", ["Draw or import something:\nlayers appear here.", "Dibuja o importa algo:\nlas capas aparecen aquí.", "Zeichne oder importiere etwas:\nEbenen erscheinen hier.", "Dessinez ou importez quelque chose :\nles calques apparaissent ici."]),
    ("Sposta su il livello selezionato", ["Move selected layer up", "Subir la capa seleccionada", "Ausgewählte Ebene nach oben", "Monter le calque sélectionné"]),
    ("Sposta giù il livello selezionato", ["Move selected layer down", "Bajar la capa seleccionada", "Ausgewählte Ebene nach unten", "Descendre le calque sélectionné"]),
    ("Elimina il livello selezionato e il suo contenuto", ["Delete selected layer and all content", "Eliminar la capa seleccionada y su contenido", "Ausgewählte Ebene samt Inhalt löschen", "Supprimer le calque sélectionné et son contenu"]),
    ("Modifica impostazioni di taglio", ["Edit cut settings", "Editar ajustes de corte", "Schnitteinstellungen bearbeiten", "Modifier les réglages de découpe"]),
    ("Colore livello", ["Layer Color", "Color de capa", "Ebenenfarbe", "Couleur du calque"]),
    ("Velocità (mm/min)", ["Speed (mm/min)", "Velocidad (mm/min)", "Geschwindigkeit (mm/min)", "Vitesse (mm/min)"]),
    ("Numero passaggi", ["Number of Passes", "Número de pasadas", "Anzahl Durchgänge", "Nombre de passes"]),
    ("Potenza massima (%)", ["Max Power (%)", "Potencia máx. (%)", "Max. Leistung (%)", "Puissance max. (%)"]),
    ("Intervallo (mm)", ["Interval (mm)", "Intervalo (mm)", "Intervall (mm)", "Intervalle (mm)"]),
    ("Potenza min. (%)", ["Min Power (%)", "Potencia mín. (%)", "Min. Leistung (%)", "Puissance min. (%)"]),
    ("Altre impostazioni…", ["More settings…", "Más ajustes…", "Weitere Einstellungen…", "Plus de réglages…"]),
    // laser tab
    ("Avvia", ["Start", "Iniciar", "Start", "Démarrer"]),
    ("Ferma", ["Stop", "Detener", "Stopp", "Arrêter"]),
    ("Salva G-code", ["Save G-code", "Guardar G-code", "G-Code speichern", "Enregistrer le G-code"]),
    ("Esegui G-code", ["Run G-code", "Ejecutar G-code", "G-Code ausführen", "Exécuter le G-code"]),
    ("Home", ["Home", "Inicio", "Referenzfahrt", "Origine machine"]),
    ("Vai all'origine", ["Go to Origin", "Ir al origen", "Zum Ursprung", "Aller à l'origine"]),
    ("Invio {} / {} righe", ["Sending {} / {} lines", "Enviando {} / {} líneas", "Sende {} / {} Zeilen", "Envoi {} / {} lignes"]),
    ("Inizia da:", ["Start From:", "Iniciar desde:", "Starten von:", "Démarrer depuis :"]),
    ("Origine lavoro", ["Job Origin", "Origen del trabajo", "Auftragsursprung", "Origine du travail"]),
    ("Taglia la grafica selezionata", ["Cut Selected Graphics", "Cortar los gráficos seleccionados", "Ausgewählte Grafiken schneiden", "Découper les graphiques sélectionnés"]),
    ("Ottimizza il percorso (interni prima, percorso più breve)", ["Optimize path (inner shapes first, shortest travel)", "Optimizar la ruta (interiores primero, recorrido más corto)", "Pfad optimieren (innere zuerst, kürzester Weg)", "Optimiser le trajet (intérieurs d'abord, trajet le plus court)"]),
    ("Periferiche", ["Devices", "Dispositivos", "Geräte", "Périphériques"]),
    ("(nessuna porta)", ["(no port)", "(sin puerto)", "(kein Anschluss)", "(aucun port)"]),
    ("Cerca di nuovo le porte", ["Rescan ports", "Volver a buscar puertos", "Anschlüsse neu suchen", "Rechercher les ports"]),
    ("Scollega", ["Disconnect", "Desconectar", "Trennen", "Déconnecter"]),
    ("Collega", ["Connect", "Conectar", "Verbinden", "Connecter"]),
    ("Dispositivo: {} ({} × {} mm) — stato: {}", ["Device: {} ({} × {} mm) — state: {}", "Dispositivo: {} ({} × {} mm) — estado: {}", "Gerät: {} ({} × {} mm) — Status: {}", "Appareil : {} ({} × {} mm) — état : {}"]),
    // move tab
    ("Distanza", ["Distance", "Distancia", "Abstand", "Distance"]),
    ("Velocità", ["Speed", "Velocidad", "Geschwindigkeit", "Vitesse"]),
    ("Leggi posizione", ["Get Position", "Leer posición", "Position abfragen", "Lire la position"]),
    ("Imposta origine", ["Set Origin", "Fijar origen", "Ursprung setzen", "Définir l'origine"]),
    ("Posizione testa: X {}  Y {}", ["Head position: X {}  Y {}", "Posición del cabezal: X {}  Y {}", "Kopfposition: X {}  Y {}", "Position de la tête : X {}  Y {}"]),
    // console
    ("Comando G-code…", ["G-code command…", "Comando G-code…", "G-Code-Befehl…", "Commande G-code…"]),
    ("$X sblocca", ["$X unlock", "$X desbloquear", "$X entsperren", "$X déverrouiller"]),
    ("$I info", ["$I info", "$I info", "$I Info", "$I infos"]),
    ("? stato", ["? status", "? estado", "? Status", "? état"]),
    // shape tab
    ("Nessuna forma selezionata.", ["No shape selected.", "Ninguna forma seleccionada.", "Keine Form ausgewählt.", "Aucune forme sélectionnée."]),
    ("Più forme selezionate.", ["Several shapes selected.", "Varias formas seleccionadas.", "Mehrere Formen ausgewählt.", "Plusieurs formes sélectionnées."]),
    ("Tipo", ["Type", "Tipo", "Typ", "Type"]),
    ("Percorso", ["Path", "Trazado", "Pfad", "Tracé"]),
    ("Raggio angoli", ["Corner radius", "Radio de esquinas", "Eckenradius", "Rayon des coins"]),
    ("Pixel", ["Pixels", "Píxeles", "Pixel", "Pixels"]),
    ("Sotto-percorsi", ["Sub-paths", "Subtrazados", "Teilpfade", "Sous-tracés"]),
    // cut settings editor
    ("Editor impostazioni di taglio — {}", ["Cut Settings Editor — {}", "Editor de ajustes de corte — {}", "Schnitteinstellungen — {}", "Éditeur de réglages de découpe — {}"]),
    ("Nome", ["Name", "Nombre", "Name", "Nom"]),
    ("Opzioni", ["Options", "Opciones", "Optionen", "Options"]),
    ("Emissione", ["Output", "Salida", "Ausgabe", "Sortie"]),
    ("= {} linee per pollice", ["= {} lines per inch", "= {} líneas por pulgada", "= {} Linien pro Zoll", "= {} lignes par pouce"]),
    ("Angolo di scansione (°)", ["Scan Angle (°)", "Ángulo de escaneo (°)", "Scanwinkel (°)", "Angle de balayage (°)"]),
    ("Scansione", ["Scanning", "Escaneo", "Scannen", "Balayage"]),
    ("Bidirezionale", ["Bi-directional", "Bidireccional", "Bidirektional", "Bidirectionnel"]),
    ("Tratteggio incrociato", ["Cross-Hatch", "Rayado cruzado", "Kreuzschraffur", "Hachures croisées"]),
    ("Overscan", ["Overscan", "Overscan", "Overscan", "Overscan"]),
    (" % della velocità", [" % of speed", " % de la velocidad", " % der Geschwindigkeit", " % de la vitesse"]),
    ("Modalità immagine", ["Image Mode", "Modo de imagen", "Bildmodus", "Mode image"]),
    ("Negativo", ["Negative", "Negativo", "Negativ", "Négatif"]),
    ("Inverti l'immagine", ["Invert the image", "Invertir la imagen", "Bild invertieren", "Inverser l'image"]),
    ("Comune", ["Common", "Común", "Allgemein", "Commun"]),
    ("Avanzate", ["Advanced", "Avanzado", "Erweitert", "Avancé"]),
    ("Offset kerf (mm)", ["Kerf offset (mm)", "Compensación de corte (mm)", "Schnittfugen-Versatz (mm)", "Décalage de saignée (mm)"]),
    ("Modalità perforazione", ["Perforation mode", "Modo perforado", "Perforationsmodus", "Mode perforation"]),
    ("Taglio", ["Cut", "Corte", "Schnitt", "Coupe"]),
    ("Salta", ["Skip", "Saltar", "Überspringen", "Saut"]),
    ("Linguette / Ponti", ["Tabs / Bridges", "Pestañas / Puentes", "Haltestege / Brücken", "Attaches / Ponts"]),
    ("Generazione", ["Generation", "Generación", "Erzeugung", "Génération"]),
    ("Automatica", ["Automatic", "Automática", "Automatisch", "Automatique"]),
    ("Manuale", ["Manual", "Manual", "Manuell", "Manuelle"]),
    ("Dimensione linguetta (mm)", ["Tab size (mm)", "Tamaño de pestaña (mm)", "Steggröße (mm)", "Taille d'attache (mm)"]),
    ("Spaziatura uniforme (mm)", ["Even spacing (mm)", "Espaciado uniforme (mm)", "Gleichmäßiger Abstand (mm)", "Espacement régulier (mm)"]),
    ("Linguette per forma", ["Tabs per shape", "Pestañas por forma", "Stege pro Form", "Attaches par forme"]),
    ("Limita linguette max", ["Limit max tabs", "Limitar pestañas máx.", "Max. Stege begrenzen", "Limiter le nombre max."]),
    ("Potenza taglio linguette (%)", ["Tab cut power (%)", "Potencia de corte en pestañas (%)", "Stegschnittleistung (%)", "Puissance sur attaches (%)"]),
    ("Cancella linguette", ["Clear tabs", "Borrar pestañas", "Stege löschen", "Effacer les attaches"]),
    ("Ignora forme interne", ["Skip inner shapes", "Omitir formas internas", "Innere Formen überspringen", "Ignorer les formes intérieures"]),
    ("Linguette", ["Tabs", "Pestañas", "Stege", "Attaches"]),
    ("Sì", ["Yes", "Sí", "Ja", "Oui"]),
    ("No", ["No", "No", "Nein", "Non"]),
    ("Linguetta aggiunta", ["Tab added", "Pestaña añadida", "Steg hinzugefügt", "Attache ajoutée"]),
    ("Linguetta rimossa", ["Tab removed", "Pestaña eliminada", "Steg entfernt", "Attache supprimée"]),
    ("Min linguette", ["Min tabs", "Pestañas mín.", "Min. Stege", "Attaches min."]),
    ("Numero linguette", ["Tab count", "Número de pestañas", "Anzahl Stege", "Nombre d'attaches"]),
    ("(spento)", ["(off)", "(apagado)", "(aus)", "(désactivé)"]),
    ("Verso l'esterno", ["Outward", "Hacia fuera", "Nach außen", "Vers l'extérieur"]),
    ("Verso l'interno", ["Inward", "Hacia dentro", "Nach innen", "Vers l'intérieur"]),
    ("Linguette posizionate: {}", ["Tabs placed: {}", "Pestañas colocadas: {}", "Gesetzte Stege: {}", "Attaches placées : {}"]),
    ("Posiziona sul disegno", ["Place on drawing", "Colocar en el dibujo", "Auf Zeichnung setzen", "Placer sur le dessin"]),
    ("Strumento Linguette: clic sul contorno per aggiungere, trascina per spostare, Ctrl/Cmd-clic o doppio clic per togliere.", ["Tabs tool: click an outline to add, drag to move, Ctrl/Cmd-click or double-click to remove.", "Herramienta Pestañas: clic en el contorno para añadir, arrastra para mover, Ctrl/Cmd-clic o doble clic para quitar.", "Steg-Werkzeug: Kontur anklicken zum Hinzufügen, ziehen zum Verschieben, Strg/Cmd-Klick oder Doppelklick zum Entfernen.", "Outil Attaches : clic sur le contour pour ajouter, glisser pour déplacer, Ctrl/Cmd-clic ou double-clic pour retirer."]),
    ("Linguette automatiche", ["Automatic tabs", "Pestañas automáticas", "Automatische Stege", "Attaches automatiques"]),
    ("Passando alle linguette automatiche verranno eliminate le linguette manuali. Continuare?", ["Switching to automatic tabs will delete the manual tabs. Continue?", "Al pasar a pestañas automáticas se eliminarán las manuales. ¿Continuar?", "Beim Wechsel zu automatischen Stegen werden die manuellen gelöscht. Fortfahren?", "Passer aux attaches automatiques supprimera les attaches manuelles. Continuer ?"]),
    ("Sovrataglio (mm)", ["Overcut (mm)", "Sobrecorte (mm)", "Überschnitt (mm)", "Surcoupe (mm)"]),
    ("Aggiungi lead-in", ["Add lead-in", "Añadir entrada", "Einlauf hinzufügen", "Ajouter une entrée"]),
    ("Aggiungi lead-out", ["Add lead-out", "Añadir salida", "Auslauf hinzufügen", "Ajouter une sortie"]),
    ("Lunghezza", ["Length", "Longitud", "Länge", "Longueur"]),
    ("Angolo", ["Angle", "Ángulo", "Winkel", "Angle"]),
    ("Stile Lead In/Out", ["Lead in/out style", "Estilo de entrada/salida", "Ein-/Auslauf-Stil", "Style d'entrée/sortie"]),
    ("Arco", ["Arc", "Arco", "Bogen", "Arc"]),
    ("Resetta a predefinito", ["Reset to default", "Restablecer predeterminado", "Auf Standard zurücksetzen", "Rétablir par défaut"]),
    ("Rendi predefinito", ["Make default", "Hacer predeterminado", "Als Standard", "Définir par défaut"]),
    ("Rendi predefinito per tutti", ["Make default for all", "Predeterminado para todas", "Standard für alle", "Par défaut pour toutes"]),
    ("Impostazioni salvate come predefinite per questo livello", ["Settings saved as this layer's default", "Ajustes guardados como predeterminados de esta capa", "Einstellungen als Standard dieser Ebene gespeichert", "Réglages enregistrés par défaut pour ce calque"]),
    ("Impostazioni salvate come predefinite per tutti i livelli", ["Settings saved as default for every layer", "Ajustes guardados como predeterminados de todas las capas", "Einstellungen als Standard aller Ebenen gespeichert", "Réglages enregistrés par défaut pour tous les calques"]),
    ("Ripristina valori predefiniti", ["Reset to defaults", "Restablecer valores predeterminados", "Auf Standard zurücksetzen", "Rétablir les valeurs par défaut"]),
    // device settings
    ("Larghezza area (mm)", ["Work area width (mm)", "Ancho del área (mm)", "Arbeitsbreite (mm)", "Largeur de la zone (mm)"]),
    ("Altezza area (mm)", ["Work area height (mm)", "Alto del área (mm)", "Arbeitshöhe (mm)", "Hauteur de la zone (mm)"]),
    ("Anteriore sinistra", ["Front left", "Delantera izquierda", "Vorne links", "Avant gauche"]),
    ("Posteriore sinistra", ["Rear left", "Trasera izquierda", "Hinten links", "Arrière gauche"]),
    ("Anteriore destra", ["Front right", "Delantera derecha", "Vorne rechts", "Avant droite"]),
    ("Posteriore destra", ["Rear right", "Trasera derecha", "Hinten rechts", "Arrière droite"]),
    ("Origine macchina", ["Machine origin", "Origen de la máquina", "Maschinenursprung", "Origine de la machine"]),
    ("Valore S massimo ($30)", ["S-value max ($30)", "Valor S máximo ($30)", "Max. S-Wert ($30)", "Valeur S max ($30)"]),
    ("Inquadra con laser acceso", ["Frame with laser on", "Encuadrar con láser encendido", "Rahmen mit Laser an", "Cadrer avec laser allumé"]),
    ("Raggio continuo all'1%", ["Continuous beam at 1%", "Haz continuo al 1%", "Dauerstrahl mit 1%", "Faisceau continu à 1 %"]),
    ("Potenza dinamica", ["Dynamic power", "Potencia dinámica", "Dynamische Leistung", "Puissance dynamique"]),
    ("M4 (spenta: M3 potenza costante)", ["M4 (off: M3 constant power)", "M4 (apagado: M3 potencia constante)", "M4 (aus: M3 konstante Leistung)", "M4 (désactivé : M3 puissance constante)"]),
    ("Velocità rapidi (mm/s)", ["Rapid speed (mm/s)", "Velocidad de rápidos (mm/s)", "Eilganggeschwindigkeit (mm/s)", "Vitesse rapide (mm/s)"]),
    ("Accelerazione (mm/s²)", ["Acceleration (mm/s²)", "Aceleración (mm/s²)", "Beschleunigung (mm/s²)", "Accélération (mm/s²)"]),
    ("Baud", ["Baud", "Baudios", "Baud", "Bauds"]),
    // about and preview
    ("Software libero per progettare e controllare laser.", ["Free software to design for and control lasers.", "Software libre para diseñar y controlar láseres.", "Freie Software zum Entwerfen und Steuern von Lasern.", "Logiciel libre pour concevoir et piloter des lasers."]),
    ("Nessuna licenza, nessuna prova a tempo, nessuna attivazione.", ["No licence, no trial period, no activation.", "Sin licencia, sin periodo de prueba, sin activación.", "Keine Lizenz, keine Testphase, keine Aktivierung.", "Aucune licence, aucune période d'essai, aucune activation."]),
    ("Controller supportati: GRBL (G-code via porta seriale).", ["Supported controllers: GRBL (G-code over serial port).", "Controladores compatibles: GRBL (G-code por puerto serie).", "Unterstützte Steuerungen: GRBL (G-Code über seriellen Anschluss).", "Contrôleurs pris en charge : GRBL (G-code via port série)."]),
    ("Software libero per laser. Nessuna licenza richiesta.", ["Free laser software. No licence required.", "Software libre para láser. No requiere licencia.", "Freie Lasersoftware. Keine Lizenz nötig.", "Logiciel laser libre. Aucune licence requise."]),
    ("Tempo stimato: {}", ["Estimated time: {}", "Tiempo estimado: {}", "Geschätzte Zeit: {}", "Durée estimée : {}"]),
    ("Taglio: {} mm", ["Cutting: {} mm", "Corte: {} mm", "Schnitt: {} mm", "Découpe : {} mm"]),
    ("{} movimenti · {} righe G-code", ["{} moves · {} G-code lines", "{} movimientos · {} líneas de G-code", "{} Bewegungen · {} G-Code-Zeilen", "{} mouvements · {} lignes de G-code"]),
    ("Avanzamento", ["Progress", "Progreso", "Fortschritt", "Progression"]),
    ("Niente da incidere: aggiungi forme su un livello con Emissione attiva.", ["Nothing to burn: add shapes on a layer with Output on.", "Nada que grabar: añade formas en una capa con Salida activa.", "Nichts zu lasern: Formen auf einer Ebene mit aktiver Ausgabe hinzufügen.", "Rien à graver : ajoutez des formes sur un calque avec Sortie activée."]),
    ("Chiudi", ["Close", "Cerrar", "Schließen", "Fermer"]),
    ("OK", ["OK", "Aceptar", "OK", "OK"]),
    // measure tool
    ("Misura", ["Measure", "Medir", "Messen", "Mesurer"]),
    ("Misurazione", ["Measurement", "Medición", "Messung", "Mesure"]),
    ("Nodi:", ["Nodes:", "Nodos:", "Knoten:", "Nœuds :"]),
    ("Linee:", ["Lines:", "Líneas:", "Linien:", "Lignes :"]),
    ("Curve:", ["Curves:", "Curvas:", "Kurven:", "Courbes :"]),
    ("Lunghezza del perimetro:", ["Perimeter length:", "Longitud del perímetro:", "Umfang:", "Longueur du périmètre :"]),
    ("Chiuso/Aperto:", ["Closed/Open:", "Cerrado/Abierto:", "Geschlossen/Offen:", "Fermé/Ouvert :"]),
    ("Chiuso", ["Closed", "Cerrado", "Geschlossen", "Fermé"]),
    ("Aperto", ["Open", "Abierto", "Offen", "Ouvert"]),
    ("Area:", ["Area:", "Área:", "Fläche:", "Surface :"]),
    ("Lunghezza del segmento:", ["Segment length:", "Longitud del segmento:", "Segmentlänge:", "Longueur du segment :"]),
    ("Punto iniziale:", ["Start point:", "Punto inicial:", "Startpunkt:", "Point de départ :"]),
    ("Punto finale:", ["End point:", "Punto final:", "Endpunkt:", "Point d'arrivée :"]),
    ("Differenza:", ["Difference:", "Diferencia:", "Differenz:", "Différence :"]),
    ("Angolo:", ["Angle:", "Ángulo:", "Winkel:", "Angle :"]),
    ("Raggio dell'arco:", ["Arc radius:", "Radio del arco:", "Bogenradius:", "Rayon de l'arc :"]),
    ("Centro dell'arco:", ["Arc center:", "Centro del arco:", "Bogenmittelpunkt:", "Centre de l'arc :"]),
    ("Cambia unità (mm / pollici)", ["Switch units (mm / inches)", "Cambiar unidades (mm / pulgadas)", "Einheit wechseln (mm / Zoll)", "Changer d'unité (mm / pouces)"]),
    ("Riempimento", ["Fill", "Relleno", "Füllen", "Remplissage"]),
    ("Riempimento offset", ["Offset Fill", "Relleno offset", "Versatzfüllung", "Remplissage décalé"]),
    ("Nome sottolivello", ["Sub-layer name", "Nombre de subcapa", "Unterebenenname", "Nom du sous-calque"]),
    ("Modalità potenza costante", ["Constant power mode", "Modo de potencia constante", "Konstante Leistung", "Mode puissance constante"]),
    ("Il laser resta a potenza costante (M3) anche nelle curve", ["The laser keeps constant power (M3), also in corners", "El láser mantiene potencia constante (M3), también en curvas", "Der Laser hält konstante Leistung (M3), auch in Kurven", "Le laser garde une puissance constante (M3), même dans les courbes"]),
    ("Riempimento bidirezionale", ["Bi-directional fill", "Relleno bidireccional", "Bidirektionale Füllung", "Remplissage bidirectionnel"]),
    ("Sovrascansione", ["Overscanning", "Sobreescaneo", "Überscannen", "Surbalayage"]),
    ("Intervallo linea (mm)", ["Line interval (mm)", "Intervalo de línea (mm)", "Zeilenabstand (mm)", "Intervalle de ligne (mm)"]),
    ("Linee per pollice", ["Lines per inch", "Líneas por pulgada", "Linien pro Zoll", "Lignes par pouce"]),
    ("Angolo di scansione (gradi)", ["Scan angle (degrees)", "Ángulo de escaneo (grados)", "Scanwinkel (Grad)", "Angle de balayage (degrés)"]),
    ("Numero di passaggi", ["Number of passes", "Número de pasadas", "Anzahl Durchgänge", "Nombre de passes"]),
    ("Offset Z (mm)", ["Z offset (mm)", "Desfase Z (mm)", "Z-Versatz (mm)", "Décalage Z (mm)"]),
    ("Fase Z per passaggio (mm)", ["Z step per pass (mm)", "Paso Z por pasada (mm)", "Z-Schritt pro Durchgang (mm)", "Pas Z par passe (mm)"]),
    ("(nessuno)", ["(none)", "(ninguno)", "(keine)", "(aucun)"]),
    ("Per cambiare il raggruppamento disattiva Riempimento completo nella scheda Avanzate", ["To change fill grouping, disable Flood Fill in the Advanced tab", "Para cambiar la agrupación, desactiva Relleno completo en la pestaña Avanzado", "Um die Gruppierung zu ändern, Flutfüllung im Reiter Erweitert ausschalten", "Pour changer le regroupement, désactivez Remplissage complet dans l'onglet Avancé"]),
    ("Riempi tutte le forme contemporaneamente", ["Fill all shapes at once", "Rellenar todas las formas a la vez", "Alle Formen gleichzeitig füllen", "Remplir toutes les formes à la fois"]),
    ("Riempi i gruppi insieme", ["Fill groups together", "Rellenar los grupos juntos", "Gruppen zusammen füllen", "Remplir les groupes ensemble"]),
    ("Riempi le forme singolarmente", ["Fill shapes individually", "Rellenar las formas una a una", "Formen einzeln füllen", "Remplir les formes individuellement"]),
    ("Lunghezza rampa", ["Ramp length", "Longitud de rampa", "Rampenlänge", "Longueur de rampe"]),
    ("La potenza sale dal minimo al massimo entro questa distanza dal bordo (mm)", ["Power rises from min to max within this distance from the edge (mm)", "La potencia sube de mín. a máx. dentro de esta distancia del borde (mm)", "Die Leistung steigt innerhalb dieses Randabstands von Min. auf Max. (mm)", "La puissance monte du min au max sur cette distance depuis le bord (mm)"]),
    ("Bordo esterno rampa", ["Ramp outer edge", "Rampa en borde exterior", "Rampe am Außenrand", "Rampe sur le bord extérieur"]),
    ("Rampa solo sui bordi esterni, non sui fori", ["Ramp only on outer edges, not on holes", "Rampa solo en bordes exteriores, no en agujeros", "Rampe nur an Außenrändern, nicht an Löchern", "Rampe uniquement sur les bords extérieurs, pas sur les trous"]),
    ("Riempimento completo", ["Flood fill", "Relleno completo", "Flutfüllung", "Remplissage complet"]),
    ("Riempie zona per zona, senza attraversare gli spazi vuoti", ["Fills area by area without crossing empty gaps", "Rellena zona por zona sin cruzar huecos", "Füllt Bereich für Bereich, ohne Lücken zu überqueren", "Remplit zone par zone sans traverser les vides"]),
    ("Nessuna opzione avanzata per il riempimento offset.", ["No advanced options for offset fill.", "No hay opciones avanzadas para el relleno offset.", "Keine erweiterten Optionen für die Versatzfüllung.", "Aucune option avancée pour le remplissage décalé."]),
    ("Potenza minima (%)", ["Min power (%)", "Potencia mínima (%)", "Minimale Leistung (%)", "Puissance minimale (%)"]),
    ("Rapporto 1:1", ["1:1 ratio", "Proporción 1:1", "Verhältnis 1:1", "Rapport 1:1"]),
    ("Option: rapporto 1:1 · Cmd: dal centro", ["Option: 1:1 ratio · Cmd: from centre", "Opción: proporción 1:1 · Cmd: desde el centro", "Wahltaste: 1:1 · Cmd: von der Mitte", "Option : rapport 1:1 · Cmd : depuis le centre"]),
    ("Passa sopra una forma o trascina per misurare.", ["Hover a shape, or drag to measure.", "Pasa sobre una forma o arrastra para medir.", "Über eine Form fahren oder ziehen, um zu messen.", "Survolez une forme ou faites glisser pour mesurer."]),
    ("{}x, {}y", ["{}x, {}y", "{}x, {}y", "{}x, {}y", "{}x, {}y"]),
    // status messages
    ("Pronto", ["Ready", "Listo", "Bereit", "Prêt"]),
    ("Annullato", ["Undone", "Deshecho", "Rückgängig gemacht", "Annulé"]),
    ("Ripetuto", ["Redone", "Rehecho", "Wiederholt", "Rétabli"]),
    ("Scollegato", ["Disconnected", "Desconectado", "Getrennt", "Déconnecté"]),
    ("Collegato", ["Connected", "Conectado", "Verbunden", "Connecté"]),
    ("BeamForge 1.0 — software laser libero. Nessuna licenza richiesta.", ["BeamForge 1.0 — free laser software. No licence required.", "BeamForge 1.0 — software láser libre. No requiere licencia.", "BeamForge 1.0 — freie Lasersoftware. Keine Lizenz nötig.", "BeamForge 1.0 — logiciel laser libre. Aucune licence requise."]),
    ("BeamForge project", ["BeamForge project", "Proyecto BeamForge", "BeamForge-Projekt", "Projet BeamForge"]),
    ("Supported files", ["Supported files", "Archivos compatibles", "Unterstützte Dateien", "Fichiers pris en charge"]),
    ("Images", ["Images", "Imágenes", "Bilder", "Images"]),
    ("Tutti supportati", ["All supported", "Todos los compatibles", "Alle unterstützten", "Tous pris en charge"]),
    ("File di immagine", ["Image files", "Archivos de imagen", "Bilddateien", "Fichiers image"]),
    ("File vettoriali", ["Vector files", "Archivos vectoriales", "Vektordateien", "Fichiers vectoriels"]),
    ("File di oggetti 3D", ["3D object files", "Archivos de objetos 3D", "3D-Objektdateien", "Fichiers d'objets 3D"]),
    ("File PCB", ["PCB files", "Archivos PCB", "PCB-Dateien", "Fichiers PCB"]),
    ("Il file non contiene forme utilizzabili.", ["The file contains no usable shapes.", "El archivo no contiene formas utilizables.", "Die Datei enthält keine verwendbaren Formen.", "Le fichier ne contient aucune forme utilisable."]),
    ("Formato di file non supportato: .{}", ["Unsupported file format: .{}", "Formato de archivo no compatible: .{}", "Nicht unterstütztes Dateiformat: .{}", "Format de fichier non pris en charge : .{}"]),
    ("Impossibile leggere il file: {}", ["Could not read the file: {}", "No se pudo leer el archivo: {}", "Datei konnte nicht gelesen werden: {}", "Impossible de lire le fichier : {}"]),
    ("Nuovo progetto creato", ["New project", "Nuevo proyecto", "Neues Projekt", "Nouveau projet"]),
    ("Aperto {}", ["Opened {}", "Abierto {}", "Geöffnet: {}", "Ouvert : {}"]),
    ("Impossibile aprire il progetto: {}", ["Could not open the project: {}", "No se pudo abrir el proyecto: {}", "Projekt konnte nicht geöffnet werden: {}", "Impossible d'ouvrir le projet : {}"]),
    ("File di progetto salvato come {}.", ["Project file saved as {}.", "Archivo de proyecto guardado como {}.", "Projektdatei gespeichert als {}.", "Fichier de projet enregistré sous {}."]),
    ("Salvataggio non riuscito: {}", ["Save failed: {}", "Error al guardar: {}", "Speichern fehlgeschlagen: {}", "Échec de l'enregistrement : {}"]),
    ("Salvato {}", ["Saved {}", "Guardado {}", "Gespeichert: {}", "Enregistré : {}"]),
    ("Impossibile salvare: {}", ["Could not save: {}", "No se pudo guardar: {}", "Speichern fehlgeschlagen: {}", "Impossible d'enregistrer : {}"]),
    ("Importato {}", ["Imported {}", "Importado {}", "Importiert: {}", "Importé : {}"]),
    ("Esportato {}", ["Exported {}", "Exportado {}", "Exportiert: {}", "Exporté : {}"]),
    ("Impossibile esportare: {}", ["Could not export: {}", "No se pudo exportar: {}", "Export fehlgeschlagen: {}", "Impossible d'exporter : {}"]),
    ("Niente da emettere: aggiungi forme su un livello con Emissione attiva.", ["Nothing to output: add shapes on a layer with Output on.", "Nada que enviar: añade formas en una capa con Salida activa.", "Nichts auszugeben: Formen auf einer Ebene mit aktiver Ausgabe hinzufügen.", "Rien à produire : ajoutez des formes sur un calque avec Sortie activée."]),
    ("G-code salvato in {}", ["G-code saved to {}", "G-code guardado en {}", "G-Code gespeichert in {}", "G-code enregistré dans {}"]),
    ("Impossibile salvare il G-code: {}", ["Could not save the G-code: {}", "No se pudo guardar el G-code: {}", "G-Code konnte nicht gespeichert werden: {}", "Impossible d'enregistrer le G-code : {}"]),
    ("Collega prima un laser (scheda Laser) oppure usa Salva G-code.", ["Connect a laser first (Laser tab), or use Save G-code.", "Conecta primero un láser (pestaña Láser) o usa Guardar G-code.", "Zuerst einen Laser verbinden (Reiter Laser) oder G-Code speichern.", "Connectez d'abord un laser (onglet Laser) ou utilisez Enregistrer le G-code."]),
    ("Lavoro avviato", ["Job started", "Trabajo iniciado", "Auftrag gestartet", "Travail démarré"]),
    ("Collega prima un laser.", ["Connect a laser first.", "Conecta primero un láser.", "Zuerst einen Laser verbinden.", "Connectez d'abord un laser."]),
    ("In esecuzione {}", ["Running {}", "Ejecutando {}", "Läuft: {}", "En cours : {}"]),
    ("Niente da inquadrare.", ["Nothing to frame.", "Nada que encuadrar.", "Nichts zum Umfahren.", "Rien à cadrer."]),
    ("Nessun laser collegato.", ["No laser connected.", "Ningún láser conectado.", "Kein Laser verbunden.", "Aucun laser connecté."]),
    ("Collegato a {}", ["Connected to {}", "Conectado a {}", "Verbunden mit {}", "Connecté à {}"]),
    ("Lavoro terminato", ["Job finished", "Trabajo terminado", "Auftrag beendet", "Travail terminé"]),
    ("Duplicato (le copie sono sopra gli originali)", ["Duplicated (copies sit on top of the originals)", "Duplicado (las copias están sobre los originales)", "Dupliziert (Kopien liegen über den Originalen)", "Dupliqué (les copies sont sur les originaux)"]),
    ("Seleziona almeno 2 oggetti da raggruppare.", ["Select at least 2 objects to group.", "Selecciona al menos 2 objetos para agrupar.", "Mindestens 2 Objekte zum Gruppieren auswählen.", "Sélectionnez au moins 2 objets à grouper."]),
    ("Raggruppati {} oggetti", ["Grouped {} objects", "{} objetos agrupados", "{} Objekte gruppiert", "{} objets groupés"]),
    ("Nessun gruppo selezionato.", ["No group selected.", "Ningún grupo seleccionado.", "Keine Gruppe ausgewählt.", "Aucun groupe sélectionné."]),
    ("Gruppo separato", ["Group split", "Grupo separado", "Gruppierung aufgehoben", "Groupe dissocié"]),
    ("Seleziona almeno 2 oggetti: si allineano all'ultimo selezionato.", ["Select at least 2 objects: they align to the last one selected.", "Selecciona al menos 2 objetos: se alinean al último seleccionado.", "Mindestens 2 Objekte wählen: sie richten sich am zuletzt gewählten aus.", "Sélectionnez au moins 2 objets : ils s'alignent sur le dernier sélectionné."]),
    ("Seleziona almeno 3 oggetti da distribuire.", ["Select at least 3 objects to distribute.", "Selecciona al menos 3 objetos para distribuir.", "Mindestens 3 Objekte zum Verteilen auswählen.", "Sélectionnez au moins 3 objets à répartir."]),
    ("Seleziona almeno 2 oggetti.", ["Select at least 2 objects.", "Selecciona al menos 2 objetos.", "Mindestens 2 Objekte auswählen.", "Sélectionnez au moins 2 objets."]),
    ("Le nuove forme vanno sul livello {}", ["New shapes go on layer {}", "Las formas nuevas van a la capa {}", "Neue Formen kommen auf Ebene {}", "Les nouvelles formes vont sur le calque {}"]),
    ("<senza titolo>", ["<untitled>", "<sin título>", "<unbenannt>", "<sans titre>"]),
    ("Sposto il laser a {}, {}", ["Moving the laser to {}, {}", "Moviendo el láser a {}, {}", "Laser fährt zu {}, {}", "Déplacement du laser vers {}, {}"]),
    ("Impossibile aprire {}: {}", ["Could not open {}: {}", "No se pudo abrir {}: {}", "{} konnte nicht geöffnet werden: {}", "Impossible d'ouvrir {} : {}"]),
    ("Collegamento perso: {}", ["Connection lost: {}", "Conexión perdida: {}", "Verbindung verloren: {}", "Connexion perdue : {}"]),
    ("Impossibile leggere l'SVG: {}", ["Could not read the SVG: {}", "No se pudo leer el SVG: {}", "SVG konnte nicht gelesen werden: {}", "Impossible de lire le SVG : {}"]),
    ("Il file SVG non contiene forme utilizzabili.", ["The SVG file has no usable shapes.", "El archivo SVG no contiene formas utilizables.", "Die SVG-Datei enthält keine verwendbaren Formen.", "Le fichier SVG ne contient aucune forme utilisable."]),
    ("Impossibile aprire l'immagine: {}", ["Could not open the picture: {}", "No se pudo abrir la imagen: {}", "Bild konnte nicht geöffnet werden: {}", "Impossible d'ouvrir l'image : {}"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_unique_and_placeholders_match() {
        let mut seen = std::collections::HashSet::new();
        for (k, v) in ROWS {
            assert!(seen.insert(*k), "duplicate key {k}");
            let n = k.matches("{}").count();
            for t in v {
                assert_eq!(t.matches("{}").count(), n, "placeholder mismatch in {k} -> {t}");
            }
        }
    }

    #[test]
    fn lookup_and_fill() {
        set(Lang::De);
        assert_eq!(tr("Salva"), "Speichern");
        assert_eq!(trf("Raggruppati {} oggetti", &[&3]), "3 Objekte gruppiert");
        set(Lang::It);
        assert_eq!(tr("Salva"), "Salva");
        assert_eq!(Lang::from_code("fr-CA"), Some(Lang::Fr));
    }
}
