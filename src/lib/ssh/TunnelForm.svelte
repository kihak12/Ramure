<script lang="ts">
  // The form of a tunnel, opened in place: under the list of the host form, or in the strip of a
  // session (guidelines, 9.15, "Tunnels", and 9.21). It is not a dialog, and not a `<form>` either:
  // in the host form it sits inside the form of the host. Enter in its fields applies the tunnel,
  // not the host.
  import { onMount, tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import Select from '../ui/Select.svelte';
  import { samePort, TUNNEL_KINDS, tunnelSentence, type Tunnel, type TunnelKind } from './types';

  let {
    tunnel = null,
    others,
    applyLabel,
    onapply,
    oncancel,
    strip = false,
    keepable = false,
  }: {
    /** The tunnel being edited; none for a new one. */
    tunnel?: Tunnel | null;
    /** The other tunnels: none may listen on the same port of the same machine. */
    others: Tunnel[];
    applyLabel: string;
    /** The tunnel is valid. `keep`: "Keep on this host" is checked. What went wrong, if anything. */
    onapply: (tunnel: Tunnel, keep: boolean) => void | Promise<string | null>;
    oncancel: () => void;
    /** In the strip of a session: "Keep on this host" instead of "Start with the session", and the
     *  order of a strip, the main button first (guidelines, 9.1, "Hierarchy"). */
    strip?: boolean;
    /** The host of the session is in the book: the tunnel can be kept on it. */
    keepable?: boolean;
  } = $props();

  // The form starts from the tunnel it edits; it is opened again for another one.
  // svelte-ignore state_referenced_locally
  const start = tunnel;
  const id = start?.id ?? crypto.randomUUID();
  const portText = (n: number | undefined) => (n ? String(n) : '');

  let kind = $state<TunnelKind>(start?.kind ?? 'local');
  // The fields by side: the port of this machine, the port of the server, and the host the tunnel
  // reaches, "Server host" for a local forward, "Local host" for a remote one.
  let localPort = $state(portText(start ? (start.kind === 'remote' ? start.targetPort : start.listenPort) : undefined));
  let serverPort = $state(portText(start ? (start.kind === 'remote' ? start.listenPort : start.kind === 'local' ? start.targetPort : undefined) : undefined));
  let targetHost = $state(start?.targetHost || 'localhost');
  let name = $state(start?.name ?? '');
  let autostart = $state(start?.autostart ?? true);
  let bindAny = $state(start?.bindAny ?? false);
  let keep = $state(false);
  /** The local port follows the server port until the user types one (guidelines, 9.21). */
  let localTyped = !!start;
  /** "Add tunnel" was tried: an empty field is then said to be missing. */
  let tried = $state(false);
  let busy = $state(false);
  /** While it works, the button says so (guidelines, 9.1, "Work in progress"). */
  const busyLabel = $derived(applyLabel === 'Apply' ? 'Applying…' : 'Adding…');
  /** What kept the tunnel from being added, such as the host that could not be saved. */
  let failure = $state('');

  let rootEl = $state<HTMLElement>();
  let kindEl = $state<HTMLSelectElement>();
  onMount(() => void tick().then(() => kindEl?.focus()));

  function kindChanged(next: TunnelKind) {
    kind = next;
    // A SOCKS proxy listens on 1080 at first; the other kinds follow the server port again.
    if (!localTyped) localPort = next === 'socks' ? '1080' : serverPort;
  }

  const PORT_RANGE = 'Enter a port from 1 to 65535.';
  const port = (text: string) => {
    const n = Number(text);
    return text.trim() !== '' && Number.isInteger(n) && n >= 1 && n <= 65535 ? n : null;
  };
  const listenPort = $derived(port(kind === 'remote' ? serverPort : localPort));

  /** What is wrong with each field: a value out of range or a port taken at once, a missing value
   *  once the user tried to apply. */
  const errors = $derived.by(() => {
    const out: { local?: string; server?: string; host?: string } = {};
    const check = (field: 'local' | 'server', text: string) => {
      if (text.trim() === '' ? tried : port(text) === null) out[field] = PORT_RANGE;
    };
    check('local', localPort);
    if (kind !== 'socks') {
      check('server', serverPort);
      if (tried && !targetHost.trim()) out.host = 'Enter a host.';
    }
    const listen = kind === 'remote' ? 'server' : 'local';
    if (!out[listen] && listenPort !== null && others.some((o) => o.id !== id && samePort(o, { kind, listenPort }))) {
      out[listen] = 'Another tunnel already listens on this port.';
    }
    return out;
  });

  /** The messages of the fields in error, each said once. */
  const fieldErrors = $derived([...new Set([errors.local, errors.server, errors.host].filter(Boolean))].join(' '));

  /** The tunnel the form describes, once every field is valid. */
  const built = $derived.by((): Tunnel | null => {
    if (listenPort === null || errors.local || errors.server) return null;
    if (kind === 'socks') return { id, name: name.trim(), kind, listenPort, targetHost: '', targetPort: 0, bindAny, autostart: strip || autostart };
    const target = port(kind === 'remote' ? localPort : serverPort);
    if (target === null || !targetHost.trim()) return null;
    return { id, name: name.trim(), kind, listenPort, targetHost: targetHost.trim(), targetPort: target, bindAny, autostart: strip || autostart };
  });

  /** Applies the tunnel when it is valid; otherwise the fields say what is missing, and the first of
   *  them takes the keyboard. True when it was applied. Also called by the host form, whose save
   *  applies an open tunnel first. */
  export function submit(): boolean {
    tried = true;
    const done = built;
    if (!done) {
      void tick().then(() => rootEl?.querySelector<HTMLInputElement>('input[aria-invalid="true"]')?.focus());
      return false;
    }
    failure = '';
    const result = onapply(done, strip && keepable && keep);
    if (result instanceof Promise) {
      busy = true;
      void result.then((said) => {
        busy = false;
        if (said) failure = said;
      });
    }
    return true;
  }

  function onKey(e: KeyboardEvent) {
    // Enter in a field, a checkbox included, applies the tunnel: the host is saved by its own button.
    if (e.key === 'Enter' && e.target instanceof HTMLInputElement) {
      e.preventDefault();
      e.stopPropagation();
      if (!busy) submit();
    } else if (e.key === 'Escape' && strip) {
      // In the strip, Escape leaves the form, as it leaves the search bar.
      e.preventDefault();
      e.stopPropagation();
      oncancel();
    }
  }

  const fieldId = `tunnel-${id}`;
</script>

{#snippet portField(which: 'local' | 'server', label: string)}
  {@const error = errors[which]}
  <label class="port">
    <span class="label">{label}</span>
    <input
      class="field mono"
      class:invalid={!!error}
      type="number"
      min="1"
      max="65535"
      aria-invalid={error ? 'true' : undefined}
      aria-describedby={error ? `${fieldId}-error` : undefined}
      value={which === 'local' ? localPort : serverPort}
      oninput={(e) => {
        const text = e.currentTarget.value;
        if (which === 'local') {
          localPort = text;
          localTyped = text !== '';
        } else {
          serverPort = text;
          if (!localTyped && kind !== 'socks') localPort = text;
        }
      }}
    />
  </label>
{/snippet}

{#snippet hostField(label: string)}
  <label class="host">
    <span class="label">{label}</span>
    <input
      class="field mono"
      class:invalid={!!errors.host}
      type="text"
      spellcheck="false"
      autocomplete="off"
      aria-invalid={errors.host ? 'true' : undefined}
      aria-describedby={errors.host ? `${fieldId}-error` : undefined}
      bind:value={targetHost}
    />
  </label>
{/snippet}

<!-- The Enter key is only watched: the fields stay the interactive elements. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="tunnel-form" class:strip bind:this={rootEl} role="group" aria-label={start ? 'Edit tunnel' : 'New tunnel'} onkeydown={onKey}>
  <div class="grid">
    <label class="kind">
      <span class="label">Kind</span>
      <Select bind:element={kindEl} value={kind} onchange={(e) => kindChanged(e.currentTarget.value as TunnelKind)}>
        {#each Object.entries(TUNNEL_KINDS) as [value, text] (value)}<option {value}>{text}</option>{/each}
      </Select>
    </label>
    <!-- The fields of the kind, in the order of the tunnel: from where it listens to where it goes. -->
    {#if kind === 'local'}
      {@render portField('local', 'Local port')}
      {@render hostField('Server host')}
      {@render portField('server', 'Server port')}
    {:else if kind === 'remote'}
      {@render portField('server', 'Server port')}
      {@render hostField('Local host')}
      {@render portField('local', 'Local port')}
    {:else}
      {@render portField('local', 'Local port')}
    {/if}
    <!-- What is wrong with these fields, under them. The line keeps its place: "Add tunnel" does not
         move when it shows (guidelines, 9.1). A port field is too narrow for its own message. -->
    <p class="errors" id="{fieldId}-error" role="alert">{fieldErrors || '\u00a0'}</p>
    <label class="name">
      <span class="label">Name</span>
      <input class="field" type="text" placeholder="postgres" spellcheck="false" autocomplete="off" bind:value={name} />
    </label>
    <div class="checks">
      {#if strip}
        {#if keepable}
          <label class="check"><input type="checkbox" bind:checked={keep} /><span>Keep on this host</span></label>
        {/if}
      {:else}
        <label class="check"><input type="checkbox" bind:checked={autostart} /><span>Start with the session</span></label>
      {/if}
    <label class="check"><input type="checkbox" bind:checked={bindAny} /><span>Let other machines connect</span></label>
    </div>
  </div>
  {#if bindAny}
    <p class="warning"><Icon name="warning" /><span>Anyone on your network can reach this port.</span></p>
  {/if}

  <!-- The tunnel in words, since localhost is the server's in a local forward and this machine's in a
       remote one. The line keeps its place while the fields are incomplete. -->
  <p class="sentence">
    {#if built}{#each tunnelSentence(built) as word, i (i)}<span class:mono={word.code}>{word.text}</span>{/each}{:else}{'\u00a0'}{/if}
  </p>

  <!-- Both labels hold the place, the hidden one invisible to screen readers too: the button keeps
       the width of the longer, and "Cancel" does not move after it (guidelines, 9.1). -->
  {#snippet label()}
    <span class="labels"><span class:off={busy}>{applyLabel}</span><span class:off={!busy}>{busyLabel}</span></span>
  {/snippet}
  <div class="buttons">
    {#if strip}
      <button class="btn small" type="button" disabled={busy} onclick={() => submit()}>{@render label()}</button>
      <button class="btn ghost small" type="button" onclick={oncancel}>Cancel</button>
      <!-- What kept the tunnel from being added, after the buttons, which do not move (guidelines, 9.1). -->
      {#if failure}<p class="failure" role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>{/if}
    {:else}
      <button class="btn ghost small" type="button" onclick={oncancel}>Cancel</button>
      <button class="btn small" type="button" disabled={busy} onclick={() => submit()}>{@render label()}</button>
    {/if}
  </div>
</div>

<style>
  .tunnel-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  /* In the strip, the terminal is wide and low: the fields of the kind share a line with it, and
     the checkboxes with the name. */
  .tunnel-form.strip {
    max-width: 760px;
  }
  /* Six columns, as in the host form: the list takes half, a port one, a host four. */
  .grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 12px;
  }
  .grid label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .kind,
  .name {
    grid-column: 1 / span 3;
  }
  .port {
    grid-column: span 1;
  }
  .kind + .port {
    grid-column-start: 1;
  }
  .host {
    grid-column: span 4;
  }
  .strip .grid {
    grid-template-columns: repeat(10, minmax(0, 1fr));
  }
  .strip .port {
    grid-column: span 2;
  }
  /* The whole shorthand: setting only its start would bring the span back to 1. */
  .strip .kind + .port {
    grid-column: span 2;
  }
  .strip .host {
    grid-column: span 3;
  }
  .strip .name {
    grid-column: 1 / span 4;
  }
  /* Under the row of the fields, across it; its height is kept when it is empty. */
  .errors {
    grid-column: 1 / -1;
    margin: -8px 0 0;
    color: var(--bad);
    font-size: var(--fs-small);
  }
  /* A checkbox and its text, 6 px apart; the text is part of the target (guidelines, 9.3). */
  .checks {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    margin-top: -4px;
  }
  .strip .checks {
    grid-column: span 6;
    flex-direction: row;
    align-items: center;
    align-self: end;
    gap: 16px;
    margin-top: 0;
    min-height: 32px;
  }
  /* More specific than `.grid label`, which stacks a label above its field. */
  .grid .check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
  /* A warning box (guidelines, 9.10): the icon in --warn, the text in --ink. */
  .warning {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .warning span {
    color: var(--ink);
  }
  .sentence {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .failure {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    padding-top: 4px;
    color: var(--bad);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .failure :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .labels {
    display: grid;
  }
  .labels > span {
    grid-area: 1 / 1;
  }
  .labels .off {
    visibility: hidden;
  }
  .buttons {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    justify-content: flex-end;
  }
  /* A strip reads the other way: the main button first, at the left (guidelines, 9.1). */
  .strip .buttons {
    justify-content: flex-start;
  }
</style>
