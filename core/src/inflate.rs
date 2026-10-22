//! DEFLATE and gzip decoding (RFC 1951 and 1952), standard library only. Map files keep their
//! tiles and tables gzip-compressed, so reading roads out of them needs this. Decoding only.
//! Every length is checked, so a damaged file gives an error, never a crash or a huge allocation.

type R<T> = Result<T, String>;

fn err<T>(m: &str) -> R<T> {
    Err(format!("gzip: {}", m))
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    buf: u32,
    n: u32,
}

impl<'a> Bits<'a> {
    fn need(&mut self, k: u32) -> R<()> {
        while self.n < k {
            let Some(&b) = self.d.get(self.pos) else { return err("ran out of data") };
            self.pos += 1;
            self.buf |= (b as u32) << self.n;
            self.n += 8;
        }
        Ok(())
    }
    fn bits(&mut self, k: u32) -> R<u32> {
        if k == 0 {
            return Ok(0);
        }
        self.need(k)?;
        let v = self.buf & ((1u32 << k) - 1);
        self.buf >>= k;
        self.n -= k;
        Ok(v)
    }
}

/// A canonical Huffman table read from code lengths.
struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> R<Huff> {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut left: i32 = 1;
        for len in 1..16 {
            left <<= 1;
            left -= count[len] as i32;
            if left < 0 {
                return err("over-subscribed code");
            }
        }
        let mut offs = [0u16; 16];
        for len in 1..15 {
            offs[len + 1] = offs[len] + count[len];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huff { count, symbol })
    }
    fn decode(&self, b: &mut Bits) -> R<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.bits(1)? as i32;
            let c = self.count[len] as i32;
            if code - c < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += c;
            first += c;
            first <<= 1;
            code <<= 1;
        }
        err("bad code")
    }
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn codes(b: &mut Bits, out: &mut Vec<u8>, lit: &Huff, dist: &Huff, max_out: usize) -> R<()> {
    loop {
        let s = lit.decode(b)? as usize;
        if s < 256 {
            if out.len() >= max_out {
                return err("result too large");
            }
            out.push(s as u8);
        } else if s == 256 {
            return Ok(());
        } else {
            let s = s - 257;
            if s >= 29 {
                return err("bad length");
            }
            let len = LBASE[s] as usize + b.bits(LEXT[s] as u32)? as usize;
            let d = dist.decode(b)? as usize;
            if d >= 30 {
                return err("bad distance");
            }
            let dist_back = DBASE[d] as usize + b.bits(DEXT[d] as u32)? as usize;
            if dist_back > out.len() {
                return err("distance too far back");
            }
            if out.len() + len > max_out {
                return err("result too large");
            }
            for _ in 0..len {
                let c = out[out.len() - dist_back];
                out.push(c);
            }
        }
    }
}

/// Decode a raw DEFLATE stream. `max_out` caps the result.
pub fn inflate(data: &[u8], max_out: usize) -> R<Vec<u8>> {
    let mut b = Bits { d: data, pos: 0, buf: 0, n: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.bits(1)?;
        match b.bits(2)? {
            0 => {
                b.buf = 0;
                b.n = 0;
                let p = b.pos;
                if p + 4 > data.len() {
                    return err("short stored block");
                }
                let len = u16::from_le_bytes([data[p], data[p + 1]]) as usize;
                let nlen = u16::from_le_bytes([data[p + 2], data[p + 3]]) as usize;
                if len != (!nlen & 0xFFFF) {
                    return err("stored block length does not match");
                }
                if p + 4 + len > data.len() {
                    return err("short stored block");
                }
                if out.len() + len > max_out {
                    return err("result too large");
                }
                out.extend_from_slice(&data[p + 4..p + 4 + len]);
                b.pos = p + 4 + len;
            }
            1 => {
                let mut l = [0u8; 288];
                for (i, x) in l.iter_mut().enumerate() {
                    *x = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
                }
                let lit = Huff::new(&l)?;
                let dist = Huff::new(&[5u8; 30])?;
                codes(&mut b, &mut out, &lit, &dist, max_out)?;
            }
            2 => {
                let nlen = b.bits(5)? as usize + 257;
                let ndist = b.bits(5)? as usize + 1;
                let ncode = b.bits(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return err("too many codes");
                }
                const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl[o] = b.bits(3)? as u8;
                }
                let clh = Huff::new(&cl)?;
                let mut lens = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let s = clh.decode(&mut b)?;
                    if s < 16 {
                        lens[i] = s as u8;
                        i += 1;
                    } else {
                        let (prev, rep) = match s {
                            16 => {
                                if i == 0 {
                                    return err("repeat with nothing before");
                                }
                                (lens[i - 1], 3 + b.bits(2)? as usize)
                            }
                            17 => (0, 3 + b.bits(3)? as usize),
                            _ => (0, 11 + b.bits(7)? as usize),
                        };
                        if i + rep > nlen + ndist {
                            return err("repeat too long");
                        }
                        for _ in 0..rep {
                            lens[i] = prev;
                            i += 1;
                        }
                    }
                }
                if lens[256] == 0 {
                    return err("no end code");
                }
                let lit = Huff::new(&lens[..nlen])?;
                let dist = Huff::new(&lens[nlen..])?;
                codes(&mut b, &mut out, &lit, &dist, max_out)?;
            }
            _ => return err("bad block"),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

/// Decode a gzip file (one member). `max_out` caps the result.
pub fn gunzip(data: &[u8], max_out: usize) -> R<Vec<u8>> {
    if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return err("not a gzip file");
    }
    let flg = data[3];
    let mut p = 10usize;
    if flg & 4 != 0 {
        if p + 2 > data.len() {
            return err("short header");
        }
        p += 2 + u16::from_le_bytes([data[p], data[p + 1]]) as usize;
    }
    for bit in [8u8, 16] {
        if flg & bit != 0 {
            while p < data.len() && data[p] != 0 {
                p += 1;
            }
            p += 1;
        }
    }
    if flg & 2 != 0 {
        p += 2;
    }
    if p >= data.len() {
        return err("short header");
    }
    inflate(&data[p..], max_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_block_comes_back_as_it_was() {
        // final stored block holding "hello"
        let d = [1u8, 5, 0, 0xFA, 0xFF, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(inflate(&d, 100).unwrap(), b"hello");
    }

    #[test]
    fn gzip_round_trip() {
        // Needs the gzip program (every Linux and Mac has it). Skipped when it is missing.
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut text = Vec::new();
        for i in 0..3000 {
            text.extend_from_slice(format!("road {} kind minor_road detail residential {}\n", i % 97, i * 31).as_bytes());
        }
        let Ok(mut child) = Command::new("gzip").arg("-9").arg("-c").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() else { return };
        let mut stdin = child.stdin.take().unwrap();
        let t = text.clone();
        let h = std::thread::spawn(move || {
            let _ = stdin.write_all(&t);
        });
        let out = child.wait_with_output().unwrap();
        h.join().unwrap();
        assert!(out.stdout.len() < text.len() / 2);
        assert_eq!(gunzip(&out.stdout, 10_000_000).unwrap(), text);
        // the cap holds
        assert!(gunzip(&out.stdout, 100).is_err());
    }

    #[test]
    fn damaged_data_is_an_error_not_a_crash() {
        assert!(gunzip(&[1, 2, 3], 10).is_err());
        assert!(inflate(&[0xFF, 0xFF, 0xFF], 10).is_err());
        assert!(inflate(&[], 10).is_err());
    }
}
