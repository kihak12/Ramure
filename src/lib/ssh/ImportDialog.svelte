<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { veil } from '../ui/veil';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh, type ImportSource } from './ssh.svelte';
  import type { Host } from './types';
  // The preview of an import of hosts: what will be added, renamed, is already there or cannot come
  // over, chosen before anything is written (guidelines, 9.15, "Importing and exporting hosts").

  type Status = 'added' | 'renamed' | 'same' | 'refused';
  type Row = { index: number; status: Status; alias: string; detail: string; checked: boolean; converts: boolean };
  type Preview = { rows: Row[]; upToDate: number; passwords: number; keys: number };
  type Imported = { added: number; passwordsRefused: number; keysConverted: number; keysFailed: string[] };

  const TITLE: Record<ImportSource, string> = { ramure: 'Import from a Ramure file', filezilla: 'Import from FileZilla' };

  let element = $state<HTMLDialogElement>();
  let preview = $state<Preview | null>(null);
  let checked = $state<Record<number, boolean>>({});
  let remember = $state(false);
  let loaded = $state(false);
  let busy = $state(false);
  /** What failed: reading the file, or adding the hosts. Said in place of the list, or in the last line (9.11). */
  let failure = $state('');
  let readFailed = $state(false);
  /** The import was tried with nothing checked: the reason turns into a warning (guidelines, 9.1). */
  let tried = $state(false);

  const rows = $derived(preview?.rows ?? []);
  const choosable = $derived(rows.filter((row) => row.status !== 'refused'));
  const count = $derived(choosable.filter((row) => checked[row.index]).length);
  /** The keys converted into ~/.ssh: those of the rows checked (guidelines, 9.15). */
  const converting = $derived(choosable.filter((row) => checked[row.index] && row.converts).length);
  const all = $derived(choosable.length > 0 && count === choosable.length);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const WHY = 'Check at least one host.';
  const STATE = (row: Row) =>
    row.status === 'added' ? 'Added' : row.status === 'renamed' ? `Added as ${row.alias}` : row.status === 'same' ? 'Same server' : 'Not imported';

  $effect(() => {
    if (!element) return;
    if (ssh.importing !== null && !element.open) {
      void load();
      element.showModal();
    } else if (ssh.importing === null && element.open) {
      element.close();
    }
  });

  async function load() {
    preview = null;
    checked = {};
    remember = false;
    loaded = false;
    failure = '';
    readFailed = false;
    tried = false;
    const what = ssh.importing;
    if (!what) return;
    try {
      preview = await invoke<Preview>('hosts_import_preview', { source: what.source, path: what.path });
      checked = Object.fromEntries(preview.rows.map((row) => [row.index, row.checked]));
    } catch (e) {
      failure = typeof e === 'string' ? e : String(e);
      readFailed = true;
    }
    loaded = true;
    // The focus goes to the first checkbox, otherwise to the button of the last line (guidelines, 9.10).
    await tick();
    focusWithRing(element?.querySelector<HTMLElement>('.list input[type="checkbox"], footer .btn:last-child'));
  }

  function checkAll(value: boolean) {
    for (const row of choosable) checked[row.index] = value;
  }

  const close = () => (ssh.importing = null);

  $effect(() => {
    if (count) tried = false;
  });

  /** Enter on a checkbox imports, as in a field; with nothing checked, says why. */
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !(e.target instanceof HTMLInputElement)) return;
    e.preventDefault();
    if (!count) {
      tried = true;
      return;
    }
    (e.currentTarget as HTMLFormElement).requestSubmit();
  }

  async function importChosen(e: SubmitEvent) {
    e.preventDefault();
    if (!count || busy) return;
    busy = true;
    failure = '';
    const chosen = choosable.filter((row) => checked[row.index]).map((row) => row.index);
    try {
      const done = await invoke<Imported>('hosts_import_apply', { chosen, rememberPasswords: remember });
      ssh.hosts = await invoke<Host[]>('hosts_list');
      close();
      toasts.ok(`${plural(done.added, 'host')} imported`, done.keysConverted ? `${plural(done.keysConverted, 'key')} converted into ~/.ssh.` : '');
      // A password the keyring refused: the secret is not kept, said as for any secret (9.15, "Secrets").
      if (done.passwordsRefused) {
        const them = done.passwordsRefused === 1 ? 'it' : 'them';
        toasts.warn('The secret could not be kept', `The system keyring refused ${plural(done.passwordsRefused, 'password')}: Ramure will ask for ${them} when connecting.`);
      }
      // A key that could not be converted: its host came without it, and the reason is said.
      if (done.keysFailed.length) toasts.warn(`${plural(done.keysFailed.length, 'key')} not converted`, done.keysFailed.join(' '));
    } catch (err) {
      failure = typeof err === 'string' ? err : String(err);
    } finally {
      busy = false;
    }
  }
</script>

<dialog
  bind:this={element}
  aria-labelledby="import-title"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) close();
  }}
  use:veil={() => {
    // A write does not stop halfway: nothing closes the dialog before it ends.
    if (!busy) close();
  }}
>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="box" onsubmit={importChosen} onkeydown={onKey}>
    <header>
      <h2 id="import-title">{TITLE[ssh.importing?.source ?? 'ramure']}</h2>
      <p>Ramure adds the hosts you check to the book. A host of the book is never changed or deleted.</p>
    </header>

    <div class="list">
      {#if loaded && rows.length}
        {#if choosable.length > 1}
          <label class="all">
            <input type="checkbox" checked={all} indeterminate={count > 0 && !all} onchange={(e) => checkAll(e.currentTarget.checked)} />
            <span>Check all</span>
          </label>
        {/if}
        <ul>
          {#each rows as row (row.index)}
            <li class:refused={row.status === 'refused'}>
              {#if row.status === 'refused'}
                <div class="row">
                  <span class="mark"><Icon name="warning" size={14} /></span>
                  <span class="text">
                    <span class="line"><b>{row.alias}</b> <span class="state">{STATE(row)}</span></span>
                    <small>{row.detail}</small>
                  </span>
                </div>
              {:else}
                <label class="row">
                  <input type="checkbox" bind:checked={checked[row.index]} />
                  <span class="text">
                    <span class="line"><b>{row.alias}</b> <span class="state">{STATE(row)}</span></span>
                    <small class:mono={row.status !== 'same'}>{row.detail}</small>
                  </span>
                </label>
              {/if}
            </li>
          {/each}
        </ul>
        {#if preview?.upToDate}
          <p class="count">{plural(preview.upToDate, 'host')} {preview.upToDate === 1 ? 'is' : 'are'} already in the book.</p>
        {/if}
        {#if converting}
          <p class="count">{plural(converting, 'key')} {converting === 1 ? 'is' : 'are'} converted into ~/.ssh, as {converting === 1 ? 'its host is' : 'their hosts are'} added.</p>
        {/if}
        {#if preview?.passwords}
          <label class="remember">
            <input type="checkbox" bind:checked={remember} />
            <span>Remember the passwords found in the file in the system keyring</span>
          </label>
        {/if}
      {:else if loaded && readFailed}
        <p class="none bad" role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>
      {:else if loaded}
        <p class="none">{preview?.upToDate ? 'All these hosts are already in the book.' : 'No hosts found in this file.'}</p>
      {/if}
    </div>

    <!-- The way out, then the import, on the right; on the left, why it is disabled, or what failed. -->
    <footer>
      <div class="result">
        {#if failure && !readFailed}
          <p class="bad" role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>
        {:else if choosable.length && !count && tried}
          <p class="tried" role="alert"><Icon name="warning" size={14} /><span>{WHY}</span></p>
        {:else if choosable.length && !count}
          <p class="why">{WHY}</p>
        {:else if loaded && rows.length && !choosable.length}
          <p class="why">None of these hosts can be imported.</p>
        {/if}
      </div>
      <!-- The last line shows once the file is read: its buttons do not change under the pointer. -->
      {#if loaded && choosable.length}
        <button class="btn" type="button" disabled={busy} onclick={close}>Cancel</button>
        <button class="btn primary" type="submit" disabled={!count || busy} title={count ? undefined : WHY}>
          {busy ? 'Importing…' : count ? `Import ${plural(count, 'host')}` : 'Import'}
        </button>
      {:else if loaded}
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
  .remember {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
  }
  .all {
    border-bottom: 1px solid var(--line);
  }
  .remember {
    border-top: 1px solid var(--line);
    padding-top: 8px;
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 0;
  }
  /* The checkbox and the icon of a host that is not imported take the same column, aligned with the name. */
  .row input,
  .mark {
    margin-top: 2px;
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 15px;
    color: var(--warn);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .state {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  li.refused .state {
    color: var(--warn);
  }
  small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--f-mono);
  }
  .count,
  .none {
    margin: 0;
    padding: 8px 0;
    color: var(--muted);
  }
  .none {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 12px 0 4px;
  }
  .none.bad {
    color: var(--bad);
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
  .result :global(svg),
  .none :global(svg) {
    flex: none;
  }
</style>
