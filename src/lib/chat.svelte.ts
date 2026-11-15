// The Ask conversation lives here, not inside the Ask screen, so it is still there when you look at
// another screen and come back. Each answered chat is also saved as a small text file under user/chats.
import type { Reply } from './api';

export type Msg = { who: 'you' | 'app'; text: string; reply?: Reply };

export const chat: { thread: Msg[]; file: string; busy: boolean; question: string; project: string } = $state({ thread: [], file: '', busy: false, question: '', project: '' });

const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40) || 'chat';

/** A new file name from the first question and the time now. */
export function newChatFile(first: string): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}-${slug(first)}.txt`;
}

const esc = (t: string) => t.replace(/\r?\n/g, '\\n');
const unesc = (t: string) => t.replace(/\\n/g, '\n');

/** The chat as plain text anyone can read, with each source on its own line so the chat can be opened again. */
export function chatText(thread: Msg[]): string {
  const out: string[] = ['CINDRIX ASK (saved chat)', ''];
  for (const m of thread) {
    out.push((m.who === 'you' ? 'YOU: ' : 'CINDRIX: ') + m.text);
    const r = m.reply;
    if (r?.sources.length) {
      r.sources.forEach((s, i) => out.push('SOURCE: ' + s.replace(/ => /g, ' - ') + ' => ' + (r.links?.[i] ?? '') + ' => ' + esc(r.excerpts?.[i] ?? '')));
    }
    if (r?.card) out.push('CARD: ' + r.card);
    out.push('');
  }
  return out.join('\r\n');
}

/** Read a saved chat back into messages. Older files (a "Sources:" line only) still open, without links. */
export function parseChat(text: string): Msg[] {
  const msgs: Msg[] = [];
  let cur: Msg | null = null;
  const finish = () => { if (cur) { cur.text = cur.text.replace(/\s+$/, ''); msgs.push(cur); cur = null; } };
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith('YOU: ')) { finish(); cur = { who: 'you', text: line.slice(5) }; }
    else if (line.startsWith('CINDRIX: ')) { finish(); cur = { who: 'app', text: line.slice(9), reply: { kind: 'Answered', text: '', sources: [], links: [], excerpts: [], card: null } }; }
    else if (cur?.reply && line.startsWith('SOURCE: ')) {
      const parts = line.slice(8).split(' => ');
      cur.reply.sources.push(parts[0]); cur.reply.links!.push(parts[1] ?? ''); cur.reply.excerpts!.push(unesc(parts.slice(2).join(' => ')));
    } else if (cur?.reply && line.startsWith('Sources: ')) {
      for (const s of line.slice(9).split('; ')) { cur.reply.sources.push(s); cur.reply.links!.push(''); cur.reply.excerpts!.push(''); }
    } else if (cur?.reply && line.startsWith('CARD: ')) cur.reply.card = line.slice(6);
    else if (cur && !line.startsWith('CINDRIX ASK')) cur.text += '\n' + line;
  }
  finish();
  for (const m of msgs) if (m.reply) m.reply.text = m.text;
  return msgs;
}

export function resetChat() {
  chat.thread = [];
  chat.file = '';
  chat.question = '';
}

// Intel opens a page when another screen asks it to (an Ask source, for example).
export const intelRequest: { path: string } = $state({ path: '' });

// An Academy lesson or a Shelf book page that an Ask source asks another screen to open.
export const sourceRequest: { lesson: string; book: { id: string; page: number } | null } = $state({ lesson: '', book: null });

// The top bar's menus open a tab inside Settings, or the Training sign-off inside the Academy.
export const viewRequest: { settingsTab: string; training: boolean } = $state({ settingsTab: '', training: false });

// A Bible place ("JHN.3.16") or a word to look up that an Ask source asks the Bible screen to open.
export const bibleRequest: { ref: string; term: string } = $state({ ref: '', term: '' });
