#!/usr/bin/env python3
"""Builds small ZIM files in the layouts real Kiwix archives use, so the reader can be tested
without a download: new layout (content in the C namespace, since 2021) with Zstandard, XZ and
stored/extended clusters, and the old layout (A, I, - namespaces). Uses the system libzstd
through ctypes and Python's lzma. Output: tests/fixtures/zims/*.zim"""
import ctypes, ctypes.util, hashlib, lzma, os, struct, uuid

here = os.path.dirname(os.path.abspath(__file__))
out_dir = os.path.join(here, "zims")
os.makedirs(out_dir, exist_ok=True)
z = ctypes.CDLL(ctypes.util.find_library("zstd") or "libzstd.so.1")
z.ZSTD_compress.restype = ctypes.c_size_t
z.ZSTD_compressBound.restype = ctypes.c_size_t

def zstd(d, level=3):
    cap = z.ZSTD_compressBound(len(d)); buf = ctypes.create_string_buffer(cap)
    n = z.ZSTD_compress(buf, cap, d, len(d), level); return buf.raw[:n]

def page(title, body):
    return ("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>%s</title><link rel=\"stylesheet\" href=\"%s\"></head><body><h1>%s</h1>%s</body></html>" % (title, "{CSS}", title, body)).encode()

PNG = bytes.fromhex("89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360f8cfc000000301010018dd8db00000000049454e44ae426082")

def build(path, layout, comp, extended=False, big_clusters=False):
    new = layout == "new"
    C = b"C" if new else b"A"      # article namespace
    I = b"C" if new else b"I"      # image namespace
    S = b"C" if new else b"-"      # style namespace
    css = b"body{font-family:serif;color:#222}h1{border-bottom:1px solid #999}"
    arts = [
        ("Main_Page", "Main Page", page("Cindrix sample library", "<p>Welcome. See <a href=\"Water\">Water</a>, <a href=\"Fire\">Fire</a>, <a href=\"Shelter\">Shelter</a>.</p><img src=\"pic.png\">")),
        ("Water", "Water", page("Water", "<p>Boil water for one minute to make it safe to drink. Store one gallon per person per day.</p>")),
        ("Water_purification", "Water purification", page("Water purification", "<p>Filter cloudy water, then boil it. Bleach: 8 drops per gallon.</p>")),
        ("Fire", "Fire", page("Fire", "<p>Keep tinder dry. Never use a stove indoors.</p>")),
        ("Shelter", "Shelter", page("Shelter", "<p>Stay dry and out of the wind. Insulate from the ground. Zürich naïve 日本語</p>")),
        ("first_aid", "first aid", page("first aid", "<p>Lower-case title to test case handling.</p>")),
        ("Éclair", "Éclair", page("Éclair", "<p>Accents in titles.</p>")),
    ]
    for i in range(60):   # enough entries for a real binary search
        arts.append(("Topic_%03d" % i, "Topic %03d" % i, page("Topic %03d" % i, "<p>%s</p>" % ("filler text " * (20 + i)))))
    css_name = "style.css"
    entries = []   # dict: ns, url, title, mime, cluster(group), data | redirect(url)
    for url, title, html in arts:
        entries.append(dict(ns=C, url=url, title=title if title != url else "", mime="text/html", group=0, data=html.replace(b"{CSS}", b"style.css")))
    entries.append(dict(ns=S, url=css_name, title="", mime="text/css", group=1, data=css))
    entries.append(dict(ns=I, url="pic.png", title="", mime="image/png", group=2, data=PNG))
    entries.append(dict(ns=C, url="Water_alias", title="Water alias", mime=None, redirect=(C, "Water")))
    entries.append(dict(ns=C, url="Fire_redirect", title="", mime=None, redirect=(C, "Fire")))
    meta = {"Title": "Cindrix sample", "Language": "eng", "Creator": "tests", "Description": "A tiny archive for the reader tests", "Name": "cindrix_sample", "Publisher": "Cindrix Ops tests", "Date": "2026-10-20"}
    for k, v in meta.items():
        entries.append(dict(ns=b"M", url=k, title="", mime="text/plain", group=1, data=v.encode()))
    entries.append(dict(ns=b"M", url="Illustration_48x48@1", title="", mime="image/png", group=2, data=PNG))
    if new:
        entries.append(dict(ns=b"W", url="mainPage", title="", mime=None, redirect=(C, "Main_Page")))
    entries.sort(key=lambda e: (e["ns"], e["url"].encode()))
    index = {(e["ns"], e["url"]): i for i, e in enumerate(entries)}
    mimes = []
    for e in entries:
        if e["mime"] and e["mime"] not in mimes: mimes.append(e["mime"])
    # clusters
    groups = {0: [], 1: [], 2: []}
    for e in entries:
        if e["mime"]:
            e["cluster"] = e["group"]; e["blob"] = len(groups[e["group"]]); groups[e["group"]].append(e["data"])
    if big_clusters:   # put a large page in its own cluster so a cluster is bigger than a few KB
        pass
    def make_cluster(blobs, how):
        width = 8 if extended else 4
        off = (len(blobs) + 1) * width
        offs = []
        for b in blobs: offs.append(off); off += len(b)
        offs.append(off)
        body = b"".join(struct.pack("<Q" if extended else "<I", o) for o in offs) + b"".join(blobs)
        flag = 0x10 if extended else 0
        if how == "zstd": return bytes([5 | flag]) + zstd(body)
        if how == "xz": return bytes([4 | flag]) + lzma.compress(body, format=lzma.FORMAT_XZ, check=lzma.CHECK_CRC32)
        return bytes([1 | flag]) + body
    hows = {0: comp, 1: comp, 2: "stored"}
    clusters = [make_cluster(groups[g], hows[g]) for g in (0, 1, 2)]
    # directory entries
    dir_blobs = []
    for e in entries:
        if e["mime"]:
            d = struct.pack("<HBBIII", mimes.index(e["mime"]), 0, e["ns"][0], 0, e["cluster"], e["blob"])
        else:
            d = struct.pack("<HBBII", 0xFFFF, 0, e["ns"][0], 0, index[e["redirect"]])
        d += e["url"].encode() + b"\0" + e["title"].encode() + b"\0"
        dir_blobs.append(d)
    mime_blob = b"".join(m.encode() + b"\0" for m in mimes) + b"\0"
    n = len(entries)
    pos_mime = 80
    pos_url = pos_mime + len(mime_blob)
    pos_title = pos_url + 8 * n
    pos_cluster = pos_title + 4 * n
    pos_entries = pos_cluster + 8 * len(clusters)
    entry_offs, p = [], pos_entries
    for d in dir_blobs: entry_offs.append(p); p += len(d)
    cl_offs = []
    for c in clusters: cl_offs.append(p); p += len(c)
    checksum_pos = p
    title_order = sorted(range(n), key=lambda i: (entries[i]["ns"], (entries[i]["title"] or entries[i]["url"]).encode()))
    main = index[(C, "Main_Page")]
    hdr = struct.pack("<IHH16sIIQQQQIIQ", 0x44D495A, 6, 1 if new else 0, uuid.UUID(int=0x1234).bytes, n, len(clusters), pos_url, pos_title, pos_cluster, pos_mime, main, 0xFFFFFFFF, checksum_pos)
    body = hdr + mime_blob + b"".join(struct.pack("<Q", o) for o in entry_offs) + b"".join(struct.pack("<I", i) for i in title_order) + b"".join(struct.pack("<Q", o) for o in cl_offs) + b"".join(dir_blobs) + b"".join(clusters)
    assert len(body) == checksum_pos
    with open(path, "wb") as f:
        f.write(body + hashlib.md5(body).digest())

build(os.path.join(out_dir, "new-zstd.zim"), "new", "zstd")
build(os.path.join(out_dir, "new-xz.zim"), "new", "xz")
build(os.path.join(out_dir, "new-ext-stored.zim"), "new", "stored", extended=True)
build(os.path.join(out_dir, "old-zstd.zim"), "old", "zstd")
print("ok", [(f, os.path.getsize(os.path.join(out_dir, f))) for f in sorted(os.listdir(out_dir))])
