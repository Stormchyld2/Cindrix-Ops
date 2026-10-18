<script lang="ts">
  import { listCards, ask, type Card, type Reply } from './lib/api';

  type Group = 'Ask' | 'Library' | 'Maps' | 'Plan' | 'Settings';
  const groups: Group[] = ['Ask', 'Library', 'Maps', 'Plan', 'Settings'];
  let group: Group = $state('Ask');

  let cards: Card[] = $state([]);
  let openCard: Card | null = $state(null);
  let question = $state('');
  let thread: { who: 'you' | 'app'; text: string; reply?: Reply }[] = $state([]);
  let busy = $state(false);

  $effect(() => { listCards().then((c) => (cards = c)); });

  async function send() {
    const q = question.trim();
    if (!q || busy) return;
    question = '';
    thread.push({ who: 'you', text: q });
    busy = true;
    const reply = await ask(q);
    thread.push({ who: 'app', text: reply.text, reply });
    busy = false;
    if (reply.card) openCard = cards.find((c) => c.id === reply.card) ?? null;
  }

  function showCard(id: string) {
    openCard = cards.find((c) => c.id === id) ?? null;
    group = 'Library';
  }
</script>

<div class="shell">
  <header>
    <div class="brand">
      <svg width="30" height="30" viewBox="0 0 24 24" aria-hidden="true">
        <defs><linearGradient id="fl" x1="0" y1="1" x2="0" y2="0"><stop offset="0" stop-color="#ff5a1f" /><stop offset="1" stop-color="#ffc247" /></linearGradient></defs>
        <path d="M12 2.2c.9 3.3 4.7 5.3 4.7 9.9a4.7 4.7 0 0 1-9.4 0c0-2.1 1-3.4 2.1-4.5.2 1.5.9 2.2 1.7 2.4-.4-2.8-.2-5.4.9-7.8z" fill="url(#fl)" />
      </svg>
      <div>
        <div class="name">CINDRIX <span>OPS</span></div>
        <div class="tag">Carry the library. Keep the fire.</div>
      </div>
    </div>
    <nav aria-label="Sections">
      {#each groups as g}
        <button class:on={group === g} onclick={() => (group = g)} aria-current={group === g ? 'page' : undefined}>{g}</button>
      {/each}
    </nav>
  </header>

  <main>
    {#if group === 'Ask'}
      <section class="chat">
        {#if thread.length === 0}
          <p class="muted">Ask a question. Answers come only from the library on this drive and show where they came from.</p>
        {/if}
        {#each thread as m}
          <div class="msg {m.who}">
            <div class="who">{m.who === 'you' ? 'YOU' : 'CINDRIX'}</div>
            <div class="text">{m.text}</div>
            {#if m.reply?.sources.length}
              <div class="sources">Sources: {#each m.reply.sources as s}<span class="chip">{s}</span>{/each}</div>
            {/if}
            {#if m.reply?.card}
              <button class="link" onclick={() => showCard(m.reply!.card!)}>Open the emergency card</button>
            {/if}
          </div>
        {/each}
      </section>
      <form class="input" onsubmit={(e) => { e.preventDefault(); send(); }}>
        <label class="sr" for="q">Question</label>
        <input id="q" bind:value={question} placeholder="Ask anything. It stays on this device." autocomplete="off" />
        <button class="primary" type="submit" disabled={busy}>Send</button>
      </form>
    {:else if group === 'Library'}
      <section class="library">
        <h1>Emergency cards</h1>
        <div class="cardlist">
          {#each cards as c}
            <button class="card" class:open={openCard?.id === c.id} onclick={() => (openCard = c)}>
              <strong>{c.title}</strong>
              {#if c.status !== 'reviewed'}<span class="badge">Draft · clinical review pending</span>{/if}
            </button>
          {/each}
        </div>
        {#if openCard}
          <article class="open">
            <h2>{openCard.title}</h2>
            <pre>{openCard.body}</pre>
          </article>
        {/if}
      </section>
    {:else}
      <section class="soon">
        <h1>{group}</h1>
        <p class="muted">Not built yet. See the mockups in the repository for the plan.</p>
      </section>
    {/if}
  </main>

  <footer>
    <span><b class="dot ok">●</b> Library on this drive</span>
    <span>Model: not connected yet</span>
    <span>This app only talks to itself on this device.</span>
    <span class="right">First slice · sample content</span>
  </footer>
</div>

<style>
  .shell { min-height: 100vh; display: flex; flex-direction: column; }
  header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px 24px;
    padding: 12px 24px; background: var(--panel2); border-bottom: 1px solid var(--line); }
  .brand { display: flex; gap: 12px; align-items: center; }
  .name { font-family: var(--display); font-weight: 700; font-size: 24px; letter-spacing: .12em; line-height: 1; }
  .name span { color: var(--accent); }
  .tag { font-size: 12px; color: var(--muted); margin-top: 3px; }
  nav { display: flex; flex-wrap: wrap; gap: 6px; }
  nav button { padding: 8px 14px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: #b5ada0; font-weight: 500; }
  nav button.on { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-line); font-weight: 600; }
  main { flex: 1; padding: 20px 24px; display: flex; flex-direction: column; gap: 14px; max-width: 980px; width: 100%; margin: 0 auto; }
  .muted { color: var(--muted); }
  .chat { display: flex; flex-direction: column; gap: 14px; flex: 1; }
  .msg { padding: 12px 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); max-width: 760px; }
  .msg.you { align-self: flex-end; background: #27231e; border-color: #3a342c; }
  .msg.app { border-left: 3px solid var(--accent); }
  .who { font-family: var(--mono); font-size: 11px; letter-spacing: .1em; color: var(--muted); }
  .text { margin-top: 4px; white-space: pre-wrap; }
  .sources { margin-top: 10px; font-size: 12px; color: var(--muted); display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .chip { padding: 3px 10px; border-radius: 999px; background: #1f2a1d; border: 1px solid #34502f; color: #9fd18f; }
  .link { margin-top: 8px; background: none; border: 0; color: var(--accent); padding: 0; text-decoration: underline; }
  .input { display: flex; gap: 10px; }
  .input input { flex: 1; padding: 13px 14px; border-radius: 10px; border: 1px solid var(--line2); background: var(--panel2); color: var(--text); font: inherit; }
  .primary { padding: 13px 24px; border: 0; border-radius: 10px; background: var(--accent); color: #1a1208; font-weight: 700; }
  .primary:disabled { opacity: .6; }
  .sr { position: absolute; left: -9999px; }
  h1 { margin: 0; font-family: var(--display); letter-spacing: .06em; font-weight: 600; font-size: 30px; }
  .cardlist { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; margin-top: 12px; }
  .card { text-align: left; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .card.open { border-color: var(--accent); }
  .badge { align-self: flex-start; padding: 2px 8px; border-radius: 999px; border: 1px solid #6b4a1c; background: #2a1f10; color: #f0b35a; font-size: 11px; }
  article.open { margin-top: 16px; padding: 16px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); }
  article.open h2 { margin: 0 0 8px; font-family: var(--display); letter-spacing: .08em; }
  pre { margin: 0; white-space: pre-wrap; font: inherit; }
  footer { display: flex; flex-wrap: wrap; gap: 6px 22px; padding: 9px 24px; background: var(--panel2); border-top: 1px solid var(--line);
    font-family: var(--mono); font-size: 12px; color: var(--muted); }
  footer .right { margin-left: auto; }
  .dot.ok { color: var(--ok); }
</style>
