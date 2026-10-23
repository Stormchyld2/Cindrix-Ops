// A small QR code maker, for the address and PIN shown on the Sharing screen. Text only, error
// correction level L, versions 1 to 4 (up to 78 bytes), which is plenty for a web address. No libraries.

const DATA = [0, 19, 34, 55, 80]; // data codewords for versions 1..4
const ECC = [0, 7, 10, 15, 20]; // error-correction codewords for versions 1..4

function mul(x: number, y: number): number {
  let z = 0;
  for (let i = 7; i >= 0; i--) {
    z = (z << 1) ^ ((z >>> 7) * 0x11d);
    z ^= ((y >>> i) & 1) * x;
  }
  return z & 0xff;
}

function remainder(data: number[], degree: number): number[] {
  const div: number[] = new Array(degree).fill(0);
  div[degree - 1] = 1;
  let root = 1;
  for (let i = 0; i < degree; i++) {
    for (let j = 0; j < degree; j++) {
      div[j] = mul(div[j], root);
      if (j + 1 < degree) div[j] ^= div[j + 1];
    }
    root = mul(root, 2);
  }
  const res: number[] = new Array(degree).fill(0);
  for (const b of data) {
    const f = b ^ (res.shift() as number);
    res.push(0);
    for (let i = 0; i < degree; i++) res[i] ^= mul(div[i], f);
  }
  return res;
}

const bit = (v: number, i: number) => ((v >>> i) & 1) !== 0;

function masked(mask: number, x: number, y: number): boolean {
  switch (mask) {
    case 0: return (x + y) % 2 === 0;
    case 1: return y % 2 === 0;
    case 2: return x % 3 === 0;
    case 3: return (x + y) % 3 === 0;
    case 4: return (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0;
    case 5: return ((x * y) % 2) + ((x * y) % 3) === 0;
    case 6: return ((((x * y) % 2) + ((x * y) % 3)) % 2) === 0;
    default: return ((((x + y) % 2) + ((x * y) % 3)) % 2) === 0;
  }
}

function penalty(m: boolean[][]): number {
  const n = m.length;
  let p = 0;
  const line = (get: (i: number) => boolean) => {
    let run = 1;
    for (let i = 1; i < n; i++) {
      if (get(i) === get(i - 1)) { run++; if (run === 5) p += 3; else if (run > 5) p++; } else run = 1;
    }
    const s = Array.from({ length: n }, (_, i) => (get(i) ? 1 : 0)).join('');
    for (const pat of ['10111010000', '00001011101']) {
      for (let i = s.indexOf(pat); i >= 0; i = s.indexOf(pat, i + 1)) p += 40;
    }
  };
  for (let a = 0; a < n; a++) { line((i) => m[a][i]); line((i) => m[i][a]); }
  for (let y = 0; y < n - 1; y++) for (let x = 0; x < n - 1; x++) if (m[y][x] === m[y][x + 1] && m[y][x] === m[y + 1][x] && m[y][x] === m[y + 1][x + 1]) p += 3;
  const dark = m.flat().filter(Boolean).length;
  p += Math.max(0, Math.ceil(Math.abs(dark * 20 - n * n * 10) / (n * n)) - 1) * 10;
  return p;
}

/** The dark squares of a QR code for `text`. Throws if the text is longer than 78 bytes. */
export function qrMatrix(text: string): boolean[][] {
  const bytes = Array.from(new TextEncoder().encode(text));
  let ver = 0;
  for (let v = 1; v <= 4; v++) if (bytes.length + 2 <= DATA[v]) { ver = v; break; }
  if (!ver) throw new Error('Too long for a code.');
  const size = 17 + 4 * ver;
  // the data: mode (byte), length, bytes, end, padding
  const bits: number[] = [];
  const put = (v: number, n: number) => { for (let i = n - 1; i >= 0; i--) bits.push((v >>> i) & 1); };
  put(4, 4);
  put(bytes.length, 8);
  bytes.forEach((b) => put(b, 8));
  put(0, Math.min(4, DATA[ver] * 8 - bits.length));
  while (bits.length % 8) bits.push(0);
  const data: number[] = [];
  for (let i = 0; i < bits.length; i += 8) data.push(parseInt(bits.slice(i, i + 8).join(''), 2));
  for (let pad = 0xec; data.length < DATA[ver]; pad ^= 0xec ^ 0x11) data.push(pad);
  const all = data.concat(remainder(data, ECC[ver]));

  const mod: boolean[][] = Array.from({ length: size }, () => new Array(size).fill(false));
  const fn: boolean[][] = Array.from({ length: size }, () => new Array(size).fill(false));
  const set = (x: number, y: number, dark: boolean) => { if (x >= 0 && y >= 0 && x < size && y < size) { mod[y][x] = dark; fn[y][x] = true; } };
  for (let i = 0; i < size; i++) { set(6, i, i % 2 === 0); set(i, 6, i % 2 === 0); }
  for (const [cx, cy] of [[3, 3], [size - 4, 3], [3, size - 4]]) {
    for (let dy = -4; dy <= 4; dy++) for (let dx = -4; dx <= 4; dx++) { const d = Math.max(Math.abs(dx), Math.abs(dy)); set(cx + dx, cy + dy, d !== 2 && d !== 4); }
  }
  if (ver >= 2) {
    const c = size - 7;
    for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) set(c + dx, c + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1);
  }
  const formatBits = (mask: number) => {
    const d = (1 << 3) | mask; // level L is 01
    let r = d;
    for (let i = 0; i < 10; i++) r = (r << 1) ^ ((r >>> 9) * 0x537);
    return ((d << 10) | r) ^ 0x5412;
  };
  const drawFormat = (mask: number) => {
    const b = formatBits(mask);
    for (let i = 0; i <= 5; i++) set(8, i, bit(b, i));
    set(8, 7, bit(b, 6)); set(8, 8, bit(b, 7)); set(7, 8, bit(b, 8));
    for (let i = 9; i < 15; i++) set(14 - i, 8, bit(b, i));
    for (let i = 0; i < 8; i++) set(size - 1 - i, 8, bit(b, i));
    for (let i = 8; i < 15; i++) set(8, size - 15 + i, bit(b, i));
    set(8, size - 8, true);
  };
  drawFormat(0); // reserves the places; drawn again once the mask is chosen

  // the data, in two-column strips from the right, upward then downward
  let k = 0;
  for (let right = size - 1; right >= 1; right -= 2) {
    if (right === 6) right = 5;
    for (let v = 0; v < size; v++) {
      for (let j = 0; j < 2; j++) {
        const x = right - j;
        const y = ((right + 1) & 2) === 0 ? size - 1 - v : v;
        if (!fn[y][x] && k < all.length * 8) { mod[y][x] = bit(all[k >>> 3], 7 - (k & 7)); k++; }
      }
    }
  }
  let best = 0;
  let bestScore = Infinity;
  let bestMods: boolean[][] = mod;
  for (let mask = 0; mask < 8; mask++) {
    const m = mod.map((row, y) => row.map((v, x) => (!fn[y][x] && masked(mask, x, y) ? !v : v)));
    const keep = mod.map((r) => r.slice());
    mod.forEach((r, y) => r.forEach((_, x) => { r[x] = m[y][x]; }));
    drawFormat(mask);
    const sc = penalty(mod);
    if (sc < bestScore) { bestScore = sc; best = mask; bestMods = mod.map((r) => r.slice()); }
    mod.forEach((r, y) => r.forEach((_, x) => { r[x] = keep[y][x]; }));
  }
  void best;
  return bestMods;
}
