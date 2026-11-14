<script lang="ts">
  // The strip of the tunnels of a session, under its header (guidelines, 9.15, "Tunnels"): one row
  // per tunnel, its state in words, and "Add tunnel…", whose form opens in place.
  import { tick } from 'svelte';
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh, type Session, type TunnelRuntime } from './ssh.svelte';
  import TunnelForm from './TunnelForm.svelte';
  import { count, tunnelAddress, tunnelLabel, tunnelWords, type Tunnel } from './types';

  let { session }: { session: Session } = $props();

  const rows = $derived(ssh.tunnelsOf(session));
  /** The host as the book has it: a tunnel can be kept on it, and "Edit host…" opens its form. */
  const host = $derived(ssh.hostOf(session));

  /** The state in words, with its icon: never a color alone (guidelines, 9.15). */
  const STATES: Record<TunnelRuntime['state'], { words: string; icon: IconName | null }> = {
    starting: { words: 'Starting…', icon: null },
    listening: { words: 'Listening', icon: 'check' },
    waiting: { words: 'Waiting for the connection', icon: 'refresh' },
    stopped: { words: 'Stopped', icon: 'stop' },
    failed: { words: 'Failed', icon: 'warning' },
  };

  // The time of a last failure, in the American format (guidelines, 12): "11:07 PM".
  const time = new Intl.DateTimeFormat('en-US', { hour: 'numeric', minute: '2-digit' });

  // Past five rows, the list scrolls: it stops where the sixth row starts, the lines under the
  // first five included.
  const SHOWN = 5;
  let rowsEl = $state<HTMLElement>();
  let limit = $state<number | null>(null);
  $effect(() => {
    void rows;
    void tick().then(() => {
      const sixth = rowsEl?.querySelectorAll<HTMLElement>('.row')[SHOWN];
      limit = sixth ? sixth.offsetTop : null;
    });
  });

  let adding = $state(false);
  let addEl = $state<HTMLButtonElement>();
  let rootEl = $state<HTMLElement>();

  async function backToAdd() {
    adding = false;
    await tick();
    focusWithRing(addEl);
  }

  async function add(tunnel: Tunnel, keep: boolean): Promise<string | null> {
    const failed = await ssh.tunnelAdd(session, tunnel, keep);
    if (!failed) await backToAdd();
    return failed;
  }

  async function copy(tunnel: Tunnel) {
    const address = tunnelAddress(tunnel);
    try {
      await navigator.clipboard.writeText(address);
      toasts.ok('Address copied', address);
    } catch {
      toasts.err('The address could not be copied', 'The clipboard refused it.');
    }
  }

  /** "Remove": the row is gone, the focus goes to the next one, or to "Add tunnel…". */
  async function remove(id: string) {
    const at = rows.findIndex((row) => row.tunnel.id === id);
    await ssh.tunnelRemove(session, id);
    await tick();
    const items = rootEl?.querySelectorAll<HTMLElement>('.row');
    const next = items?.[Math.min(at, items.length - 1)]?.querySelector<HTMLElement>('.btn');
    focusWithRing(next ?? addEl);
  }

  /** "Edit host…": the form of the host, where its tunnels are defined. */
  function editHost() {
    if (host) ssh.editing = host;
  }
</script>

<div class="strip" role="region" aria-label="Tunnels" bind:this={rootEl}>
  {#if rows.length}
    <!-- Past five rows, the list scrolls; "Add tunnel…" stays under it. -->
    <div class="rows" bind:this={rowsEl} style:max-height={limit === null ? null : `${limit}px`}>
      {#each rows as row (row.tunnel.id)}
        {@const { tunnel, state } = row}
        {@const label = tunnelLabel(tunnel)}
        {@const said = STATES[state.state]}
        {@const verb = state.state === 'failed' ? 'Retry' : state.state === 'stopped' ? 'Start' : 'Stop'}
        <div class="row" title={tunnelWords(tunnel)}>
          <span class="state s-{state.state}">
            {#if said.icon}<Icon name={said.icon} size={14} />{:else}<span class="spinner" aria-hidden="true"></span>{/if}
            <span>{said.words}</span>
          </span>
          <span class="name" class:mono={!tunnel.name}>{label}</span>
          <span class="count">{state.connections ? count(state.connections, 'connection') : ''}</span>
          <!-- On hover and keyboard focus (guidelines, 9.6). Each row carries the same buttons: their
               accessible name says which tunnel. -->
          <span class="acts">
            <button class="btn ghost small" type="button" title={tunnelAddress(tunnel)} aria-label="Copy address of {label}" onclick={() => void copy(tunnel)}>
              Copy address
            </button>
            <!-- One button whose word follows the state: the focus stays on it after a click. -->
            <button
              class="btn ghost small"
              type="button"
              aria-label="{verb} {label}"
              onclick={() => void (verb === 'Stop' ? ssh.tunnelStop(session, tunnel.id) : ssh.tunnelStart(session, tunnel))}
            >
              {verb}
            </button>
            {#if row.own}
              <button class="btn ghost small" type="button" aria-label="Remove {label}" onclick={() => void remove(tunnel.id)}>Remove</button>
            {:else if host}
              <button class="btn ghost small" type="button" onclick={editHost}>Edit host…</button>
            {/if}
          </span>
        </div>
        {#if state.state === 'failed' && state.error}
          <!-- Why it could not start, and what to do: announced at once. -->
          <p class="line bad" role="alert">{state.error}</p>
        {:else if state.lastFailure}
          <!-- The last connection the other side refused: the next one may work. Not announced: a
               program that retries would make it speak every second. -->
          <p class="line warn">{state.lastFailure.message} <time class="when" datetime={new Date(state.lastFailure.at).toISOString()}>{time.format(state.lastFailure.at)}</time></p>
        {/if}
      {/each}
    </div>
  {/if}

  {#if adding}
    <div class="form" class:below={rows.length > 0}>
      <TunnelForm others={rows.map((row) => row.tunnel)} applyLabel="Add tunnel" onapply={add} oncancel={() => void backToAdd()} strip keepable={!!host} />
    </div>
  {:else}
    <div class="foot">
      {#if !rows.length}<span class="none">No tunnels. Reach a port of the server from this machine, or the reverse.</span>{/if}
      <button class="btn small" type="button" bind:this={addEl} onclick={() => (adding = true)}><Icon name="plus" size={14} /> Add tunnel…</button>
    </div>
  {/if}
</div>

<style>
  /* Under the header, on --panel, with a --line-soft separator: it pushes the terminal down and never
     covers its text, as the search bar does. */
  .strip {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px 12px 8px;
    border-bottom: 1px solid var(--line-soft);
    background: var(--panel);
  }
  /* One 28 px row per tunnel; the columns line up from row to row. Past five rows, it scrolls (its
     height is set by the script, at the top of the sixth row). */
  .rows {
    position: relative;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    column-gap: 12px;
    margin: 0 -6px;
    overflow-y: auto;
  }
  .row {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: subgrid;
    align-items: center;
    min-height: 28px;
    padding: 0 6px;
    border-radius: var(--radius);
    transition: background var(--t-fast) var(--ease);
  }
  /* The background follows the pointer and the keyboard focus, never a click (guidelines, 9.6). */
  .row:hover,
  .row:has(:focus-visible),
  .row:has(:global(.focus-ring:focus)) {
    background: var(--hover);
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  /* The icon says the state with its color; the words say it in any case. */
  .s-listening :global(svg) {
    color: var(--ok);
  }
  .s-waiting :global(svg) {
    color: var(--warn);
  }
  .s-failed :global(svg) {
    color: var(--bad);
  }
  .s-stopped :global(svg) {
    color: var(--muted);
  }
  /* The 12 px ring in the 14 px box of an icon: the words do not move. */
  .state .spinner {
    margin: 1px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.mono {
    font-size: var(--fs-code);
  }
  .count {
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .acts {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  .row:hover .acts,
  .row:has(:focus-visible) .acts,
  .row:has(:global(.focus-ring:focus)) .acts {
    opacity: 1;
  }
  /* Under its row, aligned with the name: why it failed, or the last connection refused. */
  .line {
    grid-column: 2 / -1;
    margin: 0 6px 4px 0;
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .line.bad {
    color: var(--bad);
  }
  .line.warn {
    color: var(--warn);
  }
  .when {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .none {
    color: var(--muted);
  }
  .form.below {
    margin-top: 4px;
    padding-top: 10px;
    border-top: 1px solid var(--line-soft);
  }
</style>
