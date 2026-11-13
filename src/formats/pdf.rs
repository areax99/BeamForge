//! PDF and Adobe Illustrator drawings (AITranslator 0x1000615e4: modern .ai files are PDF
//! files and are read from their first page; older PostScript-style .ai files are read from
//! their path operators). Lengths are points: 1 pt = 25.4 / 72 mm.

use super::{Piece, cubic, finish};
use crate::i18n::tr;
use crate::model::{Polyline, nearest_layer};
use std::collections::HashMap;
use std::io::Read;

pub const PT_MM: f64 = 25.4 / 72.0;

pub fn import(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    let head = &bytes[..bytes.len().min(1024)];
    let pieces = if find(head, b"%PDF").is_some() || find(bytes, b"/Type").is_some() && find(bytes, b" obj").is_some() {
        read_pdf(bytes)?
    } else if find(head, b"%!PS").is_some() || find(bytes, b"%%EndSetup").is_some() {
        legacy_ai(bytes)
    } else {
        return Err(tr("Il file non sembra un PDF né un file Illustrator.").into());
    };
    if pieces.is_empty() {
        return Err(tr("Il file non contiene tracciati vettoriali utilizzabili.").into());
    }
    Ok(pieces)
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

// ------------------------------------------------------------ PDF objects

#[derive(Clone, Debug)]
enum Obj {
    Num(f64),
    Name(String),
    Str,
    Arr(Vec<Obj>),
    Dict(HashMap<String, Obj>),
    Ref(u32),
    Op(String),
    Null,
}

impl Obj {
    fn num(&self) -> Option<f64> {
        if let Obj::Num(n) = self { Some(*n) } else { None }
    }
    fn dict(&self) -> Option<&HashMap<String, Obj>> {
        if let Obj::Dict(d) = self { Some(d) } else { None }
    }
    fn name(&self) -> Option<&str> {
        if let Obj::Name(n) = self { Some(n) } else { None }
    }
}

struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
}

fn is_delim(c: u8) -> bool {
    matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

impl<'a> Lexer<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c.is_ascii_whitespace() || c == 0 {
                self.i += 1;
            } else if c == b'%' {
                while self.i < self.b.len() && self.b[self.i] != b'\n' && self.b[self.i] != b'\r' {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    /// One object; `R` references are folded in by the array/dict readers.
    fn obj(&mut self) -> Option<Obj> {
        self.ws();
        let c = *self.b.get(self.i)?;
        match c {
            b'/' => {
                self.i += 1;
                let s = self.i;
                while self.i < self.b.len() && !self.b[self.i].is_ascii_whitespace() && !is_delim(self.b[self.i]) {
                    self.i += 1;
                }
                Some(Obj::Name(String::from_utf8_lossy(&self.b[s..self.i]).into()))
            }
            b'(' => {
                let mut depth = 0;
                while self.i < self.b.len() {
                    match self.b[self.i] {
                        b'\\' => self.i += 1,
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                self.i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    self.i += 1;
                }
                Some(Obj::Str)
            }
            b'<' if self.b.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                let mut d = HashMap::new();
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    if self.b[self.i] == b'>' {
                        self.i += 2;
                        break;
                    }
                    let k = match self.obj()? {
                        Obj::Name(n) => n,
                        _ => continue,
                    };
                    let v = self.value()?;
                    d.insert(k, v);
                }
                Some(Obj::Dict(d))
            }
            b'<' => {
                while self.i < self.b.len() && self.b[self.i] != b'>' {
                    self.i += 1;
                }
                self.i += 1;
                Some(Obj::Str)
            }
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    v.push(self.value()?);
                }
                Some(Obj::Arr(v))
            }
            b'{' | b'}' | b']' | b'>' | b')' => {
                self.i += 1;
                Some(Obj::Op((c as char).to_string()))
            }
            _ => {
                let s = self.i;
                while self.i < self.b.len() && !self.b[self.i].is_ascii_whitespace() && !is_delim(self.b[self.i]) {
                    self.i += 1;
                }
                if self.i == s {
                    self.i += 1;
                    return Some(Obj::Op(String::new()));
                }
                let t = std::str::from_utf8(&self.b[s..self.i]).unwrap_or("");
                if let Ok(n) = t.parse::<f64>() {
                    Some(Obj::Num(n))
                } else if t == "null" {
                    Some(Obj::Null)
                } else {
                    Some(Obj::Op(t.to_string()))
                }
            }
        }
    }

    /// An object inside an array or dictionary: `n g R` becomes a reference.
    fn value(&mut self) -> Option<Obj> {
        let o = self.obj()?;
        if let Obj::Num(n) = o {
            let save = self.i;
            if let Some(Obj::Num(_)) = self.obj() {
                let save2 = self.i;
                if let Some(Obj::Op(op)) = self.obj() {
                    if op == "R" {
                        return Some(Obj::Ref(n as u32));
                    }
                }
                let _ = save2;
            }
            self.i = save;
        }
        Some(o)
    }
}

struct Pdf {
    objs: HashMap<u32, (Obj, Option<Vec<u8>>)>,
}

impl Pdf {
    fn get<'a>(&'a self, o: &'a Obj) -> &'a Obj {
        let mut cur = o;
        for _ in 0..16 {
            if let Obj::Ref(n) = cur {
                match self.objs.get(n) {
                    Some((v, _)) => cur = v,
                    None => return &Obj::Null,
                }
            } else {
                break;
            }
        }
        cur
    }
    fn stream(&self, o: &Obj) -> Option<Vec<u8>> {
        let Obj::Ref(n) = o else { return None };
        let (d, raw) = self.objs.get(n)?;
        decode(d.dict()?, raw.as_ref()?, self)
    }
}

fn decode(d: &HashMap<String, Obj>, raw: &[u8], pdf: &Pdf) -> Option<Vec<u8>> {
    let filters: Vec<String> = match d.get("Filter").map(|f| pdf.get(f)) {
        Some(Obj::Name(n)) => vec![n.clone()],
        Some(Obj::Arr(a)) => a.iter().filter_map(|x| x.name().map(String::from)).collect(),
        _ => vec![],
    };
    let mut data = raw.to_vec();
    for f in filters {
        match f.as_str() {
            "FlateDecode" | "Fl" => {
                let mut out = Vec::new();
                let mut z = flate2::read::ZlibDecoder::new(&data[..]);
                if z.read_to_end(&mut out).is_err() && out.is_empty() {
                    return None;
                }
                data = out;
            }
            _ => return None,
        }
    }
    Some(data)
}

fn parse_pdf(b: &[u8]) -> Pdf {
    let mut objs = HashMap::new();
    let mut i = 0;
    while let Some(off) = find(&b[i..], b" obj") {
        let at = i + off;
        i = at + 4;
        // read "num gen" backwards
        let mut s = at;
        let mut fields = Vec::new();
        for _ in 0..2 {
            while s > 0 && b[s - 1].is_ascii_whitespace() {
                s -= 1;
            }
            let e = s;
            while s > 0 && b[s - 1].is_ascii_digit() {
                s -= 1;
            }
            fields.push(std::str::from_utf8(&b[s..e]).ok().and_then(|t| t.parse::<u32>().ok()));
        }
        let Some(num) = fields[1] else { continue };
        let mut lx = Lexer { b, i };
        let Some(obj) = lx.value() else { continue };
        lx.ws();
        let mut stream = None;
        if b[lx.i..].starts_with(b"stream") {
            let mut st = lx.i + 6;
            if b.get(st) == Some(&b'\r') {
                st += 1;
            }
            if b.get(st) == Some(&b'\n') {
                st += 1;
            }
            let len = obj.dict().and_then(|d| d.get("Length")).and_then(|l| l.num()).map(|n| n as usize);
            let end = match len {
                Some(n) if st + n <= b.len() && b[st + n..].iter().take(12).any(|_| true) && find(&b[st + n..(st + n + 12).min(b.len())], b"endstream").is_some() => st + n,
                _ => st + find(&b[st..], b"endstream").unwrap_or(b.len() - st),
            };
            stream = Some(b[st..end].to_vec());
            i = end;
        }
        objs.insert(num, (obj, stream));
    }
    let mut pdf = Pdf { objs };
    // compressed object streams
    let streams: Vec<u32> = pdf
        .objs
        .iter()
        .filter(|(_, (o, _))| o.dict().and_then(|d| d.get("Type")).and_then(|t| t.name()) == Some("ObjStm"))
        .map(|(k, _)| *k)
        .collect();
    for k in streams {
        let Some(data) = pdf.stream(&Obj::Ref(k)) else { continue };
        let d = pdf.objs[&k].0.dict().cloned().unwrap_or_default();
        let n = d.get("N").and_then(|v| v.num()).unwrap_or(0.0) as usize;
        let first = d.get("First").and_then(|v| v.num()).unwrap_or(0.0) as usize;
        let mut lx = Lexer { b: &data, i: 0 };
        let mut heads = Vec::new();
        for _ in 0..n {
            if let (Some(Obj::Num(a)), Some(Obj::Num(o))) = (lx.obj(), lx.obj()) {
                heads.push((a as u32, o as usize));
            }
        }
        for (num, off) in heads {
            let mut lx = Lexer { b: &data, i: first + off };
            if let Some(o) = lx.value() {
                pdf.objs.entry(num).or_insert((o, None));
            }
        }
    }
    pdf
}

/// The page objects in order.
fn pages(pdf: &Pdf) -> Vec<u32> {
    let is = |o: &Obj, t: &str| o.dict().and_then(|d| d.get("Type")).and_then(|v| v.name()) == Some(t);
    let root = pdf.objs.iter().find(|(_, (o, _))| is(o, "Pages") && !o.dict().map(|d| d.contains_key("Parent")).unwrap_or(false)).map(|(k, _)| *k);
    let mut out = Vec::new();
    fn walk(pdf: &Pdf, k: u32, out: &mut Vec<u32>, depth: usize) {
        if depth > 32 {
            return;
        }
        let Some((o, _)) = pdf.objs.get(&k) else { return };
        let Some(d) = o.dict() else { return };
        if let Some(Obj::Arr(kids)) = d.get("Kids").map(|x| pdf.get(x)) {
            for kid in kids {
                if let Obj::Ref(n) = kid {
                    walk(pdf, *n, out, depth + 1);
                }
            }
        } else {
            out.push(k);
        }
    }
    if let Some(r) = root {
        walk(pdf, r, &mut out, 0);
    }
    if out.is_empty() {
        let mut v: Vec<u32> = pdf.objs.iter().filter(|(_, (o, _))| is(o, "Page")).map(|(k, _)| *k).collect();
        v.sort();
        out = v;
    }
    out
}

fn read_pdf(b: &[u8]) -> Result<Vec<Piece>, String> {
    let pdf = parse_pdf(b);
    let pages = pages(&pdf);
    let Some(page) = pages.first() else { return Err(tr("Il PDF non contiene pagine leggibili.").into()) };
    let pd = pdf.objs[page].0.dict().cloned().unwrap_or_default();
    // inherit MediaBox/Resources from parents
    let mut media = [0.0; 4];
    let mut res: Option<Obj> = None;
    let mut cur = Some(pd.clone());
    for _ in 0..16 {
        let Some(d) = cur.take() else { break };
        if media == [0.0; 4] {
            if let Some(Obj::Arr(a)) = d.get("MediaBox").map(|m| pdf.get(m)) {
                for (k, v) in a.iter().take(4).enumerate() {
                    media[k] = pdf.get(v).num().unwrap_or(0.0);
                }
            }
        }
        if res.is_none() {
            res = d.get("Resources").map(|r| pdf.get(r).clone());
        }
        cur = d.get("Parent").and_then(|p| pdf.get(p).dict().cloned());
    }
    let mut content = Vec::new();
    match pd.get("Contents") {
        Some(Obj::Arr(a)) => {
            for c in a {
                if let Some(s) = pdf.stream(c) {
                    content.extend(s);
                    content.push(b'\n');
                }
            }
        }
        Some(c @ Obj::Ref(_)) => {
            if let Some(Obj::Arr(a)) = Some(pdf.get(c)).filter(|o| matches!(o, Obj::Arr(_))) {
                for c in a {
                    if let Some(s) = pdf.stream(c) {
                        content.extend(s);
                    }
                }
            } else if let Some(s) = pdf.stream(c) {
                content = s;
            }
        }
        _ => {}
    }
    let mut ip = Interp { pdf: &pdf, pieces: Vec::new() };
    let origin = [media[0], media[1]];
    let base = [PT_MM, 0.0, 0.0, PT_MM, -origin[0] * PT_MM, -origin[1] * PT_MM];
    ip.run(&content, res.as_ref(), base, 0);
    Ok(ip.pieces)
}

type M = [f64; 6];
fn mul(a: &M, b: &M) -> M {
    // a then b
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}
fn apply(m: &M, x: f64, y: f64) -> [f64; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

#[derive(Clone)]
struct GState {
    ctm: M,
    stroke: [u8; 3],
    fill: [u8; 3],
}

fn color(nums: &[f64]) -> Option<[u8; 3]> {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    match nums.len() {
        1 => Some([c(nums[0]); 3]),
        3 => Some([c(nums[0]), c(nums[1]), c(nums[2])]),
        4 => {
            let k = 1.0 - nums[3];
            Some([c((1.0 - nums[0]) * k), c((1.0 - nums[1]) * k), c((1.0 - nums[2]) * k)])
        }
        _ => None,
    }
}

struct Interp<'a> {
    pdf: &'a Pdf,
    pieces: Vec<Piece>,
}

impl Interp<'_> {
    fn run(&mut self, content: &[u8], res: Option<&Obj>, ctm: M, depth: usize) {
        let mut gs = GState { ctm, stroke: [0; 3], fill: [0; 3] };
        let mut stack: Vec<GState> = Vec::new();
        let mut nums: Vec<f64> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut path: Vec<Polyline> = Vec::new();
        let mut cur: Vec<[f64; 2]> = Vec::new();
        let mut last = [0.0, 0.0];
        let mut start = [0.0, 0.0];
        let mut lx = Lexer { b: content, i: 0 };
        let mut clip_next = false;
        while let Some(o) = lx.obj() {
            match o {
                Obj::Num(n) => {
                    nums.push(n);
                    continue;
                }
                Obj::Name(n) => {
                    names.push(n);
                    continue;
                }
                Obj::Op(op) => {
                    let t = |x: f64, y: f64, g: &GState| apply(&g.ctm, x, y);
                    let n = &nums;
                    match op.as_str() {
                        "q" => stack.push(gs.clone()),
                        "Q" => {
                            if let Some(g) = stack.pop() {
                                gs = g
                            }
                        }
                        "cm" if n.len() >= 6 => {
                            let m: M = [n[n.len() - 6], n[n.len() - 5], n[n.len() - 4], n[n.len() - 3], n[n.len() - 2], n[n.len() - 1]];
                            gs.ctm = mul(&m, &gs.ctm);
                        }
                        "m" if n.len() >= 2 => {
                            if cur.len() > 1 {
                                path.push(Polyline { pts: std::mem::take(&mut cur), closed: false });
                            }
                            cur.clear();
                            last = [n[n.len() - 2], n[n.len() - 1]];
                            start = last;
                            cur.push(t(last[0], last[1], &gs));
                        }
                        "l" if n.len() >= 2 => {
                            last = [n[n.len() - 2], n[n.len() - 1]];
                            if cur.is_empty() {
                                cur.push(t(start[0], start[1], &gs));
                            }
                            cur.push(t(last[0], last[1], &gs));
                        }
                        "c" | "v" | "y" if n.len() >= 4 => {
                            let k = n.len();
                            let (p1, p2, p3) = match op.as_str() {
                                "c" if k >= 6 => ([n[k - 6], n[k - 5]], [n[k - 4], n[k - 3]], [n[k - 2], n[k - 1]]),
                                "v" => (last, [n[k - 4], n[k - 3]], [n[k - 2], n[k - 1]]),
                                _ => ([n[k - 4], n[k - 3]], [n[k - 2], n[k - 1]], [n[k - 2], n[k - 1]]),
                            };
                            let p0 = last;
                            if cur.is_empty() {
                                cur.push(t(p0[0], p0[1], &gs));
                            }
                            for p in cubic(p0, p1, p2, p3) {
                                cur.push(t(p[0], p[1], &gs));
                            }
                            last = p3;
                        }
                        "h" => {
                            if cur.len() > 1 {
                                path.push(finish(std::mem::take(&mut cur), 1e-6).closed_anyway());
                            }
                            cur.clear();
                            last = start;
                        }
                        "re" if n.len() >= 4 => {
                            let k = n.len();
                            let (x, y, w, h) = (n[k - 4], n[k - 3], n[k - 2], n[k - 1]);
                            if cur.len() > 1 {
                                path.push(Polyline { pts: std::mem::take(&mut cur), closed: false });
                            }
                            cur.clear();
                            path.push(Polyline { pts: vec![t(x, y, &gs), t(x + w, y, &gs), t(x + w, y + h, &gs), t(x, y + h, &gs)], closed: true });
                            last = [x, y];
                            start = last;
                        }
                        "W" | "W*" => clip_next = true,
                        "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                            if cur.len() > 1 {
                                let closing = matches!(op.as_str(), "s" | "b" | "b*");
                                let pl = finish(std::mem::take(&mut cur), 1e-6);
                                path.push(if closing { pl.closed_anyway() } else { pl });
                            }
                            cur.clear();
                            let paths = std::mem::take(&mut path);
                            if op != "n" && !clip_next && !paths.is_empty() {
                                let stroked = matches!(op.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*");
                                let c = if stroked { gs.stroke } else { gs.fill };
                                let paths = if stroked { paths } else { paths.into_iter().map(|p| p.closed_anyway()).collect() };
                                self.pieces.push(Piece { layer: nearest_layer(c[0], c[1], c[2]), paths });
                            }
                            clip_next = false;
                        }
                        "RG" | "G" | "K" | "SC" | "SCN" => {
                            if let Some(c) = color(&n[n.len().saturating_sub(if op == "RG" { 3 } else if op == "K" { 4 } else if op == "G" { 1 } else { n.len().min(4) })..]) {
                                gs.stroke = c;
                            }
                        }
                        "rg" | "g" | "k" | "sc" | "scn" => {
                            if let Some(c) = color(&n[n.len().saturating_sub(if op == "rg" { 3 } else if op == "k" { 4 } else if op == "g" { 1 } else { n.len().min(4) })..]) {
                                gs.fill = c;
                            }
                        }
                        "Do" if depth < 8 => {
                            if let Some(name) = names.last() {
                                self.form(name, res, &gs.ctm, depth);
                            }
                        }
                        "BI" => {
                            // skip inline image data up to EI
                            if let Some(p) = find(&content[lx.i..], b"EI") {
                                lx.i += p + 2;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            nums.clear();
            names.clear();
        }
    }

    fn form(&mut self, name: &str, res: Option<&Obj>, ctm: &M, depth: usize) {
        let pdf = self.pdf;
        let Some(rd) = res.map(|r| pdf.get(r)).and_then(|r| r.dict()) else { return };
        let Some(xo) = rd.get("XObject").map(|x| pdf.get(x)).and_then(|x| x.dict()) else { return };
        let Some(r @ Obj::Ref(n)) = xo.get(name) else { return };
        let Some((o, _)) = pdf.objs.get(n) else { return };
        let Some(d) = o.dict() else { return };
        if d.get("Subtype").and_then(|s| s.name()) != Some("Form") {
            return;
        }
        let Some(data) = pdf.stream(r) else { return };
        let mut m: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        if let Some(Obj::Arr(a)) = d.get("Matrix").map(|x| pdf.get(x)) {
            for (k, v) in a.iter().take(6).enumerate() {
                m[k] = pdf.get(v).num().unwrap_or(m[k]);
            }
        }
        let sub_res = d.get("Resources").map(|r| pdf.get(r).clone());
        let res = sub_res.as_ref().or(res);
        self.run(&data, res, mul(&m, ctm), depth + 1);
    }
}

trait ClosedAnyway {
    fn closed_anyway(self) -> Self;
}
impl ClosedAnyway for Polyline {
    fn closed_anyway(mut self) -> Self {
        if self.pts.len() > 2 {
            self.closed = true;
        }
        self
    }
}

// ------------------------------------------------------------ legacy Illustrator

/// Old (PostScript-based) Illustrator files: m/l/L/c/C/v/V/y/Y path operators after the setup,
/// painted by S/s/f/F/b/B (lower case closes); K/k (CMYK), G/g (grey) and XA/Xa (RGB) colours.
fn legacy_ai(b: &[u8]) -> Vec<Piece> {
    let text = String::from_utf8_lossy(b);
    let body = text.find("%%EndSetup").map(|i| &text[i..]).unwrap_or(&text);
    let m: M = [PT_MM, 0.0, 0.0, PT_MM, 0.0, 0.0];
    let mut pieces = Vec::new();
    let (mut stroke, mut fill) = ([0u8; 3], [0u8; 3]);
    let mut path: Vec<Polyline> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut last = [0.0, 0.0];
    for line in body.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some(op) = toks.last() else { continue };
        let n: Vec<f64> = toks[..toks.len() - 1].iter().filter_map(|t| t.parse().ok()).collect();
        let t = |x: f64, y: f64| apply(&m, x, y);
        match *op {
            "m" if n.len() == 2 => {
                if cur.len() > 1 {
                    path.push(Polyline { pts: std::mem::take(&mut cur), closed: false });
                }
                cur.clear();
                last = [n[0], n[1]];
                cur.push(t(n[0], n[1]));
            }
            "l" | "L" if n.len() == 2 => {
                last = [n[0], n[1]];
                cur.push(t(n[0], n[1]));
            }
            "c" | "C" if n.len() == 6 => {
                for p in cubic(last, [n[0], n[1]], [n[2], n[3]], [n[4], n[5]]) {
                    cur.push(t(p[0], p[1]));
                }
                last = [n[4], n[5]];
            }
            "v" | "V" if n.len() == 4 => {
                for p in cubic(last, last, [n[0], n[1]], [n[2], n[3]]) {
                    cur.push(t(p[0], p[1]));
                }
                last = [n[2], n[3]];
            }
            "y" | "Y" if n.len() == 4 => {
                for p in cubic(last, [n[0], n[1]], [n[2], n[3]], [n[2], n[3]]) {
                    cur.push(t(p[0], p[1]));
                }
                last = [n[2], n[3]];
            }
            "K" if n.len() == 4 => stroke = color(&n).unwrap_or(stroke),
            "k" if n.len() == 4 => fill = color(&n).unwrap_or(fill),
            "G" if n.len() == 1 => stroke = color(&[1.0 - n[0]]).unwrap_or(stroke),
            "g" if n.len() == 1 => fill = color(&[1.0 - n[0]]).unwrap_or(fill),
            "XA" if n.len() == 3 => stroke = color(&n).unwrap_or(stroke),
            "Xa" if n.len() == 3 => fill = color(&n).unwrap_or(fill),
            "S" | "s" | "f" | "F" | "b" | "B" | "n" | "N" if n.is_empty() => {
                if cur.len() > 1 {
                    let pl = finish(std::mem::take(&mut cur), 1e-6);
                    path.push(if op.chars().all(|c| c.is_lowercase()) { pl.closed_anyway() } else { pl });
                }
                cur.clear();
                let paths = std::mem::take(&mut path);
                if !matches!(*op, "n" | "N") && !paths.is_empty() {
                    let c = if matches!(*op, "S" | "s" | "B" | "b") { stroke } else { fill };
                    pieces.push(Piece { layer: nearest_layer(c[0], c[1], c[2]), paths });
                }
            }
            _ => {}
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_pdf(content: &str) -> Vec<u8> {
        format!(
            "%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >> endobj\n4 0 obj << /Length {} >>\nstream\n{}\nendstream\nendobj\ntrailer << /Root 1 0 R >>\n%%EOF\n",
            content.len(),
            content
        )
        .into_bytes()
    }

    #[test]
    fn pdf_rectangle_in_red() {
        let p = import(&tiny_pdf("1 0 0 RG 0 0 72 72 re S")).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].layer, 2); // red layer
        let pts = &p[0].paths[0].pts;
        assert!((pts[1][0] - 25.4).abs() < 1e-9);
    }

    #[test]
    fn pdf_compressed_stream() {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(b"0 0 m 72 0 l 72 72 l h S").unwrap();
        let z = e.finish().unwrap();
        let mut f = b"%PDF-1.5\n1 0 obj << /Type /Pages /Kids [2 0 R] >> endobj\n2 0 obj << /Type /Page /Parent 1 0 R /MediaBox [0 0 100 100] /Contents 3 0 R >> endobj\n".to_vec();
        f.extend(format!("3 0 obj << /Length {} /Filter /FlateDecode >>\nstream\n", z.len()).as_bytes());
        f.extend(&z);
        f.extend(b"\nendstream\nendobj\n%%EOF\n");
        let p = import(&f).unwrap();
        assert!(p[0].paths[0].closed);
        assert_eq!(p[0].paths[0].pts.len(), 3);
    }

    #[test]
    fn legacy_ai_square() {
        let ai = b"%!PS-Adobe-3.0\n%%EndSetup\n0 0 0 1 K\n0 0 m\n72 0 l\n72 72 l\n0 72 l\ns\n";
        let p = import(ai).unwrap();
        assert!(p[0].paths[0].closed);
        assert_eq!(p[0].paths[0].pts.len(), 4);
    }
}

#[cfg(test)]
mod ai_sample {
    /// Imports the one file named by BEAMFORGE_AI_FILE and prints how long it took and what came out.
    /// `BEAMFORGE_AI_FILE=x.ai cargo test ai_sample -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn ai_sample() {
        let Ok(path) = std::env::var("BEAMFORGE_AI_FILE") else { return };
        let bytes = std::fs::read(&path).unwrap();
        let t = std::time::Instant::now();
        let r = super::import(&bytes);
        let ms = t.elapsed().as_millis();
        match r {
            Ok(p) => println!("RESULT OK {} pieces, {} paths, {} ms", p.len(), p.iter().map(|x| x.paths.len()).sum::<usize>(), ms),
            Err(e) => println!("RESULT ERR {e} ({ms} ms)"),
        }
    }
}
