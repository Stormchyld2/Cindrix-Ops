// The Ask conversation lives here, not inside the Ask screen, so it is still there when you look at
// another screen and come back. Each answered chat is also saved as a small text file under user/chats.
import type { Reply } from './api';

export type Msg = { who: 'you' | 'app'; text: string; reply?: Reply };

export const chat: { thread: Msg[]; file: string; busy: boolean; question: string } = $state({ thread: [], file: '', busy: false, question: '' });

const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40) || 'chat';

/** A new file name from the first question and the time now. */
export function newChatFile(first: string): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}-${slug(first)}.txt`;
}

/** The chat as plain text anyone can read. */
export function chatText(thread: Msg[]): string {
  const out: string[] = ['CINDRIX ASK (saved chat)', ''];
  for (const m of thread) {
    out.push((m.who === 'you' ? 'YOU: ' : 'CINDRIX: ') + m.text);
    if (m.reply?.sources.length) out.push('Sources: ' + m.reply.sources.join('; '));
    out.push('');
  }
  return out.join('\r\n');
}

export function resetChat() {
  chat.thread = [];
  chat.file = '';
  chat.question = '';
}

// Intel opens a page when another screen asks it to (an Ask source, for example).
export const intelRequest: { path: string } = $state({ path: '' });
