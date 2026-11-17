<script lang="ts">
  // An empty pane chooses its location: this machine, or a host of the book (guidelines, 9.24).
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { modules } from '../ui/modules.svelte';
  import { ssh } from '../ssh/ssh.svelte';
  import { addressOf, type Host } from '../ssh/types';
  import { files, type Pane } from './files.svelte';

  let { pane }: { pane: Pane } = $props();

  let filter = $state('');
  let home = $state('');

  const needle = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    ssh.hosts.filter(
      (h) => !needle || [h.alias, h.hostname, h.user, h.group, ...h.tags].some((text) => text.toLowerCase().includes(needle)),
    ),
  );
  // The hosts come sorted by group, as in the book: they are cut into sections.
  const sections = $derived.by(() => {
    const out: { group: string; hosts: Host[] }[] = [];
    for (const host of shown) {
      const last = out.at(-1);
      if (last && last.group === host.group) last.hosts.push(host);
      else out.push({ group: host.group, hosts: [host] });
    }
    return out;
  });
  const local = $derived(!needle || 'this machine'.includes(needle));

  onMount(() => {
    invoke<string>('files_home', { place: { kind: 'local' } }).then((path) => (home = path)).catch(() => {});
  });

  /** ↑ and ↓ go from location to location, Home and End to the ends (guidelines, 9.6); from the
   *  filter, ↓ goes to the first. */
  function onKey(e: KeyboardEvent) {
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
    const rows = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('button.row:not(:disabled)')];
    if (!rows.length) return;
    const fromFilter = e.target instanceof HTMLInputElement;
    if (fromFilter && e.key !== 'ArrowDown') return;
    e.preventDefault();
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      fromFilter ? 0 : e.key === 'Home' ? 0 : e.key === 'End' ? rows.length - 1 : e.key === 'ArrowDown' ? Math.min(at + 1, rows.length - 1) : Math.max(at - 1, 0);
    rows[next].focus();
  }

  const address = (host: Host) => addressOf(host.user || ssh.defaultUser, host.hostname, host.port);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="picker" onkeydown={onKey}>
  <h3>Choose a location</h3>
  {#if ssh.hosts.length}
    <label class="search">
      <Icon name="search" size={14} />
      <input type="search" placeholder="Filter by name, address, group, tag" aria-label="Filter locations" bind:value={filter} />
    </label>
  {/if}
  <div class="scroll">
    <ul aria-label="Locations">
      {#if local}
        <li>
          <button type="button" class="row" onclick={() => files.choose(pane, { kind: 'local' })}>
            <span class="icon-box" aria-hidden="true"><Icon name="laptop" size={16} /></span>
            <span class="text">
              <b>This machine</b>
              {#if home}<small>{home}</small>{/if}
            </span>
          </button>
        </li>
      {/if}
    </ul>
    {#each sections as section (section.group)}
      {#if section.group}<div class="group">{section.group}</div>{/if}
      <ul aria-label={section.group || 'Hosts'}>
        {#each section.hosts as host (host.id)}
          <li>
            <button
              type="button"
              class="row"
              disabled={host.proxyJump}
              title={host.proxyJump ? 'This host goes through a bastion: not supported yet' : `${host.alias} · ${address(host)}`}
              onclick={() => files.choose(pane, { kind: 'host', hostId: host.id })}
            >
              <!-- The chosen color names the host, as in the book (guidelines, 9.21). -->
              <span class="tint" aria-hidden="true" style:--c={host.color ? `var(--l${host.color})` : 'transparent'}></span>
              <span class="text">
                <b>{host.alias}</b>
                <small class="mono">{address(host)}</small>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/each}
    {#if !ssh.hosts.length && ssh.loaded}
      <div class="none">
        <p>Add hosts in the SSH module to browse their files.</p>
        <button class="btn small" type="button" onclick={() => modules.show('ssh')}><Icon name="terminal" size={14} /> Show SSH terminal</button>
      </div>
    {:else if needle && !local && !shown.length}
      <p class="none">No location matches “{filter}”.</p>
    {/if}
  </div>
</div>

<style>
  .picker {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  h3 {
    margin: 0;
    padding: 12px 12px 8px;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 12px 8px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .search:hover:not(:focus-within) {
    border-color: color-mix(in srgb, var(--muted) 50%, var(--line));
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
    padding: 5px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: 0 6px 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .group {
    padding: 10px 6px 4px;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: stretch;
    gap: 8px;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--ink);
    text-align: left;
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover:not(:disabled) {
    background: var(--hover);
  }
  .row:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  /* The ring of the focus follows the corners of the row. */
  .row:focus-visible {
    outline-offset: -2px;
  }
  .tint {
    flex: none;
    width: 4px;
    border-radius: var(--radius-pill);
    background: var(--c);
  }
  .icon-box {
    flex: none;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.3;
  }
  .text b,
  .text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .none {
    margin: 0;
    padding: 10px;
    color: var(--muted);
    display: grid;
    gap: 8px;
    justify-items: start;
  }
  .none p {
    margin: 0;
  }
</style>
