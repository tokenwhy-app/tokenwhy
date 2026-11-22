// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

#[derive(Debug, Clone, Copy)]
pub enum Val<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed,
}

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut r: u64 = 0;
    for shift in (0..64).step_by(7) {
        let x = *b.get(*i)?;
        *i += 1;
        r |= ((x & 0x7f) as u64) << shift;
        if x < 0x80 {
            return Some(r);
        }
    }
    None
}

pub fn fields(b: &[u8]) -> Vec<(u32, Val<'_>)> {
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let Some(key) = varint(b, &mut i) else { break };
        let (num, wt) = ((key >> 3) as u32, key & 7);
        let v = match wt {
            0 => match varint(b, &mut i) {
                Some(v) => Val::Varint(v),
                None => break,
            },
            1 | 5 => {
                i += if wt == 1 { 8 } else { 4 };
                if i > b.len() {
                    break;
                }
                Val::Fixed
            }
            2 => {
                let Some(n) = varint(b, &mut i) else { break };
                let Some(end) = i.checked_add(n as usize).filter(|e| *e <= b.len()) else {
                    break;
                };
                let s = &b[i..end];
                i = end;
                Val::Bytes(s)
            }
            _ => break,
        };
        out.push((num, v));
    }
    out
}

pub fn bytes<'a>(f: &[(u32, Val<'a>)], num: u32) -> Option<&'a [u8]> {
    f.iter().find_map(|(n, v)| match v {
        Val::Bytes(b) if *n == num => Some(*b),
        _ => None,
    })
}

pub fn uint(f: &[(u32, Val<'_>)], num: u32) -> u64 {
    f.iter()
        .find_map(|(n, v)| match v {
            Val::Varint(x) if *n == num => Some(*x),
            _ => None,
        })
        .unwrap_or(0)
}

pub fn text<'a>(f: &[(u32, Val<'a>)], num: u32) -> Option<&'a str> {
    bytes(f, num)
        .and_then(|b| std::str::from_utf8(b).ok())
        .filter(|s| !s.is_empty())
}

pub fn packed(f: &[(u32, Val<'_>)], num: u32) -> Vec<u64> {
    let mut out = vec![];
    for (n, v) in f {
        match v {
            Val::Bytes(b) if *n == num => {
                let mut i = 0;
                while i < b.len() {
                    match varint(b, &mut i) {
                        Some(x) => out.push(x),
                        None => break,
                    }
                }
            }
            Val::Varint(x) if *n == num => out.push(*x),
            _ => {}
        }
    }
    out
}

pub fn time_ms(f: &[(u32, Val<'_>)], num: u32) -> Option<i64> {
    let (_, v) = f.iter().find(|(n, _)| *n == num)?;
    match v {
        Val::Varint(x) => {
            let x = *x as i64;
            if x < 1_000_000_000_000 {
                x.checked_mul(1000)
            } else {
                Some(x)
            }
            .filter(|m| *m > 0)
        }
        Val::Bytes(b) => {
            if let Some(ms) = std::str::from_utf8(b)
                .ok()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis())
            {
                return Some(ms);
            }
            let ts = fields(b);
            let secs = uint(&ts, 1) as i64;
            (secs > 0)
                .then(|| {
                    secs.checked_mul(1000)?
                        .checked_add(uint(&ts, 2) as i64 / 1_000_000)
                })
                .flatten()
        }
        Val::Fixed => None,
    }
}

#[cfg(test)]
pub mod enc {
    //! Encoder for test fixtures only.
    pub fn varint(mut x: u64, out: &mut Vec<u8>) {
        loop {
            let b = (x & 0x7f) as u8;
            x >>= 7;
            if x == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }
    pub fn uint(num: u32, x: u64, out: &mut Vec<u8>) {
        varint((num as u64) << 3, out);
        varint(x, out);
    }
    pub fn bytes(num: u32, b: &[u8], out: &mut Vec<u8>) {
        varint(((num as u64) << 3) | 2, out);
        varint(b.len() as u64, out);
        out.extend_from_slice(b);
    }
}
