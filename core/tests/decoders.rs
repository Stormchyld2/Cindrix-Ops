//! The Zstandard and XZ decoders against real data compressed by the real libzstd and xz
//! (made by tests/fixtures/make_compressed.py): every case must come back byte for byte.

use cindrix_core::md5::{hex, Md5};
use cindrix_core::{xz, zstd};

struct Case {
    name: String,
    len: usize,
    md5: String,
    comp: Vec<u8>,
}

fn load(file: &str) -> Vec<Case> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(file);
    let b = std::fs::read(path).unwrap();
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
    let n = u32_at(0);
    let mut at = 4;
    let mut v = Vec::new();
    for _ in 0..n {
        let nl = u16::from_le_bytes([b[at], b[at + 1]]) as usize;
        let name = String::from_utf8(b[at + 2..at + 2 + nl].to_vec()).unwrap();
        at += 2 + nl;
        let len = u32_at(at);
        let md5 = hex(&b[at + 4..at + 20]);
        let cl = u32_at(at + 20);
        let comp = b[at + 24..at + 24 + cl].to_vec();
        at += 24 + cl;
        v.push(Case { name, len, md5, comp });
    }
    v
}

fn md5_of(d: &[u8]) -> String {
    let mut m = Md5::new();
    m.update(d);
    hex(&m.finish())
}

#[test]
fn zstd_matches_the_real_compressor_on_every_case() {
    let cases = load("zstd.bin");
    assert!(cases.len() > 50);
    for c in &cases {
        let out = zstd::decompress(&c.comp, 64 << 20).unwrap_or_else(|e| panic!("{}: {}", c.name, e));
        assert_eq!(out.len(), c.len, "{}", c.name);
        assert_eq!(md5_of(&out), c.md5, "{}", c.name);
    }
}

#[test]
fn zstd_refuses_damaged_input_without_crashing() {
    let cases = load("zstd.bin");
    let mut rng: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for c in cases.iter().filter(|c| c.comp.len() > 20 && c.comp.len() < 40_000) {
        // truncations
        for cut in [1, 2, 5, c.comp.len() / 2, c.comp.len() - 1] {
            let _ = zstd::decompress(&c.comp[..cut.min(c.comp.len())], 1 << 20);
        }
        // flipped bytes
        for _ in 0..30 {
            let mut d = c.comp.clone();
            let i = (next() as usize) % d.len();
            d[i] ^= (next() as u8) | 1;
            let _ = zstd::decompress(&d, 1 << 20);
        }
    }
    // the output limit is honoured
    let big = cases.iter().find(|c| c.name == "text-200k-l3").unwrap();
    assert!(zstd::decompress(&big.comp, 1000).is_err());
    assert!(zstd::decompress(b"not zstd at all", 1000).is_err());
}

#[test]
fn xz_matches_the_real_compressor_on_every_case() {
    let cases = load("xz.bin");
    assert!(cases.len() > 40);
    for c in &cases {
        let out = xz::decompress(&c.comp, 64 << 20).unwrap_or_else(|e| panic!("{}: {}", c.name, e));
        assert_eq!(out.len(), c.len, "{}", c.name);
        assert_eq!(md5_of(&out), c.md5, "{}", c.name);
    }
}

#[test]
fn xz_refuses_damaged_input_without_crashing() {
    let cases = load("xz.bin");
    let mut rng: u64 = 0x1234_5678_9ABC_DEF1;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for c in cases.iter().filter(|c| c.comp.len() > 30 && c.comp.len() < 40_000) {
        for cut in [1, 6, 12, c.comp.len() / 2, c.comp.len() - 1] {
            let _ = xz::decompress(&c.comp[..cut.min(c.comp.len())], 1 << 20);
        }
        for _ in 0..30 {
            let mut d = c.comp.clone();
            let i = (next() as usize) % d.len();
            d[i] ^= (next() as u8) | 1;
            let _ = xz::decompress(&d, 1 << 20);
        }
    }
    let big = cases.iter().find(|c| c.name == "text-200k-l6").unwrap();
    assert!(xz::decompress(&big.comp, 1000).is_err());
    assert!(xz::decompress(b"not xz at all, no", 1000).is_err());
}
