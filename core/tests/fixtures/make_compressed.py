#!/usr/bin/env python3
"""Makes tests/fixtures/zstd.bin and xz.bin: real data compressed by the real libzstd / xz, with
the MD5 of the original, for the decoder tests in tests/decoders.rs. Not needed to run the tests.
Uses the system libzstd through ctypes and Python's lzma, so it needs no pip packages."""
import ctypes, ctypes.util, hashlib, lzma, os, random, struct, sys

here = os.path.dirname(os.path.abspath(__file__))
z = ctypes.CDLL(ctypes.util.find_library("zstd") or "libzstd.so.1")
z.ZSTD_compressBound.restype = ctypes.c_size_t
z.ZSTD_compress.restype = ctypes.c_size_t
z.ZSTD_createCCtx.restype = ctypes.c_void_p
z.ZSTD_CCtx_setParameter.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
z.ZSTD_compress2.restype = ctypes.c_size_t
z.ZSTD_compress2.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_char_p, ctypes.c_size_t]

def zstd(data, level=3, checksum=False):
    cap = z.ZSTD_compressBound(len(data))
    buf = ctypes.create_string_buffer(cap)
    cctx = z.ZSTD_createCCtx()
    z.ZSTD_CCtx_setParameter(cctx, 100, level)       # compressionLevel
    z.ZSTD_CCtx_setParameter(cctx, 201, 1 if checksum else 0)  # checksumFlag
    n = z.ZSTD_compress2(cctx, buf, cap, data, len(data))
    z.ZSTD_freeCCtx(ctypes.c_void_p(cctx))
    assert not z.ZSTD_isError(ctypes.c_size_t(n)), "compress failed"
    return buf.raw[:n]
z.ZSTD_isError.restype = ctypes.c_uint
z.ZSTD_isError.argtypes = [ctypes.c_size_t]

rnd = random.Random(20261020)
words = ["water", "boil", "fire", "shelter", "the", "of", "and", "to", "a", "in", "is", "that", "for", "it", "as", "with", "was", "on", "are", "be", "this", "from", "have", "or", "by", "one", "had", "not", "but", "what", "all", "were", "when", "we", "there", "can", "an", "your", "which", "their", "said", "if", "do", "will", "each", "about", "how", "up", "out", "them", "then", "she", "many", "some", "so", "these", "would", "other", "into", "has", "more", "her", "two", "like", "him", "see", "time", "could", "no", "make", "than", "first", "been", "its", "who", "now", "people", "my", "made", "over", "did", "down", "only", "way", "find", "use", "may", "long", "little", "very", "after", "words", "called", "just", "where", "most", "know", "emergency", "supplies", "kit", "gallon", "per", "person", "day", "store", "dry", "cool", "place", "Zürich", "naïve", "日本語", "ÅÄÖ"]
def text(n):
    out, size = [], 0
    while size < n:
        w = rnd.choice(words)
        if rnd.random() < 0.08: w = w.capitalize() + "."
        out.append(w); size += len(w) + 1
    return (" ".join(out)).encode()[:n]
def html(n):
    parts = ["<!DOCTYPE html><html><head><title>Sample</title><link rel=\"stylesheet\" href=\"../-/style.css\"></head><body>"]
    while sum(map(len, parts)) < n:
        parts.append("<h2 id=\"s%d\">%s</h2><p>%s <a href=\"../A/%s\">%s</a> %s</p>" % (rnd.randrange(999), rnd.choice(words).title(), text(200).decode("utf-8","ignore"), rnd.choice(words), rnd.choice(words), text(120).decode("utf-8","ignore")))
    parts.append("</body></html>")
    return "".join(parts).encode()[:n]
def binary(n):
    return bytes(rnd.randrange(256) for _ in range(n))
def skewed(n):
    return bytes(min(255, int(rnd.expovariate(0.08))) for _ in range(n))
def structured(n):
    return b"".join(struct.pack("<IHB", i * 7 % 100003, i % 977, i % 5) for i in range(n // 7))
def runs(n):
    out = bytearray()
    while len(out) < n:
        out += bytes([rnd.randrange(256)]) * rnd.randrange(1, 300)
    return bytes(out[:n])
def lzstyle(n):  # lots of repeated distances
    base = text(400)
    out = bytearray()
    while len(out) < n:
        out += base[rnd.randrange(0, 200):][:rnd.randrange(3, 80)] + bytes([rnd.randrange(32, 127)])
    return bytes(out[:n])

cases = []
def add(name, data, **kw):
    cases.append((name, data, zstd(data, **kw)))

add("empty", b"")
add("one-byte", b"x")
add("short-text", b"Boil water for one minute.")
add("run-1000", b"a" * 1000)
add("text-2k", text(2000))
add("text-30k", text(30000))
add("text-200k-l1", text(200000), level=1)
add("text-200k-l3", text(200000), level=3)
add("text-200k-l9", text(200000), level=9)
add("text-200k-l19", text(200000), level=19)
add("text-600k-multiblock", text(600000), level=5)
add("html-100k", html(100000))
add("html-100k-l19-checksum", html(100000), level=19, checksum=True)
add("random-5k", binary(5000))
add("random-300k", binary(140000))
add("skewed-50k", skewed(50000))
add("structured-70k", structured(70000))
add("runs-80k", runs(80000))
add("repeat-offsets-60k", lzstyle(60000), level=7)
add("repeat-offsets-60k-l19", lzstyle(60000), level=19)
for i in range(40):
    add("fuzz-small-%d" % i, [text, html, binary, skewed, runs, lzstyle][i % 6](rnd.randrange(1, 3000)), level=rnd.choice([1, 3, 6, 12, 19]))
# two frames back to back, and a skippable frame before data
d1, d2 = text(5000), html(7000)
cases.append(("two-frames", d1 + d2, zstd(d1) + zstd(d2)))
cases.append(("skippable-first", d1, struct.pack("<II", 0x184D2A55, 5) + b"hello" + zstd(d1)))

def write(path, items):
    with open(path, "wb") as f:
        f.write(struct.pack("<I", len(items)))
        for name, raw, comp in items:
            nb = name.encode()
            f.write(struct.pack("<H", len(nb)) + nb + struct.pack("<I", len(raw)) + hashlib.md5(raw).digest() + struct.pack("<I", len(comp)) + comp)
write(os.path.join(here, "zstd.bin"), cases)

# xz: container with LZMA2, as ZIM writes it (also a few with other settings)
xcases = []
def addx(name, data, preset=6, check=lzma.CHECK_CRC32, extreme=False):
    p = preset | (lzma.PRESET_EXTREME if extreme else 0)
    xcases.append((name, data, lzma.compress(data, format=lzma.FORMAT_XZ, check=check, preset=p)))
addx("empty", b"")
addx("one-byte", b"x")
addx("short-text", b"Boil water for one minute.")
addx("run-1000", b"a" * 1000)
addx("text-30k", text(30000))
addx("text-200k-l0", text(200000), preset=0)
addx("text-200k-l6", text(200000))
addx("text-200k-l9e", text(200000), preset=9, extreme=True)
addx("text-600k", text(600000), preset=3)
addx("html-100k", html(100000))
addx("html-100k-crc64", html(100000), check=lzma.CHECK_CRC64)
addx("html-100k-none", html(100000), check=lzma.CHECK_NONE)
addx("random-5k", binary(5000))
addx("random-300k", binary(140000))
addx("skewed-50k", skewed(50000))
addx("structured-70k", structured(70000))
addx("runs-80k", runs(80000))
addx("repeat-offsets-60k", lzstyle(60000))
for i in range(30):
    addx("fuzz-small-%d" % i, [text, html, binary, skewed, runs, lzstyle][i % 6](rnd.randrange(1, 3000)), preset=rnd.choice([0, 1, 4, 6, 9]))
# a multi-chunk LZMA2 stream with incompressible parts (stored chunks)
mixed = text(100000) + binary(70000) + text(100000)
addx("mixed-stored-chunks", mixed)
write(os.path.join(here, "xz.bin"), xcases)
print("wrote", len(cases), "zstd and", len(xcases), "xz cases;", os.path.getsize(os.path.join(here, "zstd.bin")), os.path.getsize(os.path.join(here, "xz.bin")), "bytes")
