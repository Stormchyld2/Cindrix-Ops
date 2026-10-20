//! A Zstandard decoder (RFC 8878), standard library only. Modern ZIM files (every Kiwix
//! download since 2021) keep their articles in Zstandard-compressed clusters, so reading them
//! without Kiwix's program needs this. Decoding only; dictionaries are not supported (ZIM does
//! not use them). Every length is checked, so a damaged or hostile file gives an error, never a
//! crash or a huge allocation.

const MAGIC: u32 = 0xFD2F_B528;
const MAX_BLOCK: usize = 128 * 1024;

type R<T> = Result<T, String>;

fn err<T>(m: &str) -> R<T> {
    Err(format!("zstd: {}", m))
}

// ---------- bits ----------

/// Reads bits from the end of a byte slice towards its start (how Zstandard stores its
/// entropy-coded streams). The last byte holds a marker: its highest set bit is the start.
struct Back<'a> {
    d: &'a [u8],
    /// Bits still to read.
    pos: usize,
    /// Bits asked for beyond the start of the stream.
    over: usize,
}

impl<'a> Back<'a> {
    fn new(d: &'a [u8]) -> R<Back<'a>> {
        let Some(&last) = d.last() else { return err("empty stream") };
        if last == 0 {
            return err("stream has no end marker");
        }
        let pos = d.len() * 8 - 1 - last.leading_zeros() as usize;
        Ok(Back { d, pos, over: 0 })
    }
    /// Look at the next `n` (at most 32) bits without using them.
    fn peek(&self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let n = n as usize;
        if self.pos >= n {
            self.window(self.pos - n, n)
        } else {
            self.window(0, self.pos) << (n - self.pos)
        }
    }
    /// Bits [start, start+n) as a number, bit `start` being the lowest.
    fn window(&self, start: usize, n: usize) -> u32 {
        if n == 0 {
            return 0;
        }
        let byte = start / 8;
        let mut w = 0u64;
        for k in 0..8 {
            if let Some(&b) = self.d.get(byte + k) {
                w |= (b as u64) << (8 * k);
            }
        }
        ((w >> (start % 8)) & ((1u64 << n) - 1)) as u32
    }
    fn skip(&mut self, n: u32) {
        let n = n as usize;
        if self.pos >= n {
            self.pos -= n;
        } else {
            self.over += n - self.pos;
            self.pos = 0;
        }
    }
    fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.skip(n);
        v
    }
}

/// Reads bits from the start of a slice (used for FSE table descriptions).
struct Fwd<'a> {
    d: &'a [u8],
    bit: usize,
}

impl<'a> Fwd<'a> {
    fn read(&mut self, n: u32) -> R<u32> {
        let mut v = 0u32;
        for i in 0..n {
            let p = self.bit + i as usize;
            let b = *self.d.get(p / 8).ok_or("zstd: table description runs past its end")?;
            v |= (((b >> (p % 8)) & 1) as u32) << i;
        }
        self.bit += n as usize;
        Ok(v)
    }
    fn bytes_used(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

// ---------- FSE ----------

#[derive(Clone, Copy, Default)]
struct Cell {
    symbol: u8,
    bits: u8,
    base: u16,
}

#[derive(Clone, Default)]
struct Fse {
    log: u32,
    cells: Vec<Cell>,
}

fn high_bit(v: u32) -> u32 {
    31 - v.leading_zeros()
}

impl Fse {
    fn build(freq: &[i16], log: u32) -> R<Fse> {
        let size = 1usize << log;
        let total: i32 = freq.iter().map(|&f| if f < 0 { 1 } else { f as i32 }).sum();
        if total as usize != size {
            return err("table probabilities do not add up");
        }
        let mut cells = vec![Cell::default(); size];
        let mut next = vec![0u32; freq.len()];
        let mut high = size;
        for (s, &f) in freq.iter().enumerate() {
            if f == -1 {
                high -= 1;
                cells[high].symbol = s as u8;
                next[s] = 1;
            } else {
                next[s] = f as u32;
            }
        }
        let step = (size >> 1) + (size >> 3) + 3;
        let mask = size - 1;
        let mut pos = 0usize;
        for (s, &f) in freq.iter().enumerate() {
            for _ in 0..f.max(0) {
                cells[pos].symbol = s as u8;
                loop {
                    pos = (pos + step) & mask;
                    if pos < high {
                        break;
                    }
                }
            }
        }
        for c in cells.iter_mut() {
            let s = c.symbol as usize;
            let n = next[s];
            next[s] += 1;
            let bits = log - high_bit(n);
            c.bits = bits as u8;
            c.base = ((n << bits) as usize - size) as u16;
        }
        Ok(Fse { log, cells })
    }

    /// Read a table description from the start of `d`; returns the table and the bytes used.
    fn read(d: &[u8], max_log: u32, max_sym: usize) -> R<(Fse, usize)> {
        let mut r = Fwd { d, bit: 0 };
        let log = 5 + r.read(4)?;
        if log > max_log {
            return err("table is larger than allowed");
        }
        let mut remaining: i32 = 1 << log;
        let mut freq: Vec<i16> = Vec::new();
        while remaining > 0 && freq.len() <= max_sym {
            let bits = high_bit((remaining + 1) as u32) + 1;
            let mut val = r.read(bits)? as i32;
            let low_mask = (1i32 << (bits - 1)) - 1;
            let threshold = (1i32 << bits) - 1 - (remaining + 1);
            if (val & low_mask) < threshold {
                r.bit -= 1;
                val &= low_mask;
            } else if val > low_mask {
                val -= threshold;
            }
            let p = val - 1;
            freq.push(p as i16);
            remaining -= p.abs();
            if p == 0 {
                loop {
                    let rep = r.read(2)?;
                    for _ in 0..rep {
                        if freq.len() > max_sym {
                            return err("too many symbols");
                        }
                        freq.push(0);
                    }
                    if rep != 3 {
                        break;
                    }
                }
            }
        }
        if remaining != 0 || freq.len() > max_sym + 1 {
            return err("bad table description");
        }
        let used = r.bytes_used();
        Ok((Fse::build(&freq, log)?, used))
    }

    fn rle(symbol: u8) -> Fse {
        Fse { log: 0, cells: vec![Cell { symbol, bits: 0, base: 0 }] }
    }
}

// ---------- Huffman ----------

#[derive(Clone, Default)]
struct Huff {
    bits: u32,
    /// (symbol, number of bits) for each `bits`-bit prefix.
    table: Vec<(u8, u8)>,
}

impl Huff {
    fn from_weights(weights: &[u8]) -> R<Huff> {
        if weights.is_empty() || weights.len() > 255 {
            return err("bad number of weights");
        }
        let mut total: u32 = 0;
        for &w in weights {
            if w > 11 {
                return err("weight too large");
            }
            if w > 0 {
                total += 1 << (w - 1);
            }
        }
        if total == 0 {
            return err("no weights");
        }
        let max_bits = high_bit(total) + 1;
        if max_bits > 11 {
            return err("codes too long");
        }
        let left = (1u32 << max_bits) - total;
        if !left.is_power_of_two() {
            return err("weights do not fill the code space");
        }
        let last = high_bit(left) + 1;
        let mut all: Vec<u8> = weights.to_vec();
        all.push(last as u8);
        let mut table = vec![(0u8, 0u8); 1usize << max_bits];
        let mut idx = 0usize;
        for w in 1..=max_bits as u8 {
            for (s, &sw) in all.iter().enumerate() {
                if sw == w {
                    let n = 1usize << (w - 1);
                    let bits = (max_bits + 1 - w as u32) as u8;
                    for e in table.iter_mut().skip(idx).take(n) {
                        *e = (s as u8, bits);
                    }
                    idx += n;
                }
            }
        }
        if idx != table.len() {
            return err("code space not filled");
        }
        Ok(Huff { bits: max_bits, table })
    }

    /// Read a tree description from `d`; returns the tree and the bytes used.
    fn read(d: &[u8]) -> R<(Huff, usize)> {
        let Some(&h) = d.first() else { return err("missing tree") };
        let mut weights = Vec::new();
        let used;
        if h >= 128 {
            let n = h as usize - 127;
            let bytes = n.div_ceil(2);
            let body = d.get(1..1 + bytes).ok_or("zstd: tree runs past its end")?;
            for i in 0..n {
                let b = body[i / 2];
                weights.push(if i % 2 == 0 { b >> 4 } else { b & 15 });
            }
            used = 1 + bytes;
        } else {
            let size = h as usize;
            let body = d.get(1..1 + size).ok_or("zstd: tree runs past its end")?;
            let (fse, hdr) = Fse::read(body, 6, 255)?;
            let mut bs = Back::new(body.get(hdr..).ok_or("zstd: bad tree")?)?;
            let mut s1 = bs.read(fse.log) as usize;
            let mut s2 = bs.read(fse.log) as usize;
            loop {
                if weights.len() > 255 {
                    return err("too many weights");
                }
                let c1 = fse.cells[s1];
                weights.push(c1.symbol);
                s1 = c1.base as usize + bs.read(c1.bits as u32) as usize;
                if bs.over > 0 {
                    weights.push(fse.cells[s2].symbol);
                    break;
                }
                let c2 = fse.cells[s2];
                weights.push(c2.symbol);
                s2 = c2.base as usize + bs.read(c2.bits as u32) as usize;
                if bs.over > 0 {
                    weights.push(fse.cells[s1.min(fse.cells.len() - 1)].symbol);
                    break;
                }
                if s1 >= fse.cells.len() || s2 >= fse.cells.len() {
                    return err("state out of range");
                }
            }
            used = 1 + size;
        }
        Ok((Huff::from_weights(&weights)?, used))
    }

    /// Decode `count` bytes from one backward stream.
    fn decode(&self, stream: &[u8], count: usize, out: &mut Vec<u8>) -> R<()> {
        let mut bs = Back::new(stream)?;
        for _ in 0..count {
            let (sym, n) = self.table[bs.peek(self.bits) as usize];
            bs.skip(n as u32);
            out.push(sym);
        }
        if bs.over > 0 || bs.pos != 0 {
            return err("literals stream was not used exactly");
        }
        Ok(())
    }
}

// ---------- tables for sequences ----------

const LL_BASE: [u32; 36] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32, 40, 48, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];
const LL_BITS: [u8; 36] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
const ML_BASE: [u32; 53] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 37, 39, 41, 43, 47, 51, 59, 67, 83, 99, 131, 259, 515, 1027, 2051, 4099, 8195, 16387, 32771, 65539,
];
const ML_BITS: [u8; 53] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

const LL_DEFAULT: [i16; 36] = [4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1, -1, -1, -1, -1];
const ML_DEFAULT: [i16; 53] = [1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1];
const OF_DEFAULT: [i16; 29] = [1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1];

// ---------- the decoder ----------

struct State {
    huff: Option<Huff>,
    ll: Option<Fse>,
    of: Option<Fse>,
    ml: Option<Fse>,
    rep: [usize; 3],
}

fn literals(d: &[u8], st: &mut State) -> R<(Vec<u8>, usize)> {
    let b0 = *d.first().ok_or("zstd: empty block")?;
    let kind = b0 & 3;
    let fmt = (b0 >> 2) & 3;
    match kind {
        0 | 1 => {
            let (size, hdr) = match fmt {
                0 | 2 => ((b0 >> 3) as usize, 1),
                1 => (((d.get(1).copied().ok_or("zstd: short block")? as usize) << 4) | (b0 >> 4) as usize, 2),
                _ => {
                    let (b1, b2) = (d.get(1).copied().ok_or("zstd: short block")? as usize, d.get(2).copied().ok_or("zstd: short block")? as usize);
                    ((b2 << 12) | (b1 << 4) | (b0 >> 4) as usize, 3)
                }
            };
            if size > MAX_BLOCK {
                return err("literals too large");
            }
            if kind == 0 {
                let body = d.get(hdr..hdr + size).ok_or("zstd: literals run past the block")?;
                Ok((body.to_vec(), hdr + size))
            } else {
                let v = *d.get(hdr).ok_or("zstd: short block")?;
                Ok((vec![v; size], hdr + 1))
            }
        }
        _ => {
            let hbytes = match fmt { 0 | 1 => 3, 2 => 4, _ => 5 };
            let h = d.get(..hbytes).ok_or("zstd: short block")?;
            let mut v: u64 = 0;
            for (i, b) in h.iter().enumerate() {
                v |= (*b as u64) << (8 * i);
            }
            let v = v >> 4;
            let (regen, comp, streams) = match fmt {
                0 => ((v & 0x3FF) as usize, ((v >> 10) & 0x3FF) as usize, 1),
                1 => ((v & 0x3FF) as usize, ((v >> 10) & 0x3FF) as usize, 4),
                2 => ((v & 0x3FFF) as usize, ((v >> 14) & 0x3FFF) as usize, 4),
                _ => ((v & 0x3FFFF) as usize, ((v >> 18) & 0x3FFFF) as usize, 4),
            };
            if regen > MAX_BLOCK {
                return err("literals too large");
            }
            let body = d.get(hbytes..hbytes + comp).ok_or("zstd: literals run past the block")?;
            let mut at = 0;
            if kind == 2 {
                let (t, used) = Huff::read(body)?;
                st.huff = Some(t);
                at = used;
            }
            let huff = st.huff.as_ref().ok_or("zstd: literals need a tree that was never sent")?;
            let data = body.get(at..).ok_or("zstd: bad literals")?;
            let mut out = Vec::with_capacity(regen);
            if streams == 1 {
                huff.decode(data, regen, &mut out)?;
            } else {
                let jt = data.get(..6).ok_or("zstd: short jump table")?;
                let s1 = u16::from_le_bytes([jt[0], jt[1]]) as usize;
                let s2 = u16::from_le_bytes([jt[2], jt[3]]) as usize;
                let s3 = u16::from_le_bytes([jt[4], jt[5]]) as usize;
                let rest = &data[6..];
                if s1 + s2 + s3 > rest.len() {
                    return err("literal streams overlap");
                }
                let per = regen.div_ceil(4);
                let last = regen.checked_sub(3 * per).ok_or("zstd: bad literal sizes")?;
                huff.decode(&rest[..s1], per, &mut out)?;
                huff.decode(&rest[s1..s1 + s2], per, &mut out)?;
                huff.decode(&rest[s1 + s2..s1 + s2 + s3], per, &mut out)?;
                huff.decode(&rest[s1 + s2 + s3..], last, &mut out)?;
            }
            Ok((out, hbytes + comp))
        }
    }
}

fn table_for(mode: u8, d: &[u8], at: &mut usize, default: &[i16], default_log: u32, prev: &Option<Fse>, max_log: u32, max_sym: usize) -> R<Fse> {
    match mode {
        0 => Fse::build(default, default_log),
        1 => {
            let s = *d.get(*at).ok_or("zstd: short sequences section")?;
            *at += 1;
            if s as usize > max_sym {
                return err("symbol out of range");
            }
            Ok(Fse::rle(s))
        }
        2 => {
            let (t, used) = Fse::read(d.get(*at..).ok_or("zstd: short sequences section")?, max_log, max_sym)?;
            *at += used;
            Ok(t)
        }
        _ => prev.clone().ok_or_else(|| "zstd: repeated table was never sent".to_string()),
    }
}

fn block(d: &[u8], st: &mut State, out: &mut Vec<u8>, limit: usize) -> R<()> {
    let (lits, used) = literals(d, st)?;
    let mut at = used;
    let b = *d.get(at).ok_or("zstd: missing sequences")?;
    at += 1;
    let n = if b == 0 {
        0usize
    } else if b < 128 {
        b as usize
    } else if b < 255 {
        let b1 = *d.get(at).ok_or("zstd: short block")? as usize;
        at += 1;
        ((b as usize - 128) << 8) + b1
    } else {
        let lo = *d.get(at).ok_or("zstd: short block")? as usize;
        let hi = *d.get(at + 1).ok_or("zstd: short block")? as usize;
        at += 2;
        lo + (hi << 8) + 0x7F00
    };
    if n == 0 {
        if out.len() + lits.len() > limit {
            return err("output is larger than allowed");
        }
        out.extend_from_slice(&lits);
        return Ok(());
    }
    let modes = *d.get(at).ok_or("zstd: short block")?;
    at += 1;
    let ll = table_for(modes >> 6, d, &mut at, &LL_DEFAULT, 6, &st.ll, 9, 35)?;
    let of = table_for((modes >> 4) & 3, d, &mut at, &OF_DEFAULT, 5, &st.of, 8, 31)?;
    let ml = table_for((modes >> 2) & 3, d, &mut at, &ML_DEFAULT, 6, &st.ml, 9, 52)?;
    let mut bs = Back::new(d.get(at..).ok_or("zstd: short block")?)?;
    let mut sl = bs.read(ll.log) as usize;
    let mut so = bs.read(of.log) as usize;
    let mut sm = bs.read(ml.log) as usize;
    let mut lit_at = 0usize;
    for i in 0..n {
        let (cl, co, cm) = (ll.cells[sl], of.cells[so], ml.cells[sm]);
        let ofcode = co.symbol as u32;
        if ofcode > 31 {
            return err("offset code out of range");
        }
        let off_value = (1u32 << ofcode) + bs.read(ofcode);
        let ml_code = cm.symbol as usize;
        let ll_code = cl.symbol as usize;
        if ml_code > 52 || ll_code > 35 {
            return err("length code out of range");
        }
        let match_len = ML_BASE[ml_code] as usize + bs.read(ML_BITS[ml_code] as u32) as usize;
        let lit_len = LL_BASE[ll_code] as usize + bs.read(LL_BITS[ll_code] as u32) as usize;
        // which distance?
        let offset: usize;
        if off_value > 3 {
            offset = (off_value - 3) as usize;
            st.rep = [offset, st.rep[0], st.rep[1]];
        } else {
            let idx = (off_value - 1) as usize + if lit_len == 0 { 1 } else { 0 };
            match idx {
                0 => offset = st.rep[0],
                1 => {
                    offset = st.rep[1];
                    st.rep = [st.rep[1], st.rep[0], st.rep[2]];
                }
                2 => {
                    offset = st.rep[2];
                    st.rep = [st.rep[2], st.rep[0], st.rep[1]];
                }
                _ => {
                    offset = st.rep[0].checked_sub(1).filter(|o| *o > 0).ok_or("zstd: bad repeated distance")?;
                    st.rep = [offset, st.rep[0], st.rep[1]];
                }
            }
        }
        if offset == 0 {
            return err("distance of zero");
        }
        // literals, then the match
        let lend = lit_at.checked_add(lit_len).filter(|e| *e <= lits.len()).ok_or("zstd: sequence uses too many literals")?;
        if out.len() + lit_len + match_len > limit {
            return err("output is larger than allowed");
        }
        out.extend_from_slice(&lits[lit_at..lend]);
        lit_at = lend;
        if offset > out.len() {
            return err("match reaches before the start");
        }
        let start = out.len() - offset;
        if offset >= match_len {
            out.extend_from_within(start..start + match_len);
        } else {
            for k in 0..match_len {
                let b = out[start + k];
                out.push(b);
            }
        }
        if i + 1 < n {
            // update in the order literals, match, offset
            sl = cl.base as usize + bs.read(cl.bits as u32) as usize;
            sm = cm.base as usize + bs.read(cm.bits as u32) as usize;
            so = co.base as usize + bs.read(co.bits as u32) as usize;
            if sl >= ll.cells.len() || sm >= ml.cells.len() || so >= of.cells.len() {
                return err("state out of range");
            }
        }
    }
    if bs.over > 0 || bs.pos != 0 {
        return err("sequence stream was not used exactly");
    }
    if out.len() + (lits.len() - lit_at) > limit {
        return err("output is larger than allowed");
    }
    out.extend_from_slice(&lits[lit_at..]);
    st.ll = Some(ll);
    st.of = Some(of);
    st.ml = Some(ml);
    Ok(())
}

/// Decompress one or more Zstandard frames. `max_out` caps the result.
pub fn decompress(input: &[u8], max_out: usize) -> R<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let mut at = 0usize;
    while at < input.len() {
        if input.len() - at < 4 {
            return err("trailing bytes");
        }
        let magic = u32::from_le_bytes([input[at], input[at + 1], input[at + 2], input[at + 3]]);
        if (0x184D_2A50..=0x184D_2A5F).contains(&magic) {
            let sz = input.get(at + 4..at + 8).ok_or("zstd: short skippable frame")?;
            let sz = u32::from_le_bytes([sz[0], sz[1], sz[2], sz[3]]) as usize;
            at = at.checked_add(8 + sz).filter(|e| *e <= input.len()).ok_or("zstd: skippable frame runs past the end")?;
            continue;
        }
        if magic != MAGIC {
            return err("not a Zstandard frame");
        }
        at += 4;
        let fd = *input.get(at).ok_or("zstd: short header")?;
        at += 1;
        let single = fd & 0x20 != 0;
        let checksum = fd & 0x04 != 0;
        if fd & 0x08 != 0 {
            return err("reserved bit set");
        }
        if !single {
            at += 1; // window descriptor
        }
        at += match fd & 3 { 0 => 0, 1 => 1, 2 => 2, _ => 4 };
        let fcs = match fd >> 6 { 0 => if single { 1 } else { 0 }, 1 => 2, 2 => 4, _ => 8 };
        if fcs > 0 {
            let b = input.get(at..at + fcs).ok_or("zstd: short header")?;
            let mut v: u64 = 0;
            for (i, x) in b.iter().enumerate() {
                v |= (*x as u64) << (8 * i);
            }
            if fcs == 2 {
                v += 256;
            }
            if v > max_out as u64 {
                return err("output is larger than allowed");
            }
            out.reserve(v as usize);
        }
        at += fcs;
        let mut st = State { huff: None, ll: None, of: None, ml: None, rep: [1, 4, 8] };
        loop {
            let h = input.get(at..at + 3).ok_or("zstd: short block header")?;
            let hv = h[0] as usize | (h[1] as usize) << 8 | (h[2] as usize) << 16;
            at += 3;
            let last = hv & 1 == 1;
            let size = hv >> 3;
            match (hv >> 1) & 3 {
                0 => {
                    let b = input.get(at..at + size).ok_or("zstd: raw block runs past the end")?;
                    if out.len() + size > max_out {
                        return err("output is larger than allowed");
                    }
                    out.extend_from_slice(b);
                    at += size;
                }
                1 => {
                    let b = *input.get(at).ok_or("zstd: short block")?;
                    if out.len() + size > max_out {
                        return err("output is larger than allowed");
                    }
                    out.resize(out.len() + size, b);
                    at += 1;
                }
                2 => {
                    if size > MAX_BLOCK {
                        return err("block too large");
                    }
                    let b = input.get(at..at + size).ok_or("zstd: block runs past the end")?;
                    block(b, &mut st, &mut out, max_out)?;
                    at += size;
                }
                _ => return err("reserved block type"),
            }
            if last {
                break;
            }
        }
        if checksum {
            at += 4;
            if at > input.len() {
                return err("missing checksum");
            }
        }
    }
    Ok(out)
}
