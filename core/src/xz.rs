//! An XZ / LZMA2 decoder, standard library only. Older ZIM files (before 2021) keep their
//! articles in XZ-compressed clusters. Decoding only. Every length is checked, so damaged or
//! hostile input gives an error, never a crash or a huge allocation.

type R<T> = Result<T, String>;

fn err<T>(m: &str) -> R<T> {
    Err(format!("xz: {}", m))
}

// ---------- range decoder ----------

struct Rc<'a> {
    d: &'a [u8],
    at: usize,
    range: u32,
    code: u32,
}

impl<'a> Rc<'a> {
    fn new(d: &'a [u8]) -> R<Rc<'a>> {
        if d.len() < 5 || d[0] != 0 {
            return err("bad range coder start");
        }
        Ok(Rc { d, at: 5, range: 0xFFFF_FFFF, code: u32::from_be_bytes([d[1], d[2], d[3], d[4]]) })
    }
    fn norm(&mut self) -> R<()> {
        if self.range < (1 << 24) {
            let b = *self.d.get(self.at).ok_or("xz: compressed data ends too soon")?;
            self.at += 1;
            self.range <<= 8;
            self.code = (self.code << 8) | b as u32;
        }
        Ok(())
    }
    fn bit(&mut self, p: &mut u16) -> R<u32> {
        self.norm()?;
        let bound = (self.range >> 11) * (*p as u32);
        if self.code < bound {
            self.range = bound;
            *p += (2048 - *p) >> 5;
            Ok(0)
        } else {
            self.range -= bound;
            self.code -= bound;
            *p -= *p >> 5;
            Ok(1)
        }
    }
    fn direct(&mut self, n: u32) -> R<u32> {
        let mut r = 0u32;
        for _ in 0..n {
            self.norm()?;
            self.range >>= 1;
            let b = if self.code >= self.range {
                self.code -= self.range;
                1
            } else {
                0
            };
            r = (r << 1) | b;
        }
        Ok(r)
    }
    fn tree(&mut self, probs: &mut [u16], bits: u32) -> R<u32> {
        let mut m = 1usize;
        for _ in 0..bits {
            m = (m << 1) | self.bit(&mut probs[m])? as usize;
        }
        Ok(m as u32 - (1 << bits))
    }
    fn tree_rev(&mut self, probs: &mut [u16], bits: u32) -> R<u32> {
        let mut m = 1usize;
        let mut r = 0u32;
        for i in 0..bits {
            let b = self.bit(&mut probs[m])?;
            m = (m << 1) | b as usize;
            r |= b << i;
        }
        Ok(r)
    }
}

// ---------- LZMA ----------

struct Len {
    choice: u16,
    choice2: u16,
    low: [[u16; 8]; 16],
    mid: [[u16; 8]; 16],
    high: [u16; 256],
}

impl Len {
    fn new() -> Len {
        Len { choice: 1024, choice2: 1024, low: [[1024; 8]; 16], mid: [[1024; 8]; 16], high: [1024; 256] }
    }
    fn decode(&mut self, rc: &mut Rc, pos_state: usize) -> R<usize> {
        if rc.bit(&mut self.choice)? == 0 {
            return Ok(2 + rc.tree(&mut self.low[pos_state], 3)? as usize);
        }
        if rc.bit(&mut self.choice2)? == 0 {
            return Ok(10 + rc.tree(&mut self.mid[pos_state], 3)? as usize);
        }
        Ok(18 + rc.tree(&mut self.high, 8)? as usize)
    }
}

struct Lzma {
    lc: u32,
    lp: u32,
    pb: u32,
    state: usize,
    reps: [usize; 4],
    is_match: [[u16; 16]; 12],
    is_rep: [u16; 12],
    is_rep0: [u16; 12],
    is_rep1: [u16; 12],
    is_rep2: [u16; 12],
    is_rep0_long: [[u16; 16]; 12],
    slot: [[u16; 64]; 4],
    special: [u16; 116],
    align: [u16; 16],
    match_len: Len,
    rep_len: Len,
    lit: Vec<u16>,
}

impl Lzma {
    fn new(lc: u32, lp: u32, pb: u32) -> Lzma {
        Lzma {
            lc,
            lp,
            pb,
            state: 0,
            reps: [0; 4],
            is_match: [[1024; 16]; 12],
            is_rep: [1024; 12],
            is_rep0: [1024; 12],
            is_rep1: [1024; 12],
            is_rep2: [1024; 12],
            is_rep0_long: [[1024; 16]; 12],
            slot: [[1024; 64]; 4],
            special: [1024; 116],
            align: [1024; 16],
            match_len: Len::new(),
            rep_len: Len::new(),
            lit: vec![1024; 0x300 << (lc + lp)],
        }
    }
    fn reset(&mut self, lc: u32, lp: u32, pb: u32) {
        *self = Lzma::new(lc, lp, pb);
    }

    /// Decode exactly `want` bytes into `out`. `base` is where this dictionary started in `out`.
    fn run(&mut self, rc: &mut Rc, out: &mut Vec<u8>, base: usize, want: usize, limit: usize) -> R<()> {
        let end = out.len() + want;
        if end > limit {
            return err("output is larger than allowed");
        }
        let pb_mask = (1usize << self.pb) - 1;
        let lp_mask = (1usize << self.lp) - 1;
        while out.len() < end {
            let pos = out.len() - base;
            let ps = pos & pb_mask;
            let st = self.state;
            if rc.bit(&mut self.is_match[st][ps])? == 0 {
                // a literal
                let prev = if pos > 0 { out[out.len() - 1] as usize } else { 0 };
                let ctx = ((pos & lp_mask) << self.lc) + (prev >> (8 - self.lc));
                let probs = &mut self.lit[0x300 * ctx..0x300 * ctx + 0x300];
                let mut sym = 1usize;
                if st >= 7 {
                    let dist = self.reps[0] + 1;
                    if dist > pos {
                        return err("match reaches before the start");
                    }
                    let mut mb = out[out.len() - dist] as usize;
                    let mut off = 0x100usize;
                    while sym < 0x100 {
                        mb <<= 1;
                        let mbit = mb & off;
                        let b = rc.bit(&mut probs[off + mbit + sym])? as usize;
                        sym = (sym << 1) | b;
                        if b == 1 {
                            off &= mbit;
                        } else {
                            off &= !mbit;
                        }
                    }
                } else {
                    while sym < 0x100 {
                        sym = (sym << 1) | rc.bit(&mut probs[sym])? as usize;
                    }
                }
                out.push(sym as u8);
                self.state = if st < 4 { 0 } else if st < 10 { st - 3 } else { st - 6 };
                continue;
            }
            let len;
            if rc.bit(&mut self.is_rep[st])? == 0 {
                // a new distance
                len = self.match_len.decode(rc, ps)?;
                self.state = if st < 7 { 7 } else { 10 };
                let ls = (len - 2).min(3);
                let slot = rc.tree(&mut self.slot[ls], 6)?;
                let dist: u32;
                if slot < 4 {
                    dist = slot;
                } else {
                    let nb = (slot >> 1) - 1;
                    let mut d = (2 | (slot & 1)) << nb;
                    if slot < 14 {
                        // the table is shifted by one so the first slot's start is not negative
                        let base_i = (d - slot) as usize;
                        d += rc.tree_rev(&mut self.special[base_i..], nb)?;
                    } else {
                        d += rc.direct(nb - 4)? << 4;
                        d += rc.tree_rev(&mut self.align, 4)?;
                    }
                    dist = d;
                }
                if dist == 0xFFFF_FFFF {
                    return err("unexpected end marker");
                }
                self.reps = [dist as usize, self.reps[0], self.reps[1], self.reps[2]];
            } else {
                if pos == 0 {
                    return err("repeat before any data");
                }
                if rc.bit(&mut self.is_rep0[st])? == 0 {
                    if rc.bit(&mut self.is_rep0_long[st][ps])? == 0 {
                        // a one-byte repeat
                        self.state = if st < 7 { 9 } else { 11 };
                        let dist = self.reps[0] + 1;
                        if dist > pos {
                            return err("match reaches before the start");
                        }
                        let b = out[out.len() - dist];
                        out.push(b);
                        continue;
                    }
                } else {
                    let d;
                    if rc.bit(&mut self.is_rep1[st])? == 0 {
                        d = self.reps[1];
                    } else {
                        if rc.bit(&mut self.is_rep2[st])? == 0 {
                            d = self.reps[2];
                        } else {
                            d = self.reps[3];
                            self.reps[3] = self.reps[2];
                        }
                        self.reps[2] = self.reps[1];
                    }
                    self.reps[1] = self.reps[0];
                    self.reps[0] = d;
                }
                len = self.rep_len.decode(rc, ps)?;
                self.state = if st < 7 { 8 } else { 11 };
            }
            let dist = self.reps[0] + 1;
            if dist > pos {
                return err("match reaches before the start");
            }
            if out.len() + len > end {
                return err("match runs past the end of its chunk");
            }
            let start = out.len() - dist;
            if dist >= len {
                out.extend_from_within(start..start + len);
            } else {
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
        }
        Ok(())
    }
}

fn props(b: u8) -> R<(u32, u32, u32)> {
    if b >= 225 {
        return err("bad LZMA properties");
    }
    let b = b as u32;
    let (lc, lp, pb) = (b % 9, (b / 9) % 5, b / 45);
    if lc + lp > 4 {
        return err("unsupported LZMA properties");
    }
    Ok((lc, lp, pb))
}

fn lzma2(d: &[u8], out: &mut Vec<u8>, limit: usize) -> R<usize> {
    let mut at = 0usize;
    let mut lz: Option<Lzma> = None;
    let mut base = out.len();
    let mut need_props = true;
    let mut need_dict_reset = true;
    loop {
        let c = *d.get(at).ok_or("xz: LZMA2 data ends too soon")?;
        at += 1;
        if c == 0 {
            return Ok(at);
        }
        if c == 1 || c == 2 {
            if c == 1 {
                base = out.len();
                need_dict_reset = false;
            } else if need_dict_reset {
                return err("data before a dictionary reset");
            }
            let sz = d.get(at..at + 2).ok_or("xz: short chunk")?;
            let n = u16::from_be_bytes([sz[0], sz[1]]) as usize + 1;
            at += 2;
            let b = d.get(at..at + n).ok_or("xz: stored chunk runs past the end")?;
            if out.len() + n > limit {
                return err("output is larger than allowed");
            }
            out.extend_from_slice(b);
            at += n;
            continue;
        }
        if c < 0x80 {
            return err("bad chunk type");
        }
        let hdr = d.get(at..at + 4).ok_or("xz: short chunk")?;
        let unpacked = (((c & 0x1F) as usize) << 16) + ((hdr[0] as usize) << 8) + hdr[1] as usize + 1;
        let packed = ((hdr[2] as usize) << 8) + hdr[3] as usize + 1;
        at += 4;
        let mode = (c >> 5) & 3;
        if mode == 3 {
            base = out.len();
            need_dict_reset = false;
        } else if need_dict_reset {
            return err("data before a dictionary reset");
        }
        if mode >= 2 {
            let p = *d.get(at).ok_or("xz: short chunk")?;
            at += 1;
            let (lc, lp, pb) = props(p)?;
            match lz.as_mut() {
                Some(l) => l.reset(lc, lp, pb),
                None => lz = Some(Lzma::new(lc, lp, pb)),
            }
            need_props = false;
        } else if need_props {
            return err("missing LZMA properties");
        } else if mode == 1 {
            let l = lz.as_mut().ok_or("xz: missing LZMA state")?;
            let (lc, lp, pb) = (l.lc, l.lp, l.pb);
            l.reset(lc, lp, pb);
        }
        let body = d.get(at..at + packed).ok_or("xz: chunk runs past the end")?;
        let mut rc = Rc::new(body)?;
        lz.as_mut().ok_or("xz: missing LZMA state")?.run(&mut rc, out, base, unpacked, limit)?;
        at += packed;
    }
}

// ---------- the .xz container ----------

fn varint(d: &[u8], at: &mut usize) -> R<u64> {
    let mut v = 0u64;
    for i in 0..9 {
        let b = *d.get(*at).ok_or("xz: short header")?;
        *at += 1;
        v |= ((b & 0x7F) as u64) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    err("bad number in header")
}

fn check_size(kind: u8) -> R<usize> {
    Ok(match kind {
        0 => 0,
        1 => 4,
        4 => 8,
        10 => 32,
        _ => return err("unsupported integrity check"),
    })
}

/// Decompress an `.xz` stream (or several). `max_out` caps the result.
pub fn decompress(input: &[u8], max_out: usize) -> R<Vec<u8>> {
    const MAGIC: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0];
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < input.len() {
        if input.len() - at < 12 || input[at..at + 6] != MAGIC {
            // padding of zeros between streams is allowed
            if input[at..].iter().all(|b| *b == 0) {
                break;
            }
            return err("not an xz stream");
        }
        let csize = check_size(input[at + 7] & 0x0F)?;
        at += 12;
        loop {
            let first = *input.get(at).ok_or("xz: stream ends too soon")?;
            if first == 0 {
                break; // the index starts here
            }
            let hlen = (first as usize + 1) * 4;
            let h = input.get(at..at + hlen).ok_or("xz: short block header")?;
            let flags = h[1];
            let mut p = 2usize;
            if flags & 0x40 != 0 {
                varint(h, &mut p)?;
            }
            if flags & 0x80 != 0 {
                varint(h, &mut p)?;
            }
            let nfilters = (flags & 3) as usize + 1;
            if nfilters != 1 {
                return err("only plain LZMA2 blocks are supported");
            }
            let id = varint(h, &mut p)?;
            let psize = varint(h, &mut p)? as usize;
            if id != 0x21 || psize != 1 {
                return err("only LZMA2 is supported");
            }
            at += hlen;
            let used = lzma2(&input[at..], &mut out, max_out)?;
            at += used;
            // padding to a multiple of four, then the check
            let total = hlen + used;
            at += (4 - total % 4) % 4 + csize;
            if at > input.len() {
                return err("stream ends too soon");
            }
        }
        // The index: 0x00, a count, two numbers per block, zero padding to a multiple of four
        // (counted from the 0x00), then a CRC32. After it comes the 12-byte stream footer.
        let idx_start = at;
        at += 1;
        let count = varint(input, &mut at)?;
        for _ in 0..count {
            varint(input, &mut at)?;
            varint(input, &mut at)?;
        }
        at = idx_start + (at - idx_start).div_ceil(4) * 4 + 4 + 12;
        if at > input.len() || &input[at - 2..at] != b"YZ" {
            return err("missing stream footer");
        }
    }
    Ok(out)
}
