//! Ruida controller files: .rd job files (RDTranslator::Import 0x1000b35c4) and .rld projects
//! (RLDTranslator::Import 0x1000b4314).

use super::Piece;
use crate::i18n::tr;
use crate::model::nearest_layer;
use std::collections::HashMap;

/// Default scramble key (Obfuscator_Ruida ctor 0x100a0c778 stores 0x88); files carry their own,
/// found from the first byte.
#[allow(dead_code)]
pub const RD_MAGIC: u8 = 0x88;
/// Ruida coordinates are micrometres (÷1000, constant 0x447a0000 in RDTranslator::Import).
pub const RD_UNIT_MM: f64 = 0.001;

/// Obfuscator_Ruida::Decode 0x100a0c800: (b − 1) xor key, then bits 0 and 7 swapped.
pub fn decode(b: u8, key: u8) -> u8 {
    let x = b.wrapping_sub(1) ^ key;
    (x & 0x7e) | (x >> 7) | (x << 7)
}

/// Obfuscator_Ruida::FindMagic 0x100a0c7c8: the key that turns the first byte into 0xD8
/// (255 when none does).
pub fn find_key(first: u8) -> u8 {
    (0..=255u8).find(|k| decode(first, *k) == 0xd8).unwrap_or(255)
}

struct Reader {
    b: Vec<u8>,
    i: usize,
}

impl Reader {
    fn next(&mut self) -> Option<u8> {
        let v = self.b.get(self.i).copied();
        self.i += 1;
        v
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    /// Messages::ReadLong 0x1009898f4: five 7-bit groups, most significant first.
    fn long(&mut self) -> i32 {
        let mut v: u32 = 0;
        for k in 0..5 {
            let byte = self.b.get(self.i + k).copied().unwrap_or(0) as u32;
            v = (v << 7) | (byte & 0x7f);
        }
        self.i += 5;
        v as i32
    }
    /// Messages::ReadShort 0x100989968: two 7-bit groups, negative when bit 6 of the first is set.
    fn short(&mut self) -> i32 {
        let hi = self.b.get(self.i).copied().unwrap_or(0) as i32;
        let lo = self.b.get(self.i + 1).copied().unwrap_or(0) as i32;
        self.i += 2;
        let mut v = (hi << 7) | lo;
        if hi & 0x40 != 0 {
            v |= 0xe000;
        }
        v as i16 as i32
    }
}

pub fn rd(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    if bytes.is_empty() {
        return Err(tr("Il file RD è vuoto.").into());
    }
    let key = find_key(bytes[0]);
    let mut r = Reader { b: bytes.iter().map(|b| decode(*b, key)).collect(), i: 0 };
    let mut layer_color: HashMap<u8, u32> = HashMap::new();
    let mut cur_layer: u8 = 0;
    let mut pos = [0i32; 2];
    let mut pieces: Vec<Piece> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut cur_l = 0usize;
    let to_mm = |p: [i32; 2]| [p[0] as f64 * RD_UNIT_MM, -(p[1] as f64) * RD_UNIT_MM];
    let layer_of = |l: u8, map: &HashMap<u8, u32>| -> usize {
        match map.get(&l) {
            Some(c) => nearest_layer((c & 0xff) as u8, ((c >> 8) & 0xff) as u8, ((c >> 16) & 0xff) as u8),
            None => (l as usize).min(29),
        }
    };
    while let Some(cmd) = r.next() {
        if cmd < 0x80 {
            continue;
        }
        let (dx, dy, abs) = match cmd {
            0x88 | 0xa8 => {
                let x = r.long();
                let y = r.long();
                (x, y, true)
            }
            0x89 | 0xa9 => {
                let x = r.short();
                let y = r.short();
                (x, y, false)
            }
            0x8a | 0xaa => (r.short(), 0, false),
            0x8b | 0xab => (0, r.short(), false),
            0xca => {
                match r.peek() {
                    Some(6) => {
                        r.next();
                        let l = r.next().unwrap_or(0);
                        let c = r.long() as u32;
                        layer_color.insert(l, c);
                    }
                    Some(2) => {
                        r.next();
                        cur_layer = r.next().unwrap_or(0);
                    }
                    _ => {}
                }
                skip_data(&mut r);
                continue;
            }
            _ => {
                skip_data(&mut r);
                continue;
            }
        };
        let next = if abs { [dx, dy] } else { [pos[0] + dx, pos[1] + dy] };
        if cmd & 0xf0 == 0x80 {
            // a move starts a new shape (StartNewShape 0x1000b32a4) on the current layer
            if cur.len() > 1 {
                pieces.push(Piece { layer: cur_l, paths: vec![super::finish(std::mem::take(&mut cur), 1e-6)] });
            }
            cur.clear();
            cur_l = layer_of(cur_layer, &layer_color);
        } else {
            if cur.is_empty() {
                cur.push(to_mm(pos));
                cur_l = layer_of(cur_layer, &layer_color);
            }
            cur.push(to_mm(next));
        }
        pos = next;
    }
    if cur.len() > 1 {
        pieces.push(Piece { layer: cur_l, paths: vec![super::finish(cur, 1e-6)] });
    }
    if pieces.is_empty() {
        return Err(tr("Il file RD non contiene tagli.").into());
    }
    Ok(pieces)
}

fn skip_data(r: &mut Reader) {
    while let Some(b) = r.peek() {
        if b >= 0x80 {
            break;
        }
        r.i += 1;
    }
}

/// The original reads an .rld project's header and settings but builds no shapes from it
/// (RLDTranslator::Import 0x1000b4314 returns an empty group, which it discards).
pub fn rld(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    if bytes.len() < 8 {
        return Err(tr("Il file RLD è vuoto o danneggiato.").into());
    }
    Err(tr("Il file RLD (progetto Ruida) è stato letto, ma non contiene forme importabili: come nel programma originale, se ne legge solo l'intestazione. Esporta il disegno come .rd per importarlo.").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(b: u8, key: u8) -> u8 {
        let s = (b & 0x7e) | (b >> 7) | (b << 7);
        (s ^ key).wrapping_add(1)
    }

    #[test]
    fn decode_inverts_encode() {
        for b in 0..=255u8 {
            assert_eq!(decode(encode(b, RD_MAGIC), RD_MAGIC), b);
        }
        assert_eq!(find_key(encode(0xd8, RD_MAGIC)), RD_MAGIC);
    }

    #[test]
    fn rd_square() {
        let long = |v: i32| -> Vec<u8> { (0..5).rev().map(|k| ((v as u32 >> (7 * k)) & 0x7f) as u8).collect() };
        let mut plain = vec![0xd8, 0x00];
        for (c, x, y) in [(0x88u8, 0, 0), (0xa8, 10000, 0), (0xa8, 10000, 10000), (0xa8, 0, 0)] {
            plain.push(c);
            plain.extend(long(x));
            plain.extend(long(y));
        }
        let enc: Vec<u8> = plain.iter().map(|b| encode(*b, RD_MAGIC)).collect();
        let p = rd(&enc).unwrap();
        assert_eq!(p.len(), 1);
        assert!((p[0].paths[0].pts[1][0] - 10.0).abs() < 1e-9);
    }
}
