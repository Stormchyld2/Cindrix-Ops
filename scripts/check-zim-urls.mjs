// Guards the reader's address building (docs/53-zim-reader-compatibility.md).
// convertFileSrc('x', 'cindrix-zim') is "http://cindrix-zim.localhost/x" on Windows and Android
// and "cindrix-zim://localhost/x" elsewhere. The origin must come out WITHOUT a trailing slash, so
// that origin + "/content/<book>" has a single slash (a doubled slash is an empty 404 in the reader).
import { readFileSync } from 'node:fs';

const src = readFileSync(new URL('../src/lib/api.ts', import.meta.url), 'utf8');
let bad = 0;
const fail = (m) => { console.error('FAIL: ' + m); bad++; };

// the cut is taken from the source itself, so a change to it is tested
const m = src.match(/return convertFileSrc\('x', 'cindrix-zim'\)(\.[^;]+);/);
if (!m) fail('zimOrigin no longer returns convertFileSrc(\'x\', \'cindrix-zim\')... in the form this check knows');
if (/convertFileSrc\('x', 'cindrix-zim'\)\.slice\(0, -1\)/.test(src)) fail('slice(0, -1) leaves the trailing slash (the doubled-slash bug)');

if (m) {
  const cut = new Function('s', `return s${m[1]};`);
  for (const base of ['http://cindrix-zim.localhost/x', 'cindrix-zim://localhost/x']) {
    const origin = cut(base);
    if (origin.endsWith('/')) fail(`origin "${origin}" ends with a slash`);
    const url = origin + '/content/libretexts.org_en_bio_2025-01';
    const path = new URL(url).pathname;
    if (path !== '/content/libretexts.org_en_bio_2025-01') fail(`"${url}" has the path "${path}"`);
    const sug = new URL(`${origin}/suggest?content=b&term=t`).pathname;
    if (sug !== '/suggest') fail(`suggest path is "${sug}"`);
  }
}
if (bad) process.exit(1);
console.log('reader addresses ok');
