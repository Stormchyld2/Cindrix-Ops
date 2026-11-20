// Makes the content review sheet (docs/62-content-review-sheet.md): every shipped emergency card with its status, and
// every Academy course with its lesson count, as tick boxes for a person to go through. Run: node scripts/review-sheet.mjs
import { readdirSync, readFileSync, writeFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const front = (text) => {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  const o = {};
  if (m) for (const l of m[1].split(/\r?\n/)) { const i = l.indexOf(':'); if (i > 0) o[l.slice(0, i).trim()] = l.slice(i + 1).trim(); }
  return o;
};
const walk = (d) => readdirSync(d).flatMap((n) => { const p = join(d, n); return statSync(p).isDirectory() ? walk(p) : [p]; });

const done = (c) => c.status === 'reviewed' && !!c.reviewer && !!c.reviewed;
const cards = readdirSync('content/cards').filter((n) => n.endsWith('.md') && !n.startsWith('_')).map((n) => ({ n, ...front(readFileSync(join('content/cards', n), 'utf8')) }));
const kinds = {};
for (const c of cards) (kinds[c.kind || '?'] ??= []).push(c);

let out = '# 62. Content review sheet\n\nMade by `node scripts/review-sheet.mjs`. A card counts as reviewed when its front matter has `status: reviewed`, a `reviewer:` and a `reviewed:` date (docs/27). Tick here as you go, then put the same three lines in the card file.\n\n';
out += `Cards: ${cards.length}. Reviewed: ${cards.filter(done).length}. Still draft: ${cards.filter((c) => !done(c)).length}.\n\n`;
for (const k of Object.keys(kinds).sort()) {
  out += `## Cards: ${k} (${kinds[k].length})\n\n`;
  for (const c of kinds[k].sort((a, b) => a.n.localeCompare(b.n))) out += `- [${done(c) ? 'x' : ' '}] \`${c.n}\` ${c.title || ''} (${c.status || 'no status'})\n`;
  out += '\n';
}
out += '## Academy courses\n\nLessons are counted from the `.md` files that are not module headings. Review each course against its sources and the no-doses rule.\n\n';
for (const d of readdirSync('content/academy').filter((n) => statSync(join('content/academy', n)).isDirectory() && !n.startsWith('_'))) {
  const files = walk(join('content/academy', d)).filter((p) => p.endsWith('.md'));
  const lessons = files.filter((p) => !/(^|[\\/])(course|_module)\.md$/.test(p));
  out += `- [ ] \`${d}\`: ${lessons.length} lessons\n`;
}
writeFileSync('docs/62-content-review-sheet.md', out + '\n');
console.log(`cards ${cards.length}, courses written`);
