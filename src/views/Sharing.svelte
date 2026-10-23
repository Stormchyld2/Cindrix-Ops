<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { shareInfo, shareSet, shareSetPin, shareInvite, shareTrusted, shareRevoke, type ShareInfo, type Invite, type TrustedDevice } from '../lib/api';
  import { qrMatrix } from '../lib/qr';
  import { everyone, loadPersonList } from '../lib/person.svelte';
  import { learner } from '../lib/learner.svelte';

  let info = $state<ShareInfo | null>(null);
  let error = $state('');
  let pinText = $state('');
  let busy = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try { info = await shareInfo(); } catch (e) { error = String(e); }
  }
  onMount(() => { void refresh(); void loadPersonList(); void loadTrusted(); timer = setInterval(() => { void refresh(); void loadTrusted(); }, 3000); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function toggle() {
    if (!info || busy) return;
    busy = true; error = '';
    try { info = await shareSet(!info.on); } catch (e) { error = String(e); }
    busy = false;
  }
  async function setPin(value: string) {
    error = '';
    try { info = await shareSetPin(value); pinText = ''; } catch (e) { error = String(e); }
  }

  function qrFor(url: string | undefined) {
    if (!url) return null;
    try {
      const m = qrMatrix(url);
      let d = '';
      m.forEach((row, y) => row.forEach((v, x) => { if (v) d += `M${x + 4} ${y + 4}h1v1h-1z`; }));
      return { size: m.length + 8, d };
    } catch { return null; }
  }
  const main = $derived(info?.addresses[0]);
  const code = $derived(qrFor(main?.url));

  // family members: an invite code works once and makes the phone that scans it a trusted device
  let trusted = $state<TrustedDevice[]>([]);
  let invitee = $state('');
  let invite = $state<Invite | null>(null);
  const inviteCode = $derived(qrFor(invite?.url));
  async function loadTrusted() { try { trusted = await shareTrusted(); } catch { /* the list just stays as it was */ } }
  async function makeInvite() {
    error = '';
    try { invite = await shareInvite(invitee || everyone()[0]); } catch (e) { error = String(e); }
  }
  async function removeDevice(id: string) {
    try { trusted = await shareRevoke(id); } catch (e) { error = String(e); }
  }
  const when = (t: number) => new Date(t * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
</script>

<div class="panel">
  <div class="label">SHARE THE LIBRARY WITH OTHER DEVICES</div>
  {#if !info}
    <p class="muted">Looking…</p>
  {:else}
    <p>
      {#if info.on}
        <b class="on">Sharing is on</b>{info.reason === 'hotspot' ? ', switched on by your hotspot.' : ', switched on by you.'}
      {:else}
        <b>Sharing is off.</b> {info.hotspot_up ? 'Your hotspot is on, so you can switch it on here.' : 'It switches on by itself when you start a hotspot.'}
      {/if}
    </p>
    <div class="row">
      <button class="btn primary" disabled={busy} onclick={toggle} title={info.on ? 'Stop sharing. Other devices lose the connection.' : 'Let other devices on this Wi-Fi or hotspot read the library'}>{info.on ? 'Turn sharing off' : 'Turn sharing on'}</button>
    </div>

    {#if info.on}
      <div class="how">
        <p><b>On the other device:</b> join this computer's or phone's Wi-Fi or hotspot, then scan the code or type the address in a web browser. It asks for the PIN once. Anyone who types the PIN is a guest and can only read.</p>
        <div class="join">
          {#if code}
            <svg class="qr" viewBox="0 0 {code.size} {code.size}" role="img" aria-label="Code to scan for {main?.url}"><rect width={code.size} height={code.size} fill="#fff" /><path d={code.d} fill="#000" /></svg>
          {/if}
          <div>
            <div class="small muted">Address{info.addresses.length > 1 ? 'es (try the first one)' : ''}</div>
            {#each info.addresses as a}
              <div class="addr"><code>{a.url}</code> <span class="small muted">{a.label}{a.hotspot ? ' (your hotspot)' : ''}</span></div>
            {:else}
              <p class="muted">No network address was found. Turn on the hotspot, or join a Wi-Fi network.</p>
            {/each}
            <div class="small muted pinlabel">PIN</div>
            <div class="pin">{info.pin}</div>
            <div class="small muted">{info.signed_in} guest{info.signed_in === 1 ? '' : 's'} signed in</div>
          </div>
        </div>
      </div>

      <div class="family">
        <div class="label sub">FAMILY DEVICES</div>
        <p class="small">A family member's own phone or laptop can join with an invite instead of the PIN. It then stays trusted, shows their name, and lets them add pictures to the shared album and to their own. Make the invite for the right person, and have them scan it within {invite?.minutes ?? 10} minutes. It works once.</p>
        <div class="row">
          <select class="input" bind:value={invitee} aria-label="Who is this invite for">
            {#each everyone() as n}<option value={n}>{n}</option>{/each}
          </select>
          <button class="btn primary" onclick={makeInvite} title="Make a one-time code for this person's phone">Make an invite</button>
        </div>
        {#if learner.locked}<p class="small muted">The Family page is locked, so only “Me” is listed. Unlock it on the Family page to invite someone else.</p>{/if}
        {#if invite && inviteCode}
          <div class="join">
            <svg class="qr" viewBox="0 0 {inviteCode.size} {inviteCode.size}" role="img" aria-label="Invite code for {invite.person}"><rect width={inviteCode.size} height={inviteCode.size} fill="#fff" /><path d={inviteCode.d} fill="#000" /></svg>
            <div>
              <p><b>Invite for {invite.person}</b></p>
              <p class="small muted">Have them scan this with their phone's camera while it is on this Wi-Fi or hotspot. It works once and runs out after {invite.minutes} minutes. Do not post or send a picture of it.</p>
              <button class="btn" onclick={() => (invite = null)}>Hide the invite</button>
            </div>
          </div>
        {/if}
        <div class="small muted pinlabel">Trusted devices</div>
        {#each trusted as t (t.id)}
          <div class="line"><div class="grow"><b>{t.person}</b> <span class="small muted">joined {when(t.since)}</span></div><button class="btn" onclick={() => removeDevice(t.id)} title="This device will have to use the PIN again, as a guest">Remove</button></div>
        {:else}
          <p class="small muted">No family device has joined yet.</p>
        {/each}
      </div>
    {/if}

    <div class="setpin">
      <label for="newpin"><b>PIN</b> <span class="small muted">4 to 8 digits. Choosing a new one signs everyone out.</span></label>
      <div class="row">
        <input id="newpin" class="input" inputmode="numeric" maxlength="8" placeholder={info.pin} bind:value={pinText} />
        <button class="btn" onclick={() => setPin(pinText)} disabled={!pinText.trim()}>Use this PIN</button>
        <button class="btn" onclick={() => setPin('')} title="Make a new random PIN">New random PIN</button>
      </div>
    </div>
    {#if error}<div class="warn" role="alert">{error}</div>{/if}

    <details class="more">
      <summary>What guests can and cannot do</summary>
      <ul>
        <li>A PIN makes a guest. Guests can read: archives, the shelf, emergency cards, Academy, plans, drills, maps, music and Ask.</li>
        <li>Guests cannot change anything, and cannot see your family page, inventory, notes, chats or My details.</li>
        <li>A family member who joined with an invite can also add and remove pictures in the shared album and in their own. Nothing else changes: the journal, family page and inventory stay on this computer.</li>
        <li>Invites only travel over your own Wi-Fi or hotspot, but that network is not scrambled. Make an invite only when the person is with you, and remove a device you no longer trust.</li>
        <li>Changing the PIN signs guests out. Family devices stay until you remove them.</li>
        <li>Ask for guests never reads your own notes. Two questions at a time; others wait their turn.</li>
        <li>The first time, Windows may ask whether to let this program use the network. Say yes for private networks.</li>
        <li>Sharing is off whenever the program starts, until a hotspot is on (or you turn it on here).</li>
      </ul>
    </details>
  {/if}
</div>

<style>
  .on { color: #6fcf8d; }
  .how { margin-top: 14px; }
  .join { display: flex; gap: 20px; flex-wrap: wrap; align-items: flex-start; }
  .qr { width: 190px; height: 190px; border-radius: 8px; background: #fff; flex: none; }
  .addr { margin: 4px 0; word-break: break-all; }
  .addr code { font-size: 16px; }
  .pinlabel { margin-top: 12px; }
  .pin { font-size: 34px; letter-spacing: .25em; font-weight: 700; }
  .setpin { margin-top: 16px; display: flex; flex-direction: column; gap: 6px; }
  .more { margin-top: 14px; }
  .more summary { cursor: pointer; font-weight: 600; }
  .more ul { margin: 8px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; }
  .small { font-size: 12px; }
  .family { margin-top: 18px; padding-top: 12px; border-top: 1px solid var(--line); display: flex; flex-direction: column; gap: 8px; }
  .line { display: flex; gap: 12px; align-items: center; padding: 6px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; }
</style>
