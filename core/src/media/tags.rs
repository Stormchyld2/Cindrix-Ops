//! Song information (title, artist, album, track number, year) and cover pictures, read straight
//! from audio files. Standard library only. Every reader is bounded: it never reads more than a
//! few megabytes, never trusts a length it has not checked, and a damaged or unknown file just
//! gives empty fields. Formats: MP3 (ID3v1, ID3v2.2 to 2.4), FLAC, Ogg Vorbis and Opus, WAV
//! (INFO and ID3 chunks), M4A/MP4, and WMA (ASF).

use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Track number, 0 when unknown.
    pub track: u32,
    /// Four-digit year, 0 when unknown.
    pub year: u32,
    /// The genre named in the file ("Rock"), empty when none.
    pub genre: String,
}

impl Tags {
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.artist.is_empty() && self.album.is_empty() && self.track == 0 && self.year == 0 && self.genre.is_empty()
    }

    /// Fill the fields that are still empty from `o`.
    fn fill(&mut self, o: Tags) {
        if self.title.is_empty() {
            self.title = o.title;
        }
        if self.artist.is_empty() {
            self.artist = o.artist;
        }
        if self.album.is_empty() {
            self.album = o.album;
        }
        if self.track == 0 {
            self.track = o.track;
        }
        if self.year == 0 {
            self.year = o.year;
        }
        if self.genre.is_empty() {
            self.genre = o.genre;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Art {
    pub mime: &'static str,
    pub data: Vec<u8>,
}

/// A text field longer than this is ignored.
const MAX_TEXT: u64 = 8192;
/// The largest cover picture we will return.
pub const MAX_ART: usize = 8 * 1024 * 1024;

// ---------- small helpers ----------

/// The 80 genres a plain ID3 tag can name by number.
const ID3_GENRES: [&str; 80] = [
    "Blues", "Classic Rock", "Country", "Dance", "Disco", "Funk", "Grunge", "Hip-Hop", "Jazz", "Metal", "New Age", "Oldies", "Other", "Pop", "R&B", "Rap",
    "Reggae", "Rock", "Techno", "Industrial", "Alternative", "Ska", "Death Metal", "Pranks", "Soundtrack", "Euro-Techno", "Ambient", "Trip-Hop", "Vocal",
    "Jazz+Funk", "Fusion", "Trance", "Classical", "Instrumental", "Acid", "House", "Game", "Sound Clip", "Gospel", "Noise", "Alternative Rock", "Bass", "Soul",
    "Punk", "Space", "Meditative", "Instrumental Pop", "Instrumental Rock", "Ethnic", "Gothic", "Darkwave", "Techno-Industrial", "Electronic", "Pop-Folk",
    "Eurodance", "Dream", "Southern Rock", "Comedy", "Cult", "Gangsta", "Top 40", "Christian Rap", "Pop/Funk", "Jungle", "Native American", "Cabaret",
    "New Wave", "Psychedelic", "Rave", "Showtunes", "Trailer", "Lo-Fi", "Tribal", "Acid Punk", "Acid Jazz", "Polka", "Retro", "Musical", "Rock & Roll", "Hard Rock",
];

fn genre_by_number(n: usize) -> String {
    ID3_GENRES.get(n).map(|g| g.to_string()).unwrap_or_default()
}

/// A genre as ID3 writes it: "Rock", "(17)", "(17)Rock" or "17". Numbers are looked up; text is kept.
fn genre_text(t: &str) -> String {
    let t = t.trim();
    if let Some(rest) = t.strip_prefix('(') {
        if let Some((n, after)) = rest.split_once(')') {
            if after.trim().is_empty() {
                return n.trim().parse::<usize>().map(genre_by_number).unwrap_or_default();
            }
            return clean(after);
        }
    }
    if !t.is_empty() && t.len() <= 3 && t.chars().all(|c| c.is_ascii_digit()) {
        return t.parse::<usize>().map(genre_by_number).unwrap_or_default();
    }
    clean(t)
}

fn clean(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_string()
}

fn leading_number(s: &str) -> u32 {
    let d: String = s.trim().chars().take_while(|c| c.is_ascii_digit()).take(5).collect();
    d.parse().unwrap_or(0)
}

/// The first run of four digits that is a plausible year.
fn year_of(s: &str) -> u32 {
    let b = s.as_bytes();
    for i in 0..b.len().saturating_sub(3) {
        if b[i..i + 4].iter().all(|c| c.is_ascii_digit()) {
            let y: u32 = s[i..i + 4].parse().unwrap_or(0);
            if (1000..=2999).contains(&y) {
                return y;
            }
        }
    }
    0
}

fn sniff_image(d: &[u8]) -> Option<&'static str> {
    if d.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if d.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else {
        None
    }
}

fn read_at<R: Read + Seek>(r: &mut R, pos: u64, len: usize) -> Option<Vec<u8>> {
    r.seek(SeekFrom::Start(pos)).ok()?;
    let mut v = vec![0u8; len];
    r.read_exact(&mut v).ok()?;
    Some(v)
}

fn utf16(b: &[u8], le: bool) -> String {
    let u: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
        .collect();
    String::from_utf16_lossy(&u)
}

fn utf16_bom(b: &[u8]) -> String {
    if b.starts_with(&[0xFF, 0xFE]) {
        utf16(&b[2..], true)
    } else if b.starts_with(&[0xFE, 0xFF]) {
        utf16(&b[2..], false)
    } else {
        utf16(b, true)
    }
}

fn be32(b: &[u8], i: &mut usize) -> Option<u32> {
    let v = u32::from_be_bytes(b.get(*i..*i + 4)?.try_into().ok()?);
    *i += 4;
    Some(v)
}

fn le32(b: &[u8], i: &mut usize) -> Option<u32> {
    let v = u32::from_le_bytes(b.get(*i..*i + 4)?.try_into().ok()?);
    *i += 4;
    Some(v)
}

// ---------- ID3 (MP3, and inside WAV) ----------

fn synchsafe(b: &[u8]) -> u32 {
    ((b[0] & 0x7f) as u32) << 21 | ((b[1] & 0x7f) as u32) << 14 | ((b[2] & 0x7f) as u32) << 7 | (b[3] & 0x7f) as u32
}

/// An ID3 text frame: an encoding byte, then the text.
fn id3_text(data: &[u8]) -> String {
    let Some((&enc, rest)) = data.split_first() else { return String::new() };
    let s = match enc {
        0 => rest.iter().map(|&b| b as char).collect::<String>(),
        1 => utf16_bom(rest),
        2 => utf16(rest, false),
        _ => String::from_utf8_lossy(rest).to_string(),
    };
    clean(s.split('\0').next().unwrap_or(""))
}

fn id3_picture(d: &[u8], v22: bool) -> Option<Art> {
    let enc = *d.first()?;
    let mut i = 1usize;
    if v22 {
        i += 3; // a three-letter image format
    } else {
        let z = d.get(i..)?.iter().position(|&b| b == 0)?;
        i += z + 1; // the MIME type
    }
    i += 1; // picture type
    if enc == 1 || enc == 2 {
        // description in UTF-16: ends with two zero bytes, aligned
        let mut j = i;
        loop {
            if j + 1 >= d.len() {
                return None;
            }
            if d[j] == 0 && d[j + 1] == 0 {
                break;
            }
            j += 2;
        }
        i = j + 2;
    } else {
        let z = d.get(i..)?.iter().position(|&b| b == 0)?;
        i += z + 1;
    }
    let data = d.get(i..)?;
    if data.len() > MAX_ART {
        return None;
    }
    Some(Art { mime: sniff_image(data)?, data: data.to_vec() })
}

fn un_unsync(d: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len());
    let mut i = 0;
    while i < d.len() {
        out.push(d[i]);
        if d[i] == 0xFF && d.get(i + 1) == Some(&0) {
            i += 1;
        }
        i += 1;
    }
    out
}

fn id3v2<R: Read + Seek>(r: &mut R, want_art: bool) -> Option<(Tags, Option<Art>)> {
    let file_len = r.seek(SeekFrom::End(0)).ok()?;
    let h = read_at(r, 0, 10)?;
    if &h[..3] != b"ID3" {
        return None;
    }
    let ver = h[3];
    if !(2..=4).contains(&ver) {
        return None;
    }
    let flags = h[5];
    let end = (10 + synchsafe(&h[6..10]) as u64).min(file_len);
    let mut pos = 10u64;
    if ver >= 3 && flags & 0x40 != 0 {
        let e = read_at(r, pos, 4)?;
        pos += if ver == 4 { synchsafe(&e) as u64 } else { u32::from_be_bytes([e[0], e[1], e[2], e[3]]) as u64 + 4 };
    }
    let hdr = if ver == 2 { 6u64 } else { 10 };
    let mut tags = Tags::default();
    let mut album_artist = String::new();
    let mut art = None;
    while pos + hdr <= end {
        let fh = read_at(r, pos, hdr as usize)?;
        if fh[0] == 0 {
            break; // padding
        }
        let (id, size) = if ver == 2 {
            (String::from_utf8_lossy(&fh[..3]).to_string(), ((fh[3] as u64) << 16) | ((fh[4] as u64) << 8) | fh[5] as u64)
        } else {
            let sz = if ver == 4 { synchsafe(&fh[4..8]) as u64 } else { u32::from_be_bytes([fh[4], fh[5], fh[6], fh[7]]) as u64 };
            (String::from_utf8_lossy(&fh[..4]).to_string(), sz)
        };
        let body = pos + hdr;
        if size == 0 || body + size > end {
            break;
        }
        let f1 = if ver >= 3 { fh[9] } else { 0 };
        let (skip, dli, mut unsync) = match ver {
            4 => (f1 & 0x0C != 0, f1 & 0x01 != 0, f1 & 0x02 != 0),
            3 => (f1 & 0xC0 != 0, false, false),
            _ => (false, false, false),
        };
        unsync |= ver == 3 && flags & 0x80 != 0;
        let is_text = id.starts_with('T') && id != "TXXX" && id != "TXX";
        let is_pic = id == "APIC" || id == "PIC";
        if !skip && ((is_text && size <= MAX_TEXT) || (want_art && art.is_none() && is_pic && size as usize <= MAX_ART + 4096)) {
            if let Some(mut d) = read_at(r, body, size as usize) {
                if dli {
                    d.drain(..4.min(d.len()));
                }
                if unsync {
                    d = un_unsync(d);
                }
                if is_pic {
                    art = id3_picture(&d, ver == 2);
                } else {
                    let t = id3_text(&d);
                    match id.as_str() {
                        "TIT2" | "TT2" => tags.title = t,
                        "TPE1" | "TP1" => tags.artist = t,
                        "TALB" | "TAL" => tags.album = t,
                        "TCON" | "TCO" => tags.genre = genre_text(&t),
                        "TPE2" | "TP2" => album_artist = t,
                        "TRCK" | "TRK" => tags.track = leading_number(&t),
                        "TDRC" | "TYER" | "TYE"
                            if tags.year == 0 => {
                                tags.year = year_of(&t)
                            }
                        _ => {}
                    }
                }
            }
        }
        pos = body + size;
    }
    if tags.artist.is_empty() {
        tags.artist = album_artist;
    }
    Some((tags, art))
}

fn latin1_field(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    clean(&b[..end].iter().map(|&c| c as char).collect::<String>())
}

fn id3v1<R: Read + Seek>(r: &mut R) -> Option<Tags> {
    let len = r.seek(SeekFrom::End(0)).ok()?;
    if len < 128 {
        return None;
    }
    let b = read_at(r, len - 128, 128)?;
    if &b[..3] != b"TAG" {
        return None;
    }
    let track = if b[125] == 0 && b[126] != 0 { b[126] as u32 } else { 0 };
    Some(Tags {
        title: latin1_field(&b[3..33]),
        artist: latin1_field(&b[33..63]),
        album: latin1_field(&b[63..93]),
        year: year_of(&latin1_field(&b[93..97])),
        track,
        genre: genre_by_number(b[127] as usize),
    })
}

fn mpeg<R: Read + Seek>(r: &mut R, want_art: bool) -> (Tags, Option<Art>) {
    let (mut tags, art) = id3v2(r, want_art).unwrap_or_default();
    if let Some(v1) = id3v1(r) {
        tags.fill(v1);
    }
    (tags, art)
}

// ---------- Vorbis comments (FLAC, Ogg Vorbis, Opus) ----------

fn vorbis_comments(b: &[u8]) -> Tags {
    let mut t = Tags::default();
    let mut i = 0usize;
    let Some(vendor) = le32(b, &mut i) else { return t };
    i = i.saturating_add(vendor as usize);
    let Some(count) = le32(b, &mut i) else { return t };
    let mut album_artist = String::new();
    for _ in 0..count.min(1000) {
        let Some(len) = le32(b, &mut i) else { break };
        let Some(s) = b.get(i..i.saturating_add(len as usize)) else { break };
        i += len as usize;
        let s = String::from_utf8_lossy(s);
        let Some((k, v)) = s.split_once('=') else { continue };
        let v = clean(v);
        match k.to_ascii_uppercase().as_str() {
            "TITLE" => t.title = v,
            "ARTIST" => {
                if t.artist.is_empty() {
                    t.artist = v
                }
            }
            "ALBUMARTIST" => album_artist = v,
            "ALBUM" => t.album = v,
            "GENRE" => {
                if t.genre.is_empty() {
                    t.genre = genre_text(&v)
                }
            }
            "TRACKNUMBER" => t.track = leading_number(&v),
            "DATE" | "YEAR"
                if t.year == 0 => {
                    t.year = year_of(&v)
                }
            _ => {}
        }
    }
    if t.artist.is_empty() {
        t.artist = album_artist;
    }
    t
}

fn flac<R: Read + Seek>(r: &mut R, want_art: bool) -> Option<(Tags, Option<Art>)> {
    if read_at(r, 0, 4)? != b"fLaC" {
        return None;
    }
    let mut pos = 4u64;
    let mut tags = Tags::default();
    let mut art: Option<(u32, Art)> = None;
    for _ in 0..64 {
        let h = read_at(r, pos, 4)?;
        let last = h[0] & 0x80 != 0;
        let typ = h[0] & 0x7f;
        let len = ((h[1] as u64) << 16) | ((h[2] as u64) << 8) | h[3] as u64;
        let body = pos + 4;
        if typ == 4 && len <= 1 << 20 {
            tags = vorbis_comments(&read_at(r, body, len as usize)?);
        } else if typ == 6 && want_art && len as usize <= MAX_ART + 4096 {
            if let Some(b) = read_at(r, body, len as usize) {
                if let Some((kind, a)) = flac_picture(&b) {
                    // a front cover (type 3) beats any other picture
                    if art.as_ref().map(|(k, _)| *k != 3 && kind == 3).unwrap_or(true) {
                        art = Some((kind, a));
                    }
                }
            }
        }
        pos = body + len;
        if last {
            break;
        }
    }
    Some((tags, art.map(|(_, a)| a)))
}

fn flac_picture(b: &[u8]) -> Option<(u32, Art)> {
    let mut i = 0usize;
    let kind = be32(b, &mut i)?;
    let mime_len = be32(b, &mut i)? as usize;
    i = i.checked_add(mime_len)?;
    let desc_len = be32(b, &mut i)? as usize;
    i = i.checked_add(desc_len)?.checked_add(16)?; // width, height, depth, colours
    let len = be32(b, &mut i)? as usize;
    let data = b.get(i..i.checked_add(len)?)?;
    if data.len() > MAX_ART {
        return None;
    }
    Some((kind, Art { mime: sniff_image(data)?, data: data.to_vec() }))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Ogg Vorbis and Opus keep their comments in the first page or two. Best effort: a comment
/// block that runs across a page boundary gives fewer fields rather than wrong ones.
fn ogg<R: Read + Seek>(r: &mut R) -> Option<Tags> {
    let len = r.seek(SeekFrom::End(0)).ok()?.min(65536) as usize;
    let b = read_at(r, 0, len)?;
    let start = if let Some(p) = find(&b, b"\x03vorbis") {
        p + 7
    } else {
        let p = find(&b, b"OpusTags")?;
        p + 8
    };
    Some(vorbis_comments(&b[start..]))
}

// ---------- WAV ----------

fn wav<R: Read + Seek>(r: &mut R, want_art: bool) -> Option<(Tags, Option<Art>)> {
    let len = r.seek(SeekFrom::End(0)).ok()?;
    let h = read_at(r, 0, 12)?;
    if &h[..4] != b"RIFF" || &h[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12u64;
    let mut tags = Tags::default();
    let mut art = None;
    for _ in 0..64 {
        if pos + 8 > len {
            break;
        }
        let ch = read_at(r, pos, 8)?;
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        let body = pos + 8;
        if &ch[..4] == b"LIST" && (4..=65536).contains(&size) {
            if let Some(b) = read_at(r, body, size as usize) {
                if &b[..4] == b"INFO" {
                    tags.fill(wav_info(&b[4..]));
                }
            }
        } else if (&ch[..4] == b"id3 " || &ch[..4] == b"ID3 ") && (10..=(1u64 << 20) + MAX_ART as u64 + 4096).contains(&size) && (want_art || size <= 1 << 20) {
            // A cover picture lives in this chunk too, so it can be much larger than the text-only tag.
            if let Some(b) = read_at(r, body, size as usize) {
                if let Some((t, a)) = id3v2(&mut Cursor::new(b), want_art) {
                    tags.fill(t);
                    if art.is_none() {
                        art = a;
                    }
                }
            }
        }
        pos = body + size + (size & 1);
    }
    if want_art && art.is_none() {
        // Some programs write the picture where the chunk list does not lead to it (after the audio
        // with no chunk around it, or in a file whose length fields are wrong). Look for the tag itself.
        if let Some((t, a)) = id3_scan(r, len) {
            tags.fill(t);
            art = a;
        }
    }
    Some((tags, art))
}

/// Look through the start and the end of a file for an ID3 tag that carries a picture.
fn id3_scan<R: Read + Seek>(r: &mut R, len: u64) -> Option<(Tags, Option<Art>)> {
    const WINDOW: u64 = 24 * 1024 * 1024;
    let spans: Vec<(u64, u64)> = if len <= WINDOW * 2 { vec![(0, len)] } else { vec![(0, WINDOW), (len - WINDOW, len)] };
    for (a, b) in spans {
        let buf = read_at(r, a, (b - a) as usize)?;
        let mut i = 0usize;
        while let Some(k) = find(&buf[i..], b"ID3") {
            let at = i + k;
            i = at + 3;
            let Some(h) = buf.get(at..at + 10) else { break };
            if !(2..=4).contains(&h[3]) || h[4] == 0xFF || h[6..10].iter().any(|&x| x & 0x80 != 0) {
                continue;
            }
            let total = 10 + synchsafe(&h[6..10]) as usize;
            if total < 20 || total > MAX_ART + (1 << 20) + 4096 {
                continue;
            }
            let end = (at + total).min(buf.len());
            if let Some((t, Some(art))) = id3v2(&mut Cursor::new(buf[at..end].to_vec()), true) {
                return Some((t, Some(art)));
            }
        }
    }
    None
}

fn wav_info(b: &[u8]) -> Tags {
    let mut t = Tags::default();
    let mut i = 0usize;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let Some(data) = b.get(i + 8..i + 8 + size) else { break };
        let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
        let text = clean(&String::from_utf8_lossy(&data[..end]));
        match id {
            b"INAM" => t.title = text,
            b"IART" => t.artist = text,
            b"IPRD" => t.album = text,
            b"IGNR" => t.genre = genre_text(&text),
            b"ITRK" | b"IPRT" => t.track = leading_number(&text),
            b"ICRD" => t.year = year_of(&text),
            _ => {}
        }
        i += 8 + size + (size & 1);
    }
    t
}

// ---------- MP4 / M4A ----------

/// The child atoms of the container at `start..end`: (name, data start, end). Reads headers only.
fn atoms<R: Read + Seek>(r: &mut R, start: u64, end: u64) -> Vec<([u8; 4], u64, u64)> {
    let mut out = Vec::new();
    let mut pos = start;
    while pos + 8 <= end && out.len() < 512 {
        let Some(h) = read_at(r, pos, 8) else { break };
        let mut size = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
        let mut hl = 8u64;
        if size == 1 {
            let Some(b) = read_at(r, pos + 8, 8) else { break };
            size = u64::from_be_bytes(b.try_into().unwrap_or([0; 8]));
            hl = 16;
        } else if size == 0 {
            size = end - pos;
        }
        if size < hl || pos + size > end {
            break;
        }
        out.push(([h[4], h[5], h[6], h[7]], pos + hl, pos + size));
        pos += size;
    }
    out
}

fn child<R: Read + Seek>(r: &mut R, start: u64, end: u64, name: &[u8; 4]) -> Option<(u64, u64)> {
    atoms(r, start, end).into_iter().find(|a| &a.0 == name).map(|a| (a.1, a.2))
}

fn mp4<R: Read + Seek>(r: &mut R, want_art: bool) -> Option<(Tags, Option<Art>)> {
    let len = r.seek(SeekFrom::End(0)).ok()?;
    let (ms, me) = child(r, 0, len, b"moov")?;
    let (us, ue) = child(r, ms, me, b"udta")?;
    let (ts, te) = child(r, us, ue, b"meta")?;
    let (is, ie) = child(r, ts + 4, te, b"ilst")?; // meta starts with a version and flags word
    let mut tags = Tags::default();
    let mut art = None;
    for (name, s, e) in atoms(r, is, ie) {
        let Some((ds, de)) = child(r, s, e, b"data") else { continue };
        let size = de - ds;
        if size < 8 {
            continue;
        }
        let is_art = &name == b"covr";
        if is_art && (!want_art || art.is_some() || size as usize > MAX_ART + 64) {
            continue;
        }
        if !is_art && size > MAX_TEXT {
            continue;
        }
        let Some(d) = read_at(r, ds, size as usize) else { continue };
        let payload = &d[8..]; // after a version and flags word and a reserved word
        let text = || clean(&String::from_utf8_lossy(payload));
        match &name {
            [0xA9, b'n', b'a', b'm'] => tags.title = text(),
            [0xA9, b'A', b'R', b'T'] => tags.artist = text(),
            b"aART" => {
                if tags.artist.is_empty() {
                    tags.artist = text()
                }
            }
            [0xA9, b'a', b'l', b'b'] => tags.album = text(),
            [0xA9, b'g', b'e', b'n'] => tags.genre = genre_text(&text()),
            b"gnre" if payload.len() >= 2 => tags.genre = genre_by_number((u16::from_be_bytes([payload[0], payload[1]]) as usize).saturating_sub(1)),
            [0xA9, b'd', b'a', b'y'] => tags.year = year_of(&text()),
            b"trkn" if payload.len() >= 4 => tags.track = u16::from_be_bytes([payload[2], payload[3]]) as u32,
            b"covr" => art = sniff_image(payload).map(|mime| Art { mime, data: payload.to_vec() }),
            _ => {}
        }
    }
    Some((tags, art))
}

// ---------- WMA (ASF) ----------

const ASF_HEADER: [u8; 16] = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C];
const ASF_CONTENT: [u8; 16] = [0x33, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C];
const ASF_EXTENDED: [u8; 16] = [0x40, 0xA4, 0xD0, 0xD2, 0x07, 0xE3, 0xD2, 0x11, 0x97, 0xF0, 0x00, 0xA0, 0xC9, 0x5E, 0xA8, 0x50];

fn u16le(b: &[u8], i: &mut usize) -> Option<usize> {
    let v = u16::from_le_bytes(b.get(*i..*i + 2)?.try_into().ok()?) as usize;
    *i += 2;
    Some(v)
}

fn asf<R: Read + Seek>(r: &mut R) -> Option<Tags> {
    let h = read_at(r, 0, 30)?;
    if h[..16] != ASF_HEADER {
        return None;
    }
    let count = u32::from_le_bytes([h[24], h[25], h[26], h[27]]).min(64);
    let mut pos = 30u64;
    let mut tags = Tags::default();
    for _ in 0..count {
        let oh = read_at(r, pos, 24)?;
        let size = u64::from_le_bytes(oh[16..24].try_into().ok()?);
        if !(24..=4 << 20).contains(&size) {
            break;
        }
        let body = pos + 24;
        if oh[..16] == ASF_CONTENT {
            let b = read_at(r, body, (size - 24) as usize)?;
            let mut i = 0usize;
            let lens: Vec<usize> = (0..5).filter_map(|_| u16le(&b, &mut i)).collect();
            if lens.len() == 5 {
                let mut take = |n: usize| -> String {
                    let s = b.get(i..i + n).map(utf16_bom_le).unwrap_or_default();
                    i += n;
                    clean(&s)
                };
                tags.title = take(lens[0]);
                tags.artist = take(lens[1]);
            }
        } else if oh[..16] == ASF_EXTENDED {
            let b = read_at(r, body, (size - 24) as usize)?;
            let mut i = 0usize;
            let n = u16le(&b, &mut i)?;
            for _ in 0..n.min(512) {
                let Some(nl) = u16le(&b, &mut i) else { break };
                let Some(name) = b.get(i..i + nl).map(utf16_bom_le) else { break };
                i += nl;
                let Some(ty) = u16le(&b, &mut i) else { break };
                let Some(vl) = u16le(&b, &mut i) else { break };
                let Some(v) = b.get(i..i + vl) else { break };
                i += vl;
                let text = match ty {
                    0 => clean(&utf16_bom_le(v)),
                    3 if v.len() >= 4 => u32::from_le_bytes([v[0], v[1], v[2], v[3]]).to_string(),
                    5 if v.len() >= 2 => u16::from_le_bytes([v[0], v[1]]).to_string(),
                    _ => String::new(),
                };
                match clean(&name).as_str() {
                    "WM/AlbumTitle" => tags.album = text,
                    "WM/Genre" => tags.genre = genre_text(&text),
                    "WM/AlbumArtist" => {
                        if tags.artist.is_empty() {
                            tags.artist = text
                        }
                    }
                    "WM/TrackNumber" => tags.track = leading_number(&text),
                    "WM/Track" => {
                        if tags.track == 0 {
                            tags.track = leading_number(&text) + 1 // the old field counts from zero
                        }
                    }
                    "WM/Year" => tags.year = year_of(&text),
                    _ => {}
                }
            }
        }
        pos += size;
    }
    Some(tags)
}

fn utf16_bom_le(b: &[u8]) -> String {
    utf16(b, true).trim_end_matches('\0').to_string()
}

// ---------- entry points ----------

#[derive(PartialEq)]
enum Kind {
    Mpeg,
    Flac,
    Ogg,
    Wav,
    Mp4,
    Asf,
    Unknown,
}

fn kind_of(head: &[u8], ext: &str) -> Kind {
    if head.starts_with(b"ID3") {
        Kind::Mpeg
    } else if head.starts_with(b"fLaC") {
        Kind::Flac
    } else if head.starts_with(b"OggS") {
        Kind::Ogg
    } else if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WAVE" {
        Kind::Wav
    } else if head.len() >= 8 && &head[4..8] == b"ftyp" {
        Kind::Mp4
    } else if head.len() >= 16 && head[..16] == ASF_HEADER {
        Kind::Asf
    } else if ext == "mp3" || ext == "mp2" {
        Kind::Mpeg
    } else {
        Kind::Unknown
    }
}

fn read_all(path: &Path, want_art: bool) -> (Tags, Option<Art>) {
    let Ok(mut f) = File::open(path) else { return (Tags::default(), None) };
    let mut head = [0u8; 16];
    let n = f.read(&mut head).unwrap_or(0);
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let r = &mut f;
    match kind_of(&head[..n], &ext) {
        Kind::Mpeg => mpeg(r, want_art),
        Kind::Flac => flac(r, want_art).unwrap_or_default(),
        Kind::Ogg => (ogg(r).unwrap_or_default(), None),
        Kind::Wav => wav(r, want_art).unwrap_or_default(),
        Kind::Mp4 => mp4(r, want_art).unwrap_or_default(),
        Kind::Asf => (asf(r).unwrap_or_default(), None),
        Kind::Unknown => (Tags::default(), None),
    }
}

/// Title, artist, album, track number and year of an audio file. A file with no readable tags
/// gives empty fields.
pub fn read_tags(path: &Path) -> Tags {
    read_all(path, false).0
}

/// The picture embedded in an audio file, when it has one (MP3, WAV, FLAC, M4A).
pub fn read_art(path: &Path) -> Option<Art> {
    read_all(path, true).1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id3_frame(ver: u8, id: &str, body: &[u8]) -> Vec<u8> {
        let mut v = id.as_bytes().to_vec();
        if ver == 4 {
            let n = body.len() as u32;
            v.extend([(n >> 21) as u8 & 0x7f, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
        } else if ver == 3 {
            v.extend((body.len() as u32).to_be_bytes());
        } else {
            let n = body.len() as u32;
            v.extend([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
        }
        if ver >= 3 {
            v.extend([0, 0]);
        }
        v.extend(body);
        v
    }

    fn text(enc: u8, s: &str) -> Vec<u8> {
        let mut v = vec![enc];
        match enc {
            0 | 3 => v.extend(s.as_bytes()), // Latin-1 tests use ASCII only
            1 => {
                v.extend([0xFF, 0xFE]);
                for u in s.encode_utf16() {
                    v.extend(u.to_le_bytes());
                }
            }
            _ => {
                for u in s.encode_utf16() {
                    v.extend(u.to_be_bytes());
                }
            }
        }
        v
    }

    fn id3_tag(ver: u8, frames: &[Vec<u8>], padding: usize) -> Vec<u8> {
        let body: Vec<u8> = frames.concat();
        let n = (body.len() + padding) as u32;
        let mut v = vec![b'I', b'D', b'3', ver, 0, 0, (n >> 21) as u8 & 0x7f, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f];
        v.extend(body);
        v.extend(vec![0u8; padding]);
        v
    }

    fn parse(bytes: Vec<u8>, art: bool) -> (Tags, Option<Art>) {
        mpeg(&mut Cursor::new(bytes), art)
    }

    const JPEG: [u8; 6] = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2];

    #[test]
    fn id3v23_text_frames_in_every_encoding() {
        for enc in [0u8, 1, 2, 3] {
            let mut f = vec![
                id3_frame(3, "TIT2", &text(enc, "Boil Water")),
                id3_frame(3, "TPE1", &text(enc, "The Ready Band")),
                id3_frame(3, "TALB", &text(enc, "Prepared")),
                id3_frame(3, "TRCK", &text(enc, "3/12")),
                id3_frame(3, "TYER", &text(enc, "2004")),
            ];
            f.push(id3_frame(3, "TXXX", &text(enc, "ignored"))); // not a field we read
            let mut b = id3_tag(3, &f, 64);
            b.extend([0xFF, 0xFB, 0x90, 0x00]); // the start of audio
            let (t, _) = parse(b, false);
            assert_eq!(
                t,
                Tags { title: "Boil Water".into(), artist: "The Ready Band".into(), album: "Prepared".into(), track: 3, year: 2004, genre: String::new() },
                "encoding {}",
                enc
            );
        }
    }

    #[test]
    fn id3v24_synchsafe_sizes_and_recording_date() {
        let f = vec![
            id3_frame(4, "TIT2", &text(3, "Fire")),
            id3_frame(4, "TPE2", &text(3, "Album Artist")),
            id3_frame(4, "TDRC", &text(3, "1999-05-01")),
        ];
        let (t, _) = parse(id3_tag(4, &f, 10), false);
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.year), ("Fire", "Album Artist", 1999)); // album artist fills a missing artist
    }

    #[test]
    fn id3v22_three_letter_frames() {
        let f = vec![id3_frame(2, "TT2", &text(0, "Old Song")), id3_frame(2, "TP1", &text(0, "Old Band")), id3_frame(2, "TRK", &text(0, "7"))];
        let (t, _) = parse(id3_tag(2, &f, 0), false);
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.track), ("Old Song", "Old Band", 7));
    }

    #[test]
    fn id3v1_fills_what_v2_lacks_and_stands_alone() {
        let mut v1 = vec![0u8; 128];
        v1[..3].copy_from_slice(b"TAG");
        v1[3..11].copy_from_slice(b"V1 Title");
        v1[33..41].copy_from_slice(b"V1 Artst");
        v1[63..71].copy_from_slice(b"V1 Album");
        v1[93..97].copy_from_slice(b"1988");
        v1[125] = 0;
        v1[126] = 9;
        // alone
        let mut b = vec![0xFF, 0xFB, 0x90, 0x00, 1, 2, 3];
        b.extend(&v1);
        let (t, _) = parse(b, false);
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.album.as_str(), t.year, t.track), ("V1 Title", "V1 Artst", "V1 Album", 1988, 9));
        // v2 wins, v1 fills the gaps
        let mut b = id3_tag(3, &[id3_frame(3, "TIT2", &text(0, "V2 Title"))], 0);
        b.extend(&v1);
        let (t, _) = parse(b, false);
        assert_eq!((t.title.as_str(), t.artist.as_str()), ("V2 Title", "V1 Artst"));
    }

    #[test]
    fn id3_cover_picture_is_found_and_checked() {
        let mut pic = vec![0u8];
        pic.extend(b"image/jpeg\0");
        pic.push(3);
        pic.extend(b"front\0");
        pic.extend(JPEG);
        let b = id3_tag(3, &[id3_frame(3, "TIT2", &text(0, "T")), id3_frame(3, "APIC", &pic)], 0);
        let (t, art) = parse(b.clone(), true);
        assert_eq!(t.title, "T");
        let art = art.expect("cover");
        assert_eq!((art.mime, art.data.as_slice()), ("image/jpeg", &JPEG[..]));
        assert!(parse(b, false).1.is_none()); // not read unless asked for

        // a "picture" that is not an image is dropped
        let mut bad = vec![0u8];
        bad.extend(b"image/jpeg\0");
        bad.push(3);
        bad.extend(b"\0");
        bad.extend(b"not an image at all");
        assert!(parse(id3_tag(3, &[id3_frame(3, "APIC", &bad)], 0), true).1.is_none());
    }

    #[test]
    fn damaged_id3_never_panics_or_over_reads() {
        // a frame that claims to be bigger than the tag
        let mut f = id3_frame(3, "TIT2", &text(0, "x"));
        f[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        let (t, _) = parse(id3_tag(3, &[f], 0), true);
        assert!(t.is_empty());
        // a tag that claims to be bigger than the file
        let mut b = id3_tag(3, &[id3_frame(3, "TIT2", &text(0, "Short"))], 0);
        b[6..10].copy_from_slice(&[0x7f, 0x7f, 0x7f, 0x7f]);
        let (t, _) = parse(b, false);
        assert_eq!(t.title, "Short");
        // every prefix of a good file
        let good = id3_tag(3, &[id3_frame(3, "TIT2", &text(1, "Whole")), id3_frame(3, "TRCK", &text(0, "2"))], 8);
        for n in 0..good.len() {
            let _ = parse(good[..n].to_vec(), true);
        }
        assert!(parse(vec![], true).0.is_empty());
        assert!(parse(b"ID3".to_vec(), true).0.is_empty());
    }

    fn comment(items: &[&str]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(5u32.to_le_bytes());
        v.extend(b"vendr");
        v.extend((items.len() as u32).to_le_bytes());
        for s in items {
            v.extend((s.len() as u32).to_le_bytes());
            v.extend(s.as_bytes());
        }
        v
    }

    #[test]
    fn vorbis_comments_in_flac_with_front_cover() {
        let c = comment(&["title=Stay Warm", "Artist=Camp Fire", "ALBUM=Winter", "TRACKNUMBER=4/9", "DATE=2011-02-03", "bad line"]);
        let mut pic = Vec::new();
        pic.extend(3u32.to_be_bytes());
        pic.extend(10u32.to_be_bytes());
        pic.extend(b"image/jpeg");
        pic.extend(0u32.to_be_bytes());
        pic.extend([0u8; 16]);
        pic.extend((JPEG.len() as u32).to_be_bytes());
        pic.extend(JPEG);
        let mut f = b"fLaC".to_vec();
        f.push(0); // STREAMINFO placeholder, 4 bytes
        f.extend([0, 0, 4, 1, 2, 3, 4]);
        f.push(4); // VORBIS_COMMENT
        f.extend([(c.len() >> 16) as u8, (c.len() >> 8) as u8, c.len() as u8]);
        f.extend(&c);
        f.push(0x80 | 6); // PICTURE, last block
        f.extend([(pic.len() >> 16) as u8, (pic.len() >> 8) as u8, pic.len() as u8]);
        f.extend(&pic);
        let (t, art) = flac(&mut Cursor::new(f), true).unwrap();
        assert_eq!(t, Tags { title: "Stay Warm".into(), artist: "Camp Fire".into(), album: "Winter".into(), track: 4, year: 2011, genre: String::new() });
        assert_eq!(art.unwrap().data, JPEG);
    }

    #[test]
    fn ogg_and_opus_comments() {
        let c = comment(&["TITLE=Opus Song", "ARTIST=Open Band"]);
        for magic in [&b"\x03vorbis"[..], &b"OpusTags"[..]] {
            let mut f = b"OggS\0\x02 some page header bytes ".to_vec();
            f.extend(magic);
            f.extend(&c);
            f.extend(vec![0u8; 100]);
            let t = ogg(&mut Cursor::new(f)).unwrap();
            assert_eq!((t.title.as_str(), t.artist.as_str()), ("Opus Song", "Open Band"));
        }
        assert!(ogg(&mut Cursor::new(b"OggS nothing here".to_vec())).is_none());
    }

    #[test]
    fn wav_info_chunk_and_skipping_the_audio() {
        let mut info = b"INFO".to_vec();
        for (id, val) in [(&b"INAM"[..], "Siren Test"), (b"IART", "Alarm Co"), (b"IPRD", "Signals"), (b"ITRK", "2"), (b"ICRD", "2020-01-02")] {
            let mut data = val.as_bytes().to_vec();
            data.push(0);
            info.extend(id);
            info.extend((data.len() as u32).to_le_bytes());
            info.extend(&data);
            if data.len() % 2 == 1 {
                info.push(0);
            }
        }
        let mut f = b"RIFF\0\0\0\0WAVE".to_vec();
        f.extend(b"data");
        f.extend(1000u32.to_le_bytes());
        f.extend(vec![0u8; 1000]); // the audio comes first; the tags are after it
        f.extend(b"LIST");
        f.extend((info.len() as u32).to_le_bytes());
        f.extend(&info);
        let t = wav(&mut Cursor::new(f), false).unwrap().0;
        assert_eq!(t, Tags { title: "Siren Test".into(), artist: "Alarm Co".into(), album: "Signals".into(), track: 2, year: 2020, genre: String::new() });
    }

    #[test]
    fn wav_with_an_id3_chunk_gives_its_cover_picture() {
        let mut pic = vec![0u8];
        pic.extend(b"image/jpeg\0");
        pic.push(3);
        pic.extend(b"front\0");
        pic.extend(JPEG);
        let tag = id3_tag(3, &[id3_frame(3, "TIT2", &text(0, "Wav Song")), id3_frame(3, "APIC", &pic)], 0);
        let mut f = b"RIFF\0\0\0\0WAVE".to_vec();
        f.extend(b"data");
        f.extend(1000u32.to_le_bytes());
        f.extend(vec![0u8; 1000]);
        f.extend(b"id3 ");
        f.extend((tag.len() as u32).to_le_bytes());
        f.extend(&tag);
        if tag.len() % 2 == 1 {
            f.push(0);
        }
        let (t, art) = wav(&mut Cursor::new(f.clone()), true).unwrap();
        assert_eq!(t.title, "Wav Song");
        let art = art.expect("cover from the WAV");
        assert_eq!((art.mime, art.data.as_slice()), ("image/jpeg", &JPEG[..]));
        // not read unless asked for
        assert!(wav(&mut Cursor::new(f), false).unwrap().1.is_none());
    }

    fn wav_pic_tag() -> Vec<u8> {
        let mut pic = vec![0u8];
        pic.extend(b"image/jpeg\0");
        pic.push(3);
        pic.extend(b"front\0");
        pic.extend(JPEG);
        id3_tag(3, &[id3_frame(3, "TIT2", &text(0, "Odd Wav")), id3_frame(3, "APIC", &pic)], 0)
    }

    #[test]
    fn wav_picture_after_the_audio_without_a_chunk_is_found() {
        let mut f = b"RIFF\0\0\0\0WAVE".to_vec();
        f.extend(b"data");
        f.extend(500u32.to_le_bytes());
        f.extend(vec![0u8; 500]);
        f.extend(wav_pic_tag());
        let art = wav(&mut Cursor::new(f), true).unwrap().1.expect("cover found by the wider search");
        assert_eq!(art.data.as_slice(), &JPEG[..]);
    }

    #[test]
    fn wav_with_a_wrong_audio_length_still_gives_its_picture() {
        let tag = wav_pic_tag();
        let mut f = b"RIFF\0\0\0\0WAVE".to_vec();
        f.extend(b"data");
        f.extend(0xFFFF_FFFFu32.to_le_bytes()); // streaming writers leave this unknown
        f.extend(vec![0u8; 800]);
        f.extend(b"id3 ");
        f.extend((tag.len() as u32).to_le_bytes());
        f.extend(&tag);
        let art = wav(&mut Cursor::new(f), true).unwrap().1.expect("cover found by the wider search");
        assert_eq!(art.data.as_slice(), &JPEG[..]);
    }

    fn atom(name: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend(name);
        v.extend(body);
        v
    }

    fn data_atom(flags: u8, payload: &[u8]) -> Vec<u8> {
        let mut b = vec![0, 0, 0, flags, 0, 0, 0, 0];
        b.extend(payload);
        atom(b"data", &b)
    }

    #[test]
    fn mp4_ilst_tags_and_cover_with_moov_after_the_audio() {
        let ilst = [
            atom(&[0xA9, b'n', b'a', b'm'], &data_atom(1, b"Radio Check")),
            atom(&[0xA9, b'A', b'R', b'T'], &data_atom(1, b"Ham Band")),
            atom(&[0xA9, b'a', b'l', b'b'], &data_atom(1, b"Frequencies")),
            atom(&[0xA9, b'd', b'a', b'y'], &data_atom(1, b"2015")),
            atom(b"trkn", &data_atom(0, &[0, 0, 0, 5, 0, 9, 0, 0])),
            atom(b"covr", &data_atom(13, &JPEG)),
        ]
        .concat();
        let mut meta = vec![0, 0, 0, 0];
        meta.extend(atom(b"ilst", &ilst));
        let moov = atom(b"moov", &atom(b"udta", &atom(b"meta", &meta)));
        let mut f = atom(b"ftyp", b"M4A \0\0\0\0");
        f.extend(atom(b"mdat", &vec![7u8; 5000]));
        f.extend(moov);
        let (t, art) = mp4(&mut Cursor::new(f), true).unwrap();
        assert_eq!(t, Tags { title: "Radio Check".into(), artist: "Ham Band".into(), album: "Frequencies".into(), track: 5, year: 2015, genre: String::new() });
        assert_eq!(art.unwrap().mime, "image/jpeg");
    }

    fn utf16z(s: &str) -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        v.extend([0, 0]);
        v
    }

    #[test]
    fn wma_asf_content_and_extended_description() {
        let (t, a) = (utf16z("Windows Song"), utf16z("WMA Band"));
        let mut content = Vec::new();
        for n in [t.len(), a.len(), 0, 0, 0] {
            content.extend((n as u16).to_le_bytes());
        }
        content.extend(&t);
        content.extend(&a);
        let mut ext = Vec::new();
        let items: Vec<(&str, u16, Vec<u8>)> = vec![
            ("WM/AlbumTitle", 0, utf16z("Media Player")),
            ("WM/TrackNumber", 0, utf16z("6")),
            ("WM/Year", 0, utf16z("2007")),
        ];
        ext.extend((items.len() as u16).to_le_bytes());
        for (name, ty, val) in &items {
            let n = utf16z(name);
            ext.extend((n.len() as u16).to_le_bytes());
            ext.extend(&n);
            ext.extend(ty.to_le_bytes());
            ext.extend((val.len() as u16).to_le_bytes());
            ext.extend(val);
        }
        let obj = |guid: &[u8; 16], body: &[u8]| {
            let mut v = guid.to_vec();
            v.extend(((body.len() + 24) as u64).to_le_bytes());
            v.extend(body);
            v
        };
        let objs = [obj(&ASF_CONTENT, &content), obj(&ASF_EXTENDED, &ext)].concat();
        let mut f = ASF_HEADER.to_vec();
        f.extend(((objs.len() + 30) as u64).to_le_bytes());
        f.extend(2u32.to_le_bytes());
        f.extend([1, 2]);
        f.extend(&objs);
        let tags = asf(&mut Cursor::new(f)).unwrap();
        assert_eq!(tags, Tags { title: "Windows Song".into(), artist: "WMA Band".into(), album: "Media Player".into(), track: 6, year: 2007, genre: String::new() });
    }

    #[test]
    fn unknown_and_truncated_files_give_empty_tags() {
        let d = std::env::temp_dir().join(format!("cindrix-tags-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for (name, bytes) in [("a.mp3", &b"not really audio"[..]), ("b.flac", b"fLaC"), ("c.wav", b"RIFF"), ("d.m4a", b"\0\0\0\x08ftyp"), ("e.wma", &ASF_HEADER[..]), ("f.ogg", b"OggS")] {
            std::fs::write(d.join(name), bytes).unwrap();
            assert!(read_tags(&d.join(name)).is_empty(), "{}", name);
            assert!(read_art(&d.join(name)).is_none(), "{}", name);
        }
        assert!(read_tags(&d.join("missing.mp3")).is_empty());
    }

    #[test]
    fn year_and_track_parsing() {
        assert_eq!(year_of("Recorded 2004-05-01"), 2004);
        assert_eq!(year_of("0000"), 0);
        assert_eq!(year_of("98"), 0);
        assert_eq!(leading_number(" 12/30"), 12);
        assert_eq!(leading_number("x"), 0);
    }

    #[test]
    fn genres_are_read_from_id3_numbers_and_names() {
        assert_eq!(genre_text("(17)"), "Rock");
        assert_eq!(genre_text("(17)Rock"), "Rock");
        assert_eq!(genre_text("17"), "Rock");
        assert_eq!(genre_text("Folk Rock"), "Folk Rock");
        assert_eq!(genre_text("(200)"), "");
        assert_eq!(genre_text(""), "");
        assert_eq!(genre_by_number(32), "Classical");
        let mut t = Tags::default();
        assert!(t.is_empty());
        t.genre = "Jazz".into();
        assert!(!t.is_empty());
        let mut a = Tags::default();
        a.fill(t);
        assert_eq!(a.genre, "Jazz");
    }
}
