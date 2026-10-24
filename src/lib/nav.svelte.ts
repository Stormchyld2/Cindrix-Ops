// A screen asks to open something it only knows by kind and id (a link on a task, a search hit). App.svelte answers.
export const nav: { open: { kind: string; id: string; n: number } | null } = $state({ open: null });
let n = 0;
export function openLink(kind: string, id: string) {
  nav.open = { kind, id, n: ++n };
}
