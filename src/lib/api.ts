// Talks to the Rust core. In a plain browser (npm run dev without Tauri) it falls back to samples,
// so the screens can be previewed.

export type Card = { id: string; title: string; kind: string; status: string; reviewer: string; reviewed: string; body: string };
export type Reply = { kind: 'Refused' | 'NotFound' | 'Answered' | 'Replaced'; text: string; sources: string[]; card: string | null };

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

export async function listCards(): Promise<Card[]> {
  if (inTauri) return call<Card[]>('list_cards');
  return [{ id: 'low-blood-sugar', title: 'Low blood sugar', kind: 'emergency', status: 'draft', reviewer: '', reviewed: '',
    body: 'Browser preview only. Run the desktop app to see the real cards.' }];
}

export async function ask(question: string): Promise<Reply> {
  if (inTauri) return call<Reply>('ask', { question });
  return { kind: 'NotFound', text: 'Browser preview only. Run the desktop app to ask the library.', sources: [], card: null };
}
