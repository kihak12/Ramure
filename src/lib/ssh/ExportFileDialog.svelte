<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { veil } from '../ui/veil';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh } from './ssh.svelte';
  import { addressOf, type Host } from './types';
  // Writing hosts of the book into a file of Ramure, to hand to someone: never a secret
  // (guidelines, 9.15, "Importing and exporting hosts").

  let element = $state<HTMLDialogElement>();
  let checked = $state<Record<string, boolean>>({});
  let filter = $state('');
  let busy = $state(false);
  /** What failed: the picker or the writing. Said in the last line (9.11). */
  let failure = $state('');
  /** The export was tried with nothing checked: the reason turns into a warning (guidelines, 9.1). */
  let tried = $state(false);

  const hosts = $derived(ssh.hosts);
  const needle = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    needle
      ? hosts.filter((h) => [h.alias, h.hostname, h.user, h.group, ...h.tags].some((v) => v.toLowerCase().includes(needle)))
      : hosts,
  );
  /** The hosts shown, under the headers of their groups, in the order of the book. */
  const groups = $derived.by(() => {
    const out: { name: string; hosts: Host[] }[] = [];
    for (const host of shown) {
      const last = out.at(-1);
      if (last && last.name === host.group) last.hosts.push(host);
      else out.push({ name: host.group, hosts: [host] });
    }
    return out;
  });
  const count = $derived(hosts.filter((h) => checked[h.id]).length);
  const all = $derived(hosts.length > 0 && count === hosts.length);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const WHY = 'Check at least one host.';

  $effect(() => {
    if (!element) return;
    if (ssh.exportingFile !== null && !element.open) {
      void start();
      element.showModal();
    } else if (ssh.exportingFile === null && element.open) {
      element.close();
    }
  });

  async function start() {
    const one = ssh.exportingFile === 'all' ? null : ssh.exportingFile;
    // All the hosts at first; opened from the menu of a host, that host only.
    checked = Object.fromEntries(hosts.map((h) => [h.id, one === null || h.id === one]));
    filter = '';
    failure = '';
    tried = false;
    await tick();
    focusWithRing(element?.querySelector<HTMLElement>('input[type="search"], .list input[type="checkbox"], footer .btn:last-child'));
  }

  const close = () => (ssh.exportingFile = null);

  function checkAll(value: boolean) {
    for (const host of hosts) checked[host.id] = value;
  }

  function checkGroup(group: Host[], value: boolean) {
    for (const host of group) checked[host.id] = value;
  }

  $effect(() => {
    if (count) tried = false;
  });

  /** Enter on a checkbox exports, as in a field; with nothing checked, says why. */
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !(e.target instanceof HTMLInputElement) || e.target.type === 'search') return;
    e.preventDefault();
    if (!count) {
      tried = true;
      return;
    }
    (e.currentTarget as HTMLFormElement).requestSubmit();
  }

  /** The name the picker proposes: after what is exported, and the day (guidelines, 9.15). */
  function proposedName(chosen: Host[]): string {
    const day = new Date();
    const date = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, '0')}-${String(day.getDate()).padStart(2, '0')}`;
    const base = chosen.length === 1 ? chosen[0].alias.replace(/[^\w.-]+/g, '-') : 'ramure-hosts';
    return `${base}_${date}.json`;
  }

  async function exportChosen(e: SubmitEvent) {
    e.preventDefault();
    if (!count || busy) return;
    failure = '';
    const chosen = hosts.filter((h) => checked[h.id]);
    let path: string | null;
    try {
      path = await save({ title: 'Save the hosts', defaultPath: proposedName(chosen), filters: [{ name: 'Hosts of Ramure', extensions: ['json'] }] });
    } catch (err) {
      failure = `The file picker could not open: ${typeof err === 'string' ? err : String(err)}`;
      return;
    }
    if (!path) return;
    busy = true;
    try {
      const written = await invoke<number>('hosts_file_export', { ids: chosen.map((h) => h.id), path });
      close();
      toasts.ok(written === 1 ? `${chosen[0].alias} exported` : `${plural(written, 'host')} exported`, path);
    } catch (err) {
      failure = typeof err === 'string' ? err : String(err);
    } finally {
      busy = false;
    }
  }
</script>

<dialog
  bind:this={element}
  aria-labelledby="export-file-title"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) close();
  }}
  use:veil={() => {
    if (!busy) close();
  }}
>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="box" onsubmit={exportChosen} onkeydown={onKey}>
    <header>
      <h2 id="export-file-title">Export to a Ramure file</h2>
      <p>Ramure writes the hosts you check into a file you can hand to someone. Passwords and passphrases are never written.</p>
      <p>The file holds names, addresses, users, key paths, groups, tags, colors, startup commands and tunnels.</p>
      {#if hosts.length > 8}
        <input class="field" type="search" placeholder="Filter by name, address, group, tag" aria-label="Filter the hosts" bind:value={filter} />
      {/if}
    </header>

    <div class="list">
      {#if hosts.length}
        <label class="all">
          <input type="checkbox" checked={all} indeterminate={count > 0 && !all} onchange={(e) => checkAll(e.currentTarget.checked)} />
          <span>Check all</span>
        </label>
        {#each groups as group (group.name)}
          {@const inGroup = group.hosts.filter((h) => checked[h.id]).length}
          {#if group.name}
            <label class="group">
              <input
                type="checkbox"
                checked={inGroup === group.hosts.length}
                indeterminate={inGroup > 0 && inGroup < group.hosts.length}
                onchange={(e) => checkGroup(group.hosts, e.currentTarget.checked)}
              />
              <span>{group.name}</span>
            </label>
          {/if}
          <ul>
            {#each group.hosts as host (host.id)}
              <li>
                <label class="row">
                  <input type="checkbox" bind:checked={checked[host.id]} />
                  <span class="text">
                    <b>{host.alias}</b>
                    <small class="mono">{addressOf(host.user || ssh.defaultUser, host.hostname, host.port)}</small>
                  </span>
                </label>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="none">No host matches “{filter}”.</p>
        {/each}
      {:else}
        <p class="none">No hosts: nothing to export.</p>
      {/if}
    </div>

    <footer>
      <div class="result">
        {#if failure}
          <p class="bad" role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>
        {:else if hosts.length && !count && tried}
          <p class="tried" role="alert"><Icon name="warning" size={14} /><span>{WHY}</span></p>
        {:else if hosts.length && !count}
          <p class="why">{WHY}</p>
        {/if}
      </div>
      {#if hosts.length}
        <button class="btn" type="button" disabled={busy} onclick={close}>Cancel</button>
        <button class="btn primary" type="submit" disabled={!count || busy} title={count ? undefined : WHY}>
          {busy ? 'Exporting…' : count ? `Export ${plural(count, 'host')}…` : 'Export…'}
        </button>
      {:else}
        <button class="btn" type="button" onclick={close}>Close</button>
      {/if}
    </footer>
  </form>
</dialog>

<style>
  dialog {
    width: min(640px, calc(100vw - 48px));
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--ink);
    box-shadow: var(--shadow-pop);
  }
  dialog[open] {
    display: flex;
    animation: pop-in var(--t-med) var(--ease);
  }
  dialog::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(2px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .box {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 0;
    margin: 0;
  }
  header {
    padding: 16px 20px 8px;
    display: grid;
    gap: 6px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  header p {
    margin: 0;
    color: var(--muted);
  }
  header .field {
    margin-top: 6px;
  }
  /* Plain rows, separated by a --line rule (guidelines, 9.6, a list inside a dialog). */
  .list {
    flex: 1 1 auto;
    overflow-y: auto;
    min-height: 0;
    padding: 4px 20px 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li + li {
    border-top: 1px solid var(--line);
  }
  .all,
  .group {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  /* The header of a group, as in the book: the name in --muted, 11.5 px. */
  .group {
    color: var(--muted);
    font-size: var(--fs-small);
    padding-top: 12px;
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 0;
  }
  .row input {
    margin-top: 2px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--f-mono);
  }
  .none {
    margin: 0;
    padding: 12px 0 4px;
    color: var(--muted);
  }
  footer {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 16px 20px;
  }
  footer .btn {
    flex: none;
  }
  .result {
    flex: 1;
    min-width: 0;
    padding-top: 8px;
    font-size: var(--fs-small);
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .result p {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
  }
  .result .why {
    color: var(--muted);
  }
  .result .tried {
    color: var(--warn);
  }
  .result .bad {
    color: var(--bad);
  }
  .result :global(svg) {
    flex: none;
  }
</style>
