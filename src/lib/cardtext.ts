// Card text is plain Markdown-like lines: paragraphs, "1." steps, "- " lists and **bold**.
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
