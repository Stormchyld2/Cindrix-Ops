// Gathers the license files of the libraries that end up inside the program.
//   node scripts/collect-licenses.mjs <folder>        (default: licenses/third-party)
// 1. Interface libraries: every package in package-lock.json that is not a development tool
//    has its LICENSE / NOTICE file copied from node_modules (run `npm install` first).
// 2. Rust libraries: `cargo metadata` lists each crate and its license; written as rust-crates.csv.
// Nothing here uses the internet. A missing piece is reported and skipped, never invented.
import { existsSync, mkdirSync, readdirSync, readFileSync, copyFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = process.argv[2] ? process.argv[2] : join(root, 'licenses', 'third-party');
mkdirSync(out, { recursive: true });
const notes = [];

// ---- interface libraries ----
const lockFile = join(root, 'package-lock.json');
let copied = 0;
if (existsSync(lockFile)) {
  const lock = JSON.parse(readFileSync(lockFile, 'utf8'));
  const rows = ['name,version,license'];
  for (const [path, info] of Object.entries(lock.packages ?? {})) {
    if (!path || info.dev || info.devOptional) continue; // development tools are not shipped
    const name = path.replace(/^.*node_modules\//, '');
    const dir = join(root, path);
    rows.push([name, info.version ?? '', String(info.license ?? '').replace(/,/g, ' ')].join(','));
    if (!existsSync(dir)) { notes.push(`not installed, skipped: ${name}`); continue; }
    for (const f of readdirSync(dir)) {
      if (!/^(licen[cs]e|notice|copying)([.-].*)?$/i.test(f)) continue;
      const safe = name.replace(/[^A-Za-z0-9._-]/g, '_');
      copyFileSync(join(dir, f), join(out, `npm-${safe}-${f}`.replace(/[^A-Za-z0-9._-]/g, '_')));
      copied++;
    }
  }
  writeFileSync(join(out, 'npm-packages.csv'), rows.join('\n') + '\n');
} else {
  notes.push('package-lock.json not found: interface libraries skipped');
}

// ---- Rust libraries ----
let crates = 0;
const meta = spawnSync('cargo', ['metadata', '--format-version', '1', '--manifest-path', join(root, 'src-tauri', 'Cargo.toml')], { encoding: 'utf8', maxBuffer: 1 << 28 });
if (meta.status === 0) {
  const m = JSON.parse(meta.stdout);
  const rows = ['name,version,license'];
  for (const p of m.packages.sort((a, b) => a.name.localeCompare(b.name))) {
    rows.push([p.name, p.version, String(p.license ?? 'see its repository').replace(/,/g, ' ')].join(','));
    crates++;
  }
  writeFileSync(join(out, 'rust-crates.csv'), rows.join('\n') + '\n');
} else {
  notes.push('cargo metadata did not run (is Rust installed and online once?): Rust libraries skipped');
}

console.log(`license files copied: ${copied}; Rust crates listed: ${crates}`);
for (const n of notes) console.log('note: ' + n);
