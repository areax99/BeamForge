//! Periferiche: the person's own list of lasers, "Trova il mio laser" (automatic search on the
//! USB serial ports) and "Crea manualmente".
//!
//! The search follows DevicesDialog::on_pbAuto_clicked (0x10094003c) → DeviceDiscoveryWizard::
//! ScanForDevices (0x1008392dc) → ConnectGCodeDevice (0x10083b59c) → ParseGRBLSettings
//! (0x10083ce78) of the original; the numbers are in MECHANICS.md ("Finding the laser").

use crate::app::RayEtch;
use crate::i18n::{tr, trf};
use crate::model::{Device, OriginCorner};
use crate::theme::{self, ACCENT};
use eframe::egui::{self, Color32, Stroke, vec2};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// Port open timeout, ms (`mov w1,#0x5dc` before Connection::Open, 0x10083b8e4).
pub const OPEN_TIMEOUT_MS: u64 = 1500;
/// Pause between attempts and after closing a port, ms (`QThread::msleep(0x32)`, 0x10083b928).
pub const RETRY_PAUSE_MS: u64 = 50;
/// How long to wait for the controller's greeting, ms (`cmp x0,#0x7cf; b.gt`, 0x10083bc40).
pub const GREETING_WAIT_MS: u64 = 2000;
/// Speeds tried, in order (230400 for Gerbil/XTool boards, then 115200; 0x10083b69c, 0x10083b954).
pub const BAUD_FAST: u32 = 230400;
pub const BAUD_STD: u32 = 115200;

/// USB chips the original recognises (VID, PID, what it says), ScanForDevices 0x1008398e8..0x100839ccc.
pub const USB_TABLE: &[(u16, u16, &str)] = &[
    (0x1d50, 0x6015, "Smoothieware / Emblaser 2"),
    (0x0483, 0x5740, "STM32: probabilmente Gerbil-STM"),
    (0x1fc9, 0x81d9, "Emblaser 2 / Core"),
    (0x1a86, 0x7523, "CH340: probabilmente GRBL"),
    (0xffff, 0x0005, "Atmega: probabilmente GRBL, forse LPC"),
    (0x2341, 0x0043, "Arduino Uno: probabilmente GRBL"),
    (0x0403, 0x6001, "FTDI: probabile Ruida / TopWisdom (forse GRBL)"),
];

/// Greetings that mark a G-code laser controller (ConnectGCodeDevice 0x10083bc20..0x10083bdf0).
pub const GREETINGS: &[&str] = &["Grbl ", "GrblHAL ", "iLaser"];

/// Machine names the original recognises in the `$i` answer ([MODEL:] / [MSG:Using machine:]),
/// with the short name it gives the device (ParseGRBLSettings 0x10083da6c..0x10083dbec).
/// Longer names first, so "Laser Master 2 Pro" isn't taken for "Laser Master 2".
pub const MODELS: &[(&str, &str)] = &[
    ("MKS DLC32", "MKS DLC32"),
    ("Ortur Laser Master 3", "Ortur LM3"),
    ("Ortur Laser Master 2 Pro", "Ortur LM2 Pro"),
    ("Ortur Laser Master 2", "Ortur LM2"),
    ("Ortur Aufero", "Ortur Aufero"),
    ("Ortur", "Ortur"),
];

/// GRBL 1.1 from this letter on has M4 dynamic power; older ones are "GRBL-M3 (1.1e or earlier)"
/// ([VER: digits compared with '1' 0x31, letter with 'f' 0x66, ParseGRBLSettings 0x10083d9d0).
pub const M4_FIRST_LETTER: char = 'f';

/// Wait for the `$i` / `$$` answers, ms (`cmp x0,#0x7cf`, 0x10083e170).
pub const SETTINGS_WAIT_MS: u64 = 2000;
/// Pause before each query, ms (`mov w2,#0xc8`, 0x10083e15c).
pub const SETTINGS_PAUSE_MS: u64 = 200;

/// Controller types offered by "Crea manualmente" (RayEtch speaks GRBL G-code).
pub const CONTROLLERS: &[&str] = &["GRBL", "GRBL-M3", "grblHAL"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct DeviceList {
    pub devices: Vec<Device>,
    pub default: Option<usize>,
}

impl DeviceList {
    pub fn load() -> Self {
        crate::i18n::load_json("devices.json").unwrap_or_default()
    }
    pub fn save(&self) {
        crate::i18n::save_json("devices.json", self);
    }
}

/// A controller that answered on a port.
#[derive(Clone, Debug)]
pub struct Found {
    pub port: String,
    pub usb: String,
    pub greeting: String,
    pub baud: u32,
    /// $130 / $131 (max travel, mm)
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// $30 (spindle max)
    pub s_max: Option<f64>,
    /// $32 (laser mode)
    pub laser_mode: Option<bool>,
    /// $110 / $111 (max rate X / Y, mm/min)
    pub max_rate: [Option<f64>; 2],
    /// [VER:...] from `$i`, e.g. "1.1h.20190825"
    pub version: Option<String>,
    /// Short machine name from [MODEL:] or [MSG:Using machine:]
    pub model: Option<String>,
}

impl Found {
    pub fn controller(&self) -> &'static str {
        if self.greeting.starts_with("GrblHAL") {
            return "grblHAL";
        }
        // the version from $i, or else the one in the greeting ("Grbl 1.1h ['$' for help]")
        let ver = self.version.clone().unwrap_or_else(|| self.greeting.split_whitespace().nth(1).unwrap_or("").to_string());
        if is_old_grbl(&ver) { "GRBL-M3" } else { "GRBL" }
    }
    pub fn to_device(&self) -> Device {
        let mut d = Device::default();
        d.controller = self.controller().into();
        d.dynamic_power = d.controller != "GRBL-M3";
        d.name = match &self.model {
            Some(m) => m.clone(),
            None => format!("{} ({})", self.controller(), short_port(&self.port)),
        };
        d.port = self.port.clone();
        d.baud = self.baud;
        if let Some(w) = self.width.filter(|w| *w > 0.0) {
            d.width = w;
        }
        if let Some(h) = self.height.filter(|h| *h > 0.0) {
            d.height = h;
        }
        if let Some(s) = self.s_max.filter(|s| *s > 0.0) {
            d.s_max = s;
        }
        // rapid moves: the slower axis' max rate, mm/min → mm/s
        if let (Some(x), Some(y)) = (self.max_rate[0], self.max_rate[1]) {
            if x > 0.0 && y > 0.0 {
                d.rapid_speed = x.min(y) / 60.0;
            }
        }
        d
    }
}

/// "1.1e" or earlier (0.9, 1.0, 1.1a–e): no M4 dynamic power.
pub fn is_old_grbl(ver: &str) -> bool {
    let mut parts = ver.split('.');
    let (Some(major), Some(minor)) = (parts.next(), parts.next()) else { return false };
    let Ok(major) = major.trim().parse::<u32>() else { return false };
    let digits: String = minor.chars().take_while(|c| c.is_ascii_digit()).collect();
    let Ok(minor_n) = digits.parse::<u32>() else { return false };
    let letter = minor.chars().find(|c| c.is_ascii_alphabetic()).map(|c| c.to_ascii_lowercase());
    match (major, minor_n) {
        (0, _) | (1, 0) => true,
        (1, 1) => letter.is_some_and(|l| l < M4_FIRST_LETTER),
        _ => false,
    }
}

fn model_name(text: &str) -> Option<String> {
    MODELS.iter().find(|m| text.contains(m.0)).map(|m| m.1.to_string())
}

pub enum ScanMsg {
    Step(String, f32),
    Found(Found),
    Done(usize),
}

pub fn short_port(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

fn usb_label(vid: u16, pid: u16) -> Option<&'static str> {
    USB_TABLE.iter().find(|e| e.0 == vid && e.1 == pid).map(|e| e.2)
}

/// Start the search in the background; messages arrive on the returned channel.
pub fn start_scan(ctx: egui::Context) -> Receiver<ScanMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || scan(tx, ctx));
    rx
}

fn scan(tx: Sender<ScanMsg>, ctx: egui::Context) {
    let send = |m: ScanMsg| {
        let _ = tx.send(m);
        ctx.request_repaint();
    };
    let ports = serialport::available_ports().unwrap_or_default();
    // macOS lists every USB port twice (tty.* and cu.*): keep the call-out one.
    let names: Vec<String> = ports.iter().map(|p| p.port_name.clone()).collect();
    let mut todo = Vec::new();
    for p in &ports {
        let serialport::SerialPortType::UsbPort(info) = &p.port_type else { continue };
        if let Some(rest) = p.port_name.strip_prefix("/dev/tty.") {
            if names.iter().any(|n| n == &format!("/dev/cu.{rest}")) {
                continue;
            }
        }
        let label = usb_label(info.vid, info.pid).map(|s| tr(s).to_string()).unwrap_or_else(|| {
            let mut s = format!("USB {:04x}:{:04x}", info.vid, info.pid);
            if let Some(m) = info.product.as_ref().or(info.manufacturer.as_ref()) {
                s.push_str(&format!(" — {m}"));
            }
            s
        });
        let fast_first = info.vid == 0x0483;
        todo.push((p.port_name.clone(), label, fast_first));
    }
    let n = todo.len().max(1) as f32;
    let mut found = 0;
    for (i, (port, label, fast_first)) in todo.into_iter().enumerate() {
        let bauds = if fast_first { [BAUD_FAST, BAUD_STD] } else { [BAUD_STD, BAUD_FAST] };
        for (j, baud) in bauds.into_iter().enumerate() {
            send(ScanMsg::Step(trf("Provo {} a {} baud…", &[&short_port(&port), &baud]), (i as f32 + j as f32 * 0.5) / n));
            if let Some(mut f) = probe(&port, baud) {
                f.usb = label.clone();
                found += 1;
                send(ScanMsg::Found(f));
                break;
            }
            std::thread::sleep(Duration::from_millis(RETRY_PAUSE_MS));
        }
    }
    send(ScanMsg::Done(found));
}

/// Open one port, wake the controller and read its greeting and settings.
fn probe(port: &str, baud: u32) -> Option<Found> {
    let mut p = serialport::new(port, baud).timeout(Duration::from_millis(50)).open().ok()?;
    let _ = p.write_data_terminal_ready(true);
    std::thread::sleep(Duration::from_millis(RETRY_PAUSE_MS));
    // Opening resets most Arduino-style boards, which then greet; others are asked with a
    // soft reset (Ctrl-X) after an empty line.
    let _ = p.write_all(b"\r\n");
    let _ = p.write_all(&[crate::gcode::ABORT_BYTE]);
    let start = Instant::now();
    let mut buf = String::new();
    let mut greeting = None;
    while start.elapsed() < Duration::from_millis(GREETING_WAIT_MS) && greeting.is_none() {
        for line in read_lines(&mut *p, &mut buf) {
            if GREETINGS.iter().any(|g| line.starts_with(g)) || line.ends_with("ok:512") {
                greeting = Some(line);
                break;
            }
        }
    }
    let greeting = greeting?;
    let mut f = Found { port: port.into(), usb: String::new(), greeting, baud, width: None, height: None, s_max: None, laser_mode: None, max_rate: [None, None], version: None, model: None };
    // $i: build info ([VER:], [MODEL:], [MSG:Using machine:]); "$i\r\n" at 0x10083dca0
    std::thread::sleep(Duration::from_millis(SETTINGS_PAUSE_MS));
    buf.clear();
    let _ = p.write_all(b"$i\r\n");
    for line in read_until_ok(&mut *p, &mut buf) {
        if let Some(v) = line.strip_prefix("[VER:") {
            f.version = Some(v.trim_end_matches(']').split(':').next().unwrap_or("").to_string());
        }
        if f.model.is_none() && (line.starts_with("[MODEL:") || line.starts_with("[MSG:Using machine:") || line.starts_with("[BUILD:")) {
            f.model = model_name(&line);
        }
    }
    // $$: every "$n=value" line ('$' 0x24, split at '=' 0x3d, 0x10083e270)
    std::thread::sleep(Duration::from_millis(SETTINGS_PAUSE_MS));
    buf.clear();
    let _ = p.write_all(b"$$\n");
    for line in read_until_ok(&mut *p, &mut buf) {
        if let Some((k, v)) = line.strip_prefix('$').and_then(|l| l.split_once('=')) {
            let v: f64 = v.split_whitespace().next().unwrap_or("").parse().unwrap_or(f64::NAN);
            if v.is_nan() {
                continue;
            }
            match k {
                "30" => f.s_max = Some(v),
                "32" => f.laser_mode = Some(v != 0.0),
                "110" => f.max_rate[0] = Some(v),
                "111" => f.max_rate[1] = Some(v),
                "130" => f.width = Some(v),
                "131" => f.height = Some(v),
                _ => {}
            }
        }
    }
    Some(f)
}

/// Lines up to "ok" / "error", for at most SETTINGS_WAIT_MS.
fn read_until_ok(p: &mut dyn serialport::SerialPort, buf: &mut String) -> Vec<String> {
    let start = Instant::now();
    let mut out = Vec::new();
    while start.elapsed() < Duration::from_millis(SETTINGS_WAIT_MS) {
        for line in read_lines(p, buf) {
            if line == "ok" || line.starts_with("error") {
                return out;
            }
            out.push(line);
        }
    }
    out
}

fn read_lines(p: &mut dyn serialport::SerialPort, buf: &mut String) -> Vec<String> {
    let mut chunk = [0u8; 256];
    if let Ok(n) = p.read(&mut chunk) {
        buf.push_str(&String::from_utf8_lossy(&chunk[..n]));
    }
    let mut out = Vec::new();
    while let Some(i) = buf.find('\n') {
        let l = buf[..i].trim().to_string();
        buf.drain(..=i);
        if !l.is_empty() {
            out.push(l);
        }
    }
    out
}

/// State of the Periferiche window while it is open.
pub struct DevicesUi {
    pub list: DeviceList,
    pub selected: Option<usize>,
    pub finder: Option<Finder>,
    /// Device being created or changed: (index to replace, draft).
    pub editor: Option<(Option<usize>, Device)>,
}

pub struct Finder {
    pub rx: Option<Receiver<ScanMsg>>,
    pub step: String,
    pub progress: f32,
    pub results: Vec<Found>,
    pub picked: Option<usize>,
    pub done: bool,
}

impl Finder {
    fn start(ctx: &egui::Context) -> Self {
        Finder { rx: Some(start_scan(ctx.clone())), step: tr("Cerco i dispositivi collegati…").into(), progress: 0.0, results: Vec::new(), picked: None, done: false }
    }
    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        for m in rx.try_iter() {
            match m {
                ScanMsg::Step(s, p) => {
                    self.step = s;
                    self.progress = p;
                }
                ScanMsg::Found(f) => {
                    if self.picked.is_none() {
                        self.picked = Some(self.results.len());
                    }
                    self.results.push(f);
                }
                ScanMsg::Done(n) => {
                    self.done = true;
                    self.progress = 1.0;
                    self.step = if n == 0 { tr("Nessun laser trovato.").into() } else { trf("Trovati {} dispositivi.", &[&n]) };
                }
            }
        }
        if self.done {
            self.rx = None;
        }
    }
}

impl RayEtch {
    pub fn open_devices(&mut self) {
        let list = self.device_list.clone();
        let selected = self.device_sel.filter(|i| *i < list.devices.len()).or(list.default);
        self.devices_ui = Some(DevicesUi { list, selected, finder: None, editor: None });
    }

    /// Use a saved device: its settings go to the document, its port to the connection box.
    pub fn use_device(&mut self, i: usize) {
        let Some(d) = self.device_list.devices.get(i).cloned() else { return };
        if d.width != self.doc.device.width || d.height != self.doc.device.height {
            self.view.fit_pending = true;
        }
        if !d.port.is_empty() {
            if !self.ports.contains(&d.port) {
                self.ports = crate::laser::list_ports();
            }
            self.port_sel = d.port.clone();
        }
        self.doc.device = d;
        self.device_sel = Some(i);
    }
}

/// Small laser-head icon drawn in a dark rounded square (own artwork).
pub fn device_icon(ui: &mut egui::Ui, size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 4.0, Color32::from_rgb(12, 12, 14));
    let c = r.center();
    let s = size / 24.0;
    let head = egui::Rect::from_center_size(c + vec2(0.0, -3.0 * s), vec2(10.0 * s, 8.0 * s));
    p.rect_filled(head, 1.5 * s, Color32::from_rgb(235, 235, 235));
    p.line_segment([c + vec2(0.0, 1.0 * s), c + vec2(0.0, 7.0 * s)], Stroke::new(1.6 * s, ACCENT));
    p.circle_filled(c + vec2(0.0, 8.0 * s), 1.6 * s, Color32::from_rgb(255, 200, 120));
}

fn subtitle(d: &Device) -> String {
    let port = if d.port.is_empty() { tr("porta automatica").to_string() } else { short_port(&d.port).to_string() };
    format!("{} | {} | {}", d.controller, port, format!("{:.0} × {:.0} mm", d.width, d.height))
}

/// Draw the Periferiche window and its helpers (finder and editor).
pub fn window(app: &mut RayEtch, ctx: &egui::Context) {
    let Some(mut st) = app.devices_ui.take() else { return };
    let mut keep = true;
    let mut accept = false;

    if st.finder.is_none() && st.editor.is_none() {
        let mut open = true;
        theme::centered(ctx, "devices", egui::Window::new(tr("Periferiche")).open(&mut open).resizable(false).collapsible(false).fixed_size(vec2(600.0, 520.0))).show(ctx, |ui| {
            ui.label(tr("Elenco periferiche personali"));
            egui::Frame::new().fill(crate::theme::p().field).stroke(Stroke::new(1.0, crate::theme::p().line)).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_min_size(vec2(590.0, 330.0));
                egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                    if st.list.devices.is_empty() {
                        ui.horizontal(|ui| {
                            device_icon(ui, 36.0);
                            ui.vertical(|ui| {
                                ui.label(tr("Nessuna macchina"));
                                ui.label(egui::RichText::new(tr("Usa «Trova il mio laser» o «Crea manualmente»")).color(crate::theme::p().text_dim));
                            });
                        });
                    }
                    let mut dbl = None;
                    for (i, d) in st.list.devices.iter().enumerate() {
                        let sel = st.selected == Some(i);
                        let r = egui::Frame::new()
                            .fill(if sel { if crate::theme::p().light { crate::theme::p().row_selected } else { Color32::from_rgb(70, 45, 30) } } else { Color32::TRANSPARENT })
                            .inner_margin(egui::Margin::same(3))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    device_icon(ui, 36.0);
                                    ui.vertical(|ui| {
                                        let star = if st.list.default == Some(i) { "  ★" } else { "" };
                                        ui.label(egui::RichText::new(format!("{}{star}", d.name)).strong());
                                        ui.label(subtitle(d));
                                    });
                                });
                            })
                            .response
                            .interact(egui::Sense::click());
                        if r.clicked() {
                            st.selected = Some(i);
                        }
                        if r.double_clicked() {
                            dbl = Some(i);
                        }
                    }
                    if let Some(i) = dbl {
                        st.editor = Some((Some(i), st.list.devices[i].clone()));
                    }
                });
            });
            ui.add_space(8.0);
            let bw = vec2(141.0, 30.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(tr("Trova il mio laser")).stroke(Stroke::new(1.0, ACCENT)).min_size(bw)).clicked() {
                    // a port in use can't be searched: free it first
                    app.laser.send(crate::laser::Cmd::Disconnect);
                    st.finder = Some(Finder::start(ctx));
                }
                if ui.add(egui::Button::new(tr("Crea manualmente")).min_size(bw)).clicked() {
                    let mut d = Device::default();
                    d.name = tr("Il mio laser").into();
                    st.editor = Some((None, d));
                }
                if ui.add(egui::Button::new(tr("Importa")).min_size(bw)).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("RayEtch device", &["bfdevice", "json"]).pick_file() {
                        match std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Device>(&s).ok()) {
                            Some(d) => {
                                st.list.devices.push(d);
                                st.selected = Some(st.list.devices.len() - 1);
                            }
                            None => app.status = tr("File del dispositivo non valido").into(),
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                let has = st.selected.is_some();
                if ui.add_enabled(has, egui::Button::new(tr("Rendi predefinito")).min_size(bw)).clicked() {
                    st.list.default = st.selected;
                }
                if ui.add_enabled(has, egui::Button::new(tr("Modifica")).min_size(bw)).clicked() {
                    if let Some(i) = st.selected {
                        st.editor = Some((Some(i), st.list.devices[i].clone()));
                    }
                }
                if ui.add_enabled(has, egui::Button::new(tr("Rimuovi")).min_size(bw)).clicked() {
                    if let Some(i) = st.selected {
                        st.list.devices.remove(i);
                        st.list.default = match st.list.default {
                            Some(d) if d == i => None,
                            Some(d) if d > i => Some(d - 1),
                            o => o,
                        };
                        st.selected = if st.list.devices.is_empty() { None } else { Some(i.min(st.list.devices.len() - 1)) };
                    }
                }
                if ui.add_enabled(st.selected.is_some(), egui::Button::new(tr("Esporta")).min_size(bw)).clicked() {
                    if let Some(d) = st.selected.and_then(|i| st.list.devices.get(i)) {
                        if let Some(p) = rfd::FileDialog::new().set_file_name(format!("{}.bfdevice", d.name)).save_file() {
                            let _ = std::fs::write(p, serde_json::to_string_pretty(d).unwrap_or_default());
                        }
                    }
                }
            });
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(tr("OK")).fill(Color32::from_rgb(170, 70, 20)).min_size(vec2(90.0, 26.0))).clicked() {
                    accept = true;
                }
                if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                    keep = false;
                }
            });
        });
        if !open {
            keep = false;
        }
    }

    // ---- Trova il mio laser
    if let Some(f) = st.finder.as_mut() {
        f.poll();
        let mut close = false;
        let mut take: Option<Device> = None;
        let mut again = false;
        let mut open = true;
        theme::centered(ctx, "finder", egui::Window::new(tr("Trova il mio laser")).open(&mut open).resizable(false).collapsible(false).fixed_size(vec2(560.0, 360.0))).show(ctx, |ui| {
            ui.label(tr("Collega il laser con il cavo USB e accendilo. RayEtch prova ogni porta USB e ascolta la risposta del controller."));
            ui.add_space(6.0);
            ui.add(egui::ProgressBar::new(f.progress).text(f.step.clone()).fill(ACCENT).animate(!f.done));
            ui.add_space(6.0);
            egui::Frame::new().fill(crate::theme::p().field).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_min_size(vec2(550.0, 190.0));
                egui::ScrollArea::vertical().max_height(190.0).show(ui, |ui| {
                    if f.results.is_empty() {
                        ui.label(egui::RichText::new(if f.done { tr("Nessun controller ha risposto. Controlla cavo, alimentazione e driver USB, poi riprova, oppure crea il dispositivo a mano.") } else { tr("In attesa di risposte…") }).color(crate::theme::p().text_dim));
                    }
                    for (i, r) in f.results.iter().enumerate() {
                        let size = match (r.width, r.height) {
                            (Some(w), Some(h)) => format!("{w:.0} × {h:.0} mm"),
                            _ => tr("area sconosciuta").into(),
                        };
                        let laser = match r.laser_mode {
                            Some(true) => tr("modalità laser attiva"),
                            Some(false) => tr("modalità laser spenta ($32=0)"),
                            None => "",
                        };
                        let text = format!("{}  —  {}  @ {} baud\n{}  |  {}  {}", short_port(&r.port), r.greeting, r.baud, r.usb, size, laser);
                        if ui.selectable_label(f.picked == Some(i), text).clicked() {
                            f.picked = Some(i);
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(f.done, egui::Button::new(tr("Cerca di nuovo"))).clicked() {
                    again = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(f.picked.is_some(), egui::Button::new(tr("Avanti")).fill(Color32::from_rgb(170, 70, 20)).min_size(vec2(90.0, 26.0))).clicked() {
                        take = f.picked.and_then(|i| f.results.get(i)).map(|r| r.to_device());
                    }
                    if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                        close = true;
                    }
                });
            });
        });
        if again {
            *f = Finder::start(ctx);
        }
        if let Some(d) = take {
            st.finder = None;
            st.editor = Some((None, d));
        } else if close || !open {
            st.finder = None;
        }
    }

    // ---- Crea manualmente / Modifica / confirm a found device
    if let Some((idx, d)) = st.editor.as_mut() {
        let mut done = None;
        let mut open = true;
        let title = if idx.is_some() { tr("Modifica dispositivo") } else { tr("Nuovo dispositivo") };
        theme::centered(ctx, "device_editor", egui::Window::new(title).open(&mut open).resizable(false).collapsible(false)).show(ctx, |ui| {
            egui::Grid::new("dev_edit").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label(tr("Nome"));
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(260.0));
                ui.end_row();
                ui.label(tr("Controller"));
                egui::ComboBox::from_id_salt("dev_ctrl").width(260.0).selected_text(d.controller.clone()).show_ui(ui, |ui| {
                    for c in CONTROLLERS {
                        if ui.selectable_label(d.controller == *c, *c).clicked() {
                            d.controller = (*c).into();
                            d.dynamic_power = *c != "GRBL-M3";
                        }
                    }
                });
                ui.end_row();
                ui.label(tr("Porta"));
                let shown = if d.port.is_empty() { tr("Automatica (la prima trovata)").to_string() } else { d.port.clone() };
                egui::ComboBox::from_id_salt("dev_port").width(260.0).selected_text(shown).show_ui(ui, |ui| {
                    if ui.selectable_label(d.port.is_empty(), tr("Automatica (la prima trovata)")).clicked() {
                        d.port.clear();
                    }
                    for p in crate::laser::list_ports() {
                        if ui.selectable_label(d.port == p, &p).clicked() {
                            d.port = p;
                        }
                    }
                });
                ui.end_row();
                ui.label(tr("Baud"));
                egui::ComboBox::from_id_salt("dev_baud").selected_text(d.baud.to_string()).show_ui(ui, |ui| {
                    for b in [9600u32, 57600, 115200, 230400, 250000] {
                        ui.selectable_value(&mut d.baud, b, b.to_string());
                    }
                });
                ui.end_row();
                ui.label(tr("Sorgente laser"));
                let src = crate::model::LASER_SOURCES.get(d.laser_source as usize).copied().unwrap_or("Sconosciuto");
                egui::ComboBox::from_id_salt("dev_source").selected_text(tr(src)).show_ui(ui, |ui| {
                    for (i, n) in crate::model::LASER_SOURCES.iter().enumerate() {
                        ui.selectable_value(&mut d.laser_source, i as u8, tr(n));
                    }
                });
                ui.end_row();
                ui.label(tr("Larghezza area (mm)"));
                ui.add(crate::i18n::dv(&mut d.width).speed(1.0).range(10.0..=15000.0));
                ui.end_row();
                ui.label(tr("Altezza area (mm)"));
                ui.add(crate::i18n::dv(&mut d.height).speed(1.0).range(10.0..=15000.0));
                ui.end_row();
                ui.label(tr("Origine macchina"));
                let names = [
                    (OriginCorner::FrontLeft, tr("Anteriore sinistra")),
                    (OriginCorner::RearLeft, tr("Posteriore sinistra")),
                    (OriginCorner::FrontRight, tr("Anteriore destra")),
                    (OriginCorner::RearRight, tr("Posteriore destra")),
                ];
                let cur = names.iter().find(|n| n.0 == d.origin).map(|n| n.1).unwrap_or("");
                egui::ComboBox::from_id_salt("dev_origin").selected_text(cur).show_ui(ui, |ui| {
                    for (o, n) in names {
                        ui.selectable_value(&mut d.origin, o, n);
                    }
                });
                ui.end_row();
                ui.label(tr("Valore S massimo ($30)"));
                ui.add(crate::i18n::dv(&mut d.s_max).speed(10.0).range(1.0..=100000.0));
                ui.end_row();
            });
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(!d.name.trim().is_empty(), egui::Button::new(tr("OK")).fill(Color32::from_rgb(170, 70, 20)).min_size(vec2(90.0, 26.0))).clicked() {
                    done = Some(true);
                }
                if ui.add(egui::Button::new(crate::i18n::cancel()).min_size(vec2(90.0, 26.0))).clicked() {
                    done = Some(false);
                }
            });
        });
        if !open {
            done = Some(false);
        }
        if let Some(save) = done {
            let (idx, d) = st.editor.take().unwrap();
            if save {
                match idx {
                    Some(i) if i < st.list.devices.len() => st.list.devices[i] = d,
                    _ => {
                        st.list.devices.push(d);
                        st.selected = Some(st.list.devices.len() - 1);
                        if st.list.default.is_none() {
                            st.list.default = st.selected;
                        }
                    }
                }
            }
        }
    }

    if accept {
        st.list.save();
        app.device_list = st.list.clone();
        app.device_sel = None;
        if let Some(i) = st.selected.filter(|i| *i < app.device_list.devices.len()) {
            app.checkpoint_now();
            app.use_device(i);
        }
        keep = false;
    }
    if keep {
        app.devices_ui = Some(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_device_takes_grbl_settings() {
        let f = Found { port: "/dev/cu.usbserial-10".into(), usb: String::new(), greeting: "Grbl 1.1h ['$' for help]".into(), baud: 115200, width: Some(410.0), height: Some(400.0), s_max: Some(1000.0), laser_mode: Some(true), max_rate: [Some(6000.0), Some(3000.0)], version: Some("1.1h.20190825".into()), model: model_name("[MSG:Using machine:Ortur Laser Master 2 Pro]") };
        let d = f.to_device();
        assert_eq!(d.controller, "GRBL");
        assert_eq!(d.width, 410.0);
        assert_eq!(d.port, "/dev/cu.usbserial-10");
        assert_eq!(usb_label(0x1a86, 0x7523), Some("CH340: probabilmente GRBL"));
        assert_eq!(d.name, "Ortur LM2 Pro");
        assert_eq!(d.rapid_speed, 50.0);
        assert!(is_old_grbl("1.1e") && is_old_grbl("0.9j") && !is_old_grbl("1.1f") && !is_old_grbl("1.1h.2019"));
    }

    /// Real-hardware check, run on request: `BF_PORT=/dev/cu.usbserial-10 cargo test hw_ -- --ignored --nocapture`.
    /// Only reads (greeting, $i, $$); nothing moves and the beam stays off.
    #[test]
    #[ignore]
    fn hw_probe_real_laser() {
        let port = std::env::var("BF_PORT").expect("BF_PORT");
        let f = probe(&port, BAUD_STD).or_else(|| probe(&port, BAUD_FAST)).expect("no GRBL answer");
        println!("HW greeting={:?} baud={} ver={:?} model={:?} size={:?}x{:?} s_max={:?} laser_mode={:?} rates={:?}",
            f.greeting, f.baud, f.version, f.model, f.width, f.height, f.s_max, f.laser_mode, f.max_rate);
        let d = f.to_device();
        println!("HW device name={} controller={} {}x{} mm", d.name, d.controller, d.width, d.height);
    }
}
