//! GRBL serial connection in a background thread: send-and-wait-for-ok streaming,
//! realtime commands, and status polling.

use crate::i18n::{tr, trf};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub enum Cmd {
    Connect(String, u32),
    Disconnect,
    /// Queue lines to send (job, jog, console)
    Lines(Vec<String>, bool),
    /// Single realtime byte: '?', '!', '~', 0x18
    Realtime(u8),
    StopJob,
}

pub enum Ev {
    Connected(String),
    Disconnected(String),
    Received(String),
    Sent(String),
    Progress(usize, usize),
    JobDone,
    Status { state: String, pos: Option<[f64; 2]> },
}

pub struct Laser {
    tx: Sender<Cmd>,
    rx: Receiver<Ev>,
}

impl Laser {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        let (tx, crx) = channel::<Cmd>();
        let (etx, rx) = channel::<Ev>();
        std::thread::spawn(move || worker(crx, etx, ctx));
        Laser { tx, rx }
    }
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
    pub fn poll(&self) -> Vec<Ev> {
        self.rx.try_iter().collect()
    }
}

pub fn list_ports() -> Vec<String> {
    serialport::available_ports().map(|v| v.into_iter().map(|p| p.port_name).collect()).unwrap_or_default()
}

fn worker(rx: Receiver<Cmd>, tx: Sender<Ev>, ctx: eframe::egui::Context) {
    let mut port: Option<Box<dyn serialport::SerialPort>> = None;
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut job_total = 0usize;
    let mut job_sent = 0usize;
    let mut in_job = false;
    let mut waiting_ok = false;
    let mut buf = String::new();
    let mut last_status = Instant::now();
    let emit = |ev: Ev, tx: &Sender<Ev>| {
        let _ = tx.send(ev);
        ctx.request_repaint();
    };
    loop {
        // commands
        let timeout = if port.is_some() { Duration::from_millis(5) } else { Duration::from_millis(100) };
        match rx.recv_timeout(timeout) {
            Ok(cmd) => match cmd {
                Cmd::Connect(name, baud) => {
                    port = None;
                    match serialport::new(&name, baud).timeout(Duration::from_millis(5)).open() {
                        Ok(p) => {
                            port = Some(p);
                            queue.clear();
                            waiting_ok = false;
                            emit(Ev::Connected(name), &tx);
                        }
                        Err(e) => emit(Ev::Disconnected(trf("Impossibile aprire {}: {}", &[&name, &e])), &tx),
                    }
                }
                Cmd::Disconnect => {
                    port = None;
                    queue.clear();
                    in_job = false;
                    emit(Ev::Disconnected(tr("Scollegato").into()), &tx);
                }
                Cmd::Lines(lines, is_job) => {
                    if port.is_none() {
                        emit(Ev::Received(tr("Nessun laser collegato.").into()), &tx);
                    } else {
                        if is_job {
                            in_job = true;
                            job_total = lines.len();
                            job_sent = 0;
                        }
                        queue.extend(lines.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with(';')));
                        if is_job {
                            job_total = queue.len();
                        }
                    }
                }
                Cmd::Realtime(b) => {
                    if let Some(p) = port.as_mut() {
                        let _ = p.write_all(&[b]);
                    }
                }
                Cmd::StopJob => {
                    queue.clear();
                    in_job = false;
                    waiting_ok = false;
                    if let Some(p) = port.as_mut() {
                        let _ = p.write_all(&[crate::gcode::ABORT_BYTE]);
                    }
                    emit(Ev::JobDone, &tx);
                }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
        let Some(p) = port.as_mut() else { continue };
        // read
        let mut chunk = [0u8; 512];
        match p.read(&mut chunk) {
            Ok(n) if n > 0 => {
                buf.push_str(&String::from_utf8_lossy(&chunk[..n]));
                while let Some(i) = buf.find('\n') {
                    let line = buf[..i].trim().to_string();
                    buf.drain(..=i);
                    if line.is_empty() {
                        continue;
                    }
                    if line.starts_with('<') {
                        let inner = line.trim_matches(|c| c == '<' || c == '>');
                        let mut parts = inner.split('|');
                        let state = parts.next().unwrap_or("").to_string();
                        let mut pos = None;
                        for part in parts {
                            if let Some(v) = part.strip_prefix("MPos:").or_else(|| part.strip_prefix("WPos:")) {
                                let n: Vec<f64> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                                if n.len() >= 2 {
                                    pos = Some([n[0], n[1]]);
                                }
                            }
                        }
                        emit(Ev::Status { state, pos }, &tx);
                        continue;
                    }
                    if line == "ok" || line.starts_with("error") {
                        waiting_ok = false;
                    }
                    if line.starts_with("ALARM") {
                        queue.clear();
                        in_job = false;
                        waiting_ok = false;
                    }
                    if line != "ok" || !in_job {
                        emit(Ev::Received(line), &tx);
                    }
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                port = None;
                emit(Ev::Disconnected(trf("Collegamento perso: {}", &[&e])), &tx);
                continue;
            }
        }
        // write
        if !waiting_ok {
            if let Some(line) = queue.pop_front() {
                if p.write_all(format!("{line}\n").as_bytes()).is_ok() {
                    waiting_ok = true;
                    if in_job {
                        job_sent += 1;
                        if job_sent % 20 == 0 || job_sent == job_total {
                            emit(Ev::Progress(job_sent, job_total), &tx);
                        }
                    } else {
                        emit(Ev::Sent(line), &tx);
                    }
                }
            } else if in_job {
                in_job = false;
                emit(Ev::JobDone, &tx);
            }
        }
        if last_status.elapsed() > Duration::from_millis(300) {
            last_status = Instant::now();
            let _ = p.write_all(b"?");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for(l: &Laser, secs: u64, mut f: impl FnMut(&Ev) -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            for e in l.poll() {
                match &e {
                    Ev::Received(t) => println!("HW < {t}"),
                    Ev::Sent(t) => println!("HW > {t}"),
                    Ev::Status { state, pos } => println!("HW status {state} {pos:?}"),
                    Ev::Connected(p) => println!("HW connected {p}"),
                    Ev::Disconnected(r) => println!("HW disconnected {r}"),
                    Ev::Progress(a, b) => println!("HW progress {a}/{b}"),
                    Ev::JobDone => println!("HW job done"),
                }
                if f(&e) {
                    return true;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn auto_search_tries_only_usb_ports() {
        let ports: Vec<String> = ["/dev/cu.Bluetooth-Incoming-Port", "/dev/cu.debug-console", "/dev/cu.JBLCharge3",
            "/dev/cu.JBLTUNE510BT", "/dev/cu.usbserial-10", "/dev/tty.usbserial-10"].iter().map(|s| s.to_string()).collect();
        if !cfg!(windows) {
            assert_eq!(crate::app::auto_port_order("", &ports), vec!["/dev/cu.usbserial-10".to_string()]);
        }
        assert_eq!(crate::app::auto_port_order("/dev/cu.JBLCharge3", &ports), vec!["/dev/cu.JBLCharge3".to_string()]);
    }

    /// Real-hardware check of the streaming worker, run on request with BF_PORT set.
    /// Connects, reads the status and build info, then disconnects; nothing moves.
    #[test]
    #[ignore]
    fn hw_worker_connects_and_queries() {
        let port = std::env::var("BF_PORT").expect("BF_PORT");
        let l = Laser::new(eframe::egui::Context::default());
        l.send(Cmd::Connect(port, 115200));
        assert!(wait_for(&l, 3, |e| matches!(e, Ev::Connected(_))));
        assert!(wait_for(&l, 4, |e| matches!(e, Ev::Status { .. })), "no status report");
        l.send(Cmd::Lines(vec!["$I".into(), "$G".into()], false));
        assert!(wait_for(&l, 4, |e| matches!(e, Ev::Received(t) if t.starts_with("[GC:"))), "no $G answer");
        l.send(Cmd::Disconnect);
        assert!(wait_for(&l, 2, |e| matches!(e, Ev::Disconnected(_))));
    }

    /// Real-hardware motion check (beam off, every move S0), run on request with BF_PORT set:
    /// unlock, jog, box frame and shapes-only frame of two squares, then back to where it started.
    #[test]
    #[ignore]
    fn hw_motion_frames_with_beam_off() {
        use crate::gcode::*;
        use crate::model::*;
        let port = std::env::var("BF_PORT").expect("BF_PORT");
        let l = Laser::new(eframe::egui::Context::default());
        l.send(Cmd::Connect(port, 115200));
        assert!(wait_for(&l, 3, |e| matches!(e, Ev::Connected(_))));
        let mut start = [0.0, 0.0];
        assert!(wait_for(&l, 4, |e| if let Ev::Status { pos: Some(p), .. } = e { start = *p; true } else { false }));
        let idle = |l: &Laser| wait_for(l, 30, |e| matches!(e, Ev::Status { state, .. } if state == "Idle"));
        let run = |l: &Laser, lines: Vec<String>| {
            assert!(lines.iter().all(|x| !x.contains("M3") && !x.contains("M4") && (!x.starts_with("G1") || x.contains("S0"))));
            l.send(Cmd::Lines(lines, true));
            assert!(wait_for(l, 30, |e| matches!(e, Ev::JobDone)), "job not acknowledged");
            std::thread::sleep(Duration::from_millis(400));
            assert!(idle(l), "machine did not return to Idle");
        };
        l.send(Cmd::Lines(vec!["$X".into()], false));
        std::thread::sleep(Duration::from_millis(300));
        run(&l, vec![jog_line(false, 10.0, 10.0, 3000.0)]);
        run(&l, vec![jog_line(false, -10.0, -10.0, 3000.0)]);
        let mut doc = Document::default();
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 30.0, 30.0));
        doc.add_shape(Shape::new(0, Geometry::Rect { w: 10.0, h: 10.0, radius: 0.0 }, 0, 70.0, 60.0));
        let opts = JobOptions { start_from: StartFrom::AbsoluteCoords, job_origin: 6, selected_only: false, optimize: true };
        let job = plan(&doc, &[], &opts);
        let f = frame_gcode(&doc, &job, &opts, 3000.0, "0");
        println!("HW frame {f:?}");
        run(&l, f);
        let o = outline_frame_gcode(&doc, &job, &opts, 0.0, "0");
        println!("HW outline {o:?}");
        run(&l, o);
        run(&l, vec![jog_line(true, start[0], start[1], 3000.0)]);
        l.send(Cmd::Disconnect);
        assert!(wait_for(&l, 2, |e| matches!(e, Ev::Disconnected(_))));
    }
}
