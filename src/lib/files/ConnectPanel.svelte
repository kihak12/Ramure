<script lang="ts">
  // What the connection of a host pane says or asks, in a panel over its list, with the words, the
  // buttons and the focus of the panels of a terminal (guidelines, 9.15 and 9.24).
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { keyringAvailable, NO_KEYRING } from '../ssh/secrets';
  import { files, type Pane } from './files.svelte';

  let { pane }: { pane: Pane } = $props();

  const phase = $derived(pane.phase);
  const host = $derived(files.host(pane));
  const target = $derived(files.address(pane));

  let secret = $state('');
  let keep = $state(false);
  let keyring = $state<boolean | null>(null);
  let card = $state<HTMLElement>();

  // The panel gives the keyboard to its field, or to its main button, as soon as it shows: a
  // panel that asks nothing keeps it out of the way (guidelines, 9.15).
  $effect(() => {
    const step = phase.step;
    if (step === 'passphrase' || step === 'password') {
      secret = '';
      void keyringAvailable().then((ok) => (keyring = ok));
    }
    if (step === 'connecting' || step === 'ready') return;
    void tick().then(() => {
      // The keyboard is taken from nobody: only when it was in this pane, or nowhere (the row or
      // the button just clicked has gone).
      const active = document.activeElement;
      const lost = !active || active === document.body;
      if (!card || (!lost && !card.closest('[data-pane]')?.contains(active))) return;
      const first = card.querySelector<HTMLElement>('input[type="password"], .btn.primary');
      if (first instanceof HTMLInputElement) first.focus();
      else focusWithRing(first ?? undefined);
    });
  });

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const kept = keep && keyring === true;
    if (phase.step === 'passphrase') void files.connect(pane, { keyPath: phase.path, passphrase: secret, keep: kept });
    else if (phase.step === 'password') void files.connect(pane, { password: secret, keep: kept });
  }
</script>

<div class="veil">
  <div class="card" class:bad={phase.step === 'hostKeyChanged'} bind:this={card}>
    {#if phase.step === 'connecting'}
      <span class="spinner wait" aria-hidden="true"></span>
      <p class="lead" role="status">Connecting to <b class="mono">{target}</b>…</p>
    {:else if phase.step === 'idle'}
      <h3>Not connected</h3>
      <p class="muted">{host?.alias ?? 'This host'} · <span class="mono">{target}</span></p>
      <div class="row">
        <button class="btn primary" type="button" onclick={() => void files.connect(pane)}>Connect</button>
        <button class="btn ghost" type="button" onclick={() => files.unchoose(pane)}>Choose another location</button>
      </div>
    {:else if phase.step === 'hostKey'}
      <h3><Icon name="warning" /> Unknown server</h3>
      <p>
        <b class="mono">{host?.hostname}</b> is not in <code>known_hosts</code>. Check that this {phase.algorithm} fingerprint is the server’s before you
        trust it:
      </p>
      <p class="print mono">{phase.fingerprint}</p>
      <div class="row">
        <button class="btn primary" type="button" onclick={() => phase.step === 'hostKey' && void files.connect(pane, { accept: phase.fingerprint, remember: true })}>
          Trust and remember
        </button>
        <button class="btn" type="button" onclick={() => phase.step === 'hostKey' && void files.connect(pane, { accept: phase.fingerprint, remember: false })}>
          Trust this time
        </button>
        <button class="btn ghost" type="button" onclick={() => files.unchoose(pane)}>Cancel</button>
      </div>
    {:else if phase.step === 'hostKeyChanged'}
      <h3><Icon name="warning" /> Server fingerprint changed</h3>
      <p>
        The fingerprint presented by <b class="mono">{host?.hostname}</b> no longer matches the one saved on line {phase.line} of
        <code>known_hosts</code>. The connection is blocked: the server may have been reinstalled, or someone may be intercepting the connection.
      </p>
      <p class="print mono">{phase.fingerprint}</p>
      <p class="muted">If the change is legitimate, remove the old line from <code>known_hosts</code>, then connect again.</p>
      <div class="row">
        <button class="btn" type="button" onclick={() => void files.connect(pane)}>Try again</button>
        <button class="btn ghost" type="button" onclick={() => files.unchoose(pane)}>Choose another location</button>
      </div>
    {:else if phase.step === 'passphrase' || phase.step === 'password'}
      <form onsubmit={submit}>
        <h3>{phase.step === 'passphrase' ? 'Key passphrase' : `Password for ${host?.user || 'this account'}`}</h3>
        <p class="muted">
          {#if phase.step === 'passphrase'}
            The key <span class="mono">{phase.path}</span> is protected.
          {:else}
            For <span class="mono">{target}</span>.
          {/if}
        </p>
        <div class="secret">
          <input
            class="field"
            class:invalid={!!phase.error}
            type="password"
            autocomplete="off"
            aria-label={phase.step === 'passphrase' ? 'Passphrase' : 'Password'}
            aria-invalid={phase.error ? 'true' : undefined}
            aria-describedby={phase.error ? `files-secret-error-${pane.key}` : undefined}
            bind:value={secret}
          />
          {#if phase.error}<p class="err small" id="files-secret-error-{pane.key}" role="alert">{phase.error}</p>{/if}
        </div>
        {#if keyring}
          <label class="keep">
            <input type="checkbox" bind:checked={keep} />
            <span>Remember in the system keyring</span>
          </label>
        {:else if keyring === false}
          <p class="muted">{NO_KEYRING[phase.step]}</p>
        {/if}
        <div class="row">
          <button class="btn primary" type="submit" disabled={!secret}>Connect</button>
          <button class="btn ghost" type="button" onclick={() => files.unchoose(pane)}>Cancel</button>
        </div>
      </form>
    {:else if phase.step === 'failed' || phase.step === 'lost'}
      <h3><Icon name="warning" /> {phase.step === 'lost' ? 'Connection lost' : 'Connection failed'}</h3>
      <p class:err={phase.step === 'failed'} role="alert">
        {phase.step === 'lost' ? `The connection to ${target} was lost: the network was cut, or the server stopped answering.` : phase.message}
      </p>
      <div class="row">
        <button class="btn primary" type="button" onclick={() => void files.connect(pane)}><Icon name="refresh" size={14} /> Connect</button>
        <button class="btn ghost" type="button" onclick={() => files.unchoose(pane)}>Choose another location</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    padding: 12px;
    overflow: auto;
    background: color-mix(in srgb, var(--panel) 82%, transparent);
    backdrop-filter: blur(1.5px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .card {
    margin: auto;
    width: min(360px, 100%);
    display: grid;
    gap: 10px;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    animation: pop-in var(--t-med) var(--ease);
  }
  .card.bad {
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  form {
    display: grid;
    gap: 10px;
  }
  .keep {
    justify-self: start;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .lead {
    text-align: center;
  }
  .muted {
    color: var(--muted);
  }
  .err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .err.small {
    font-size: var(--fs-small);
  }
  .secret {
    display: grid;
    gap: 4px;
  }
  .print {
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: var(--fs-code);
    user-select: all;
  }
  code {
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .wait {
    justify-self: center;
  }
</style>
