// Card text is plain Markdown-like lines: paragraphs, "1." steps, "- " lists and **bold**.
// A line that starts with "## " is a heading that is always open; "### " is a heading the reader taps to open.
// The "Source:" line and anything after it is always shown, never folded away.
// It is shown without ever inserting raw HTML.
export function parts(t: string): { b: boolean; t: string }[] {
  return t.split('**').map((x, i) => ({ b: i % 2 === 1, t: x })).filter((x) => x.t);
}

export type TextBlock = { kind: 'p' | 'ol' | 'ul'; lines: string[] };

export function blocks(body: string): TextBlock[] {
  const out: TextBlock[] = [];
  for (const raw of body.split('\n')) {
    const line = raw.trim();
    if (!line) { out.push({ kind: 'p', lines: [] }); continue; }
    const num = /^\d+\.\s+(.*)$/.exec(line);
    const dash = /^[-*]\s+(.*)$/.exec(line);
    const k = num ? 'ol' : dash ? 'ul' : 'p';
    const text = num ? num[1] : dash ? dash[1] : line;
    const last = out[out.length - 1];
    if (last && last.kind === k && last.lines.length && k !== 'p') last.lines.push(text);
    else out.push({ kind: k, lines: [text] });
  }
  return out.filter((b) => b.lines.length);
}

export type Section = { title: string; fold: boolean; blocks: TextBlock[] };

/** Splits a card into its parts. The first part (before any heading) has no title and is always shown. */
export function sections(body: string): Section[] {
  const out: { title: string; fold: boolean; text: string[] }[] = [{ title: '', fold: false, text: [] }];
  for (const raw of body.split('\n')) {
    const h = /^(#{2,3})\s+(.*)$/.exec(raw.trim());
    if (h) out.push({ title: h[2].replace(/\*\*/g, '').trim(), fold: h[1].length === 3, text: [] });
    else if (/^Sources?:/.test(raw.trim()) && out.length > 1) out.push({ title: '', fold: false, text: [raw] });
    else out[out.length - 1].text.push(raw);
  }
  return out
    .map((s) => ({ title: s.title, fold: s.fold, blocks: blocks(s.text.join('\n')) }))
    .filter((s) => s.title || s.blocks.length);
}
