<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { veil } from '../ui/veil';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh } from './ssh.svelte';

  // Writing hosts of the book into ~/.ssh/config, after showing what changes (guidelines, 9.15,
  // "Exporting to ~/.ssh/config").

  type Status = 'added' | 'modified' | 'current' | 'blocked';
  type Row = { id: string; alias: string; status: Status; detail: string };
  type Exported = { count: number; backup: string | null; written: string };

  const STATE: Record<Status, string> = { added: 'Added', modified: 'Changed', current: 'Up to date', blocked: 'Not exported' };

  let element = $state<HTMLDialogElement>();
  let rows = $state<Row[]>([]);
  let checked = $state<Record<string, boolean>>({});
  let loaded = $state(false);
  let busy = $state(false);
  /** What failed: reading the file, or writing it. Said in the last line (9.11). */
  let failure = $state('');
  /** The export was tried with nothing checked: the reason turns into a warning (guidelines, 9.1). */
  let tried = $state(false);
  /** The export comes from the keyboard: the focus will go to the "Undo" button of the notification (9.11). */
  let byKeyboard = false;

  const single = $derived(ssh.exporting !== null && ssh.exporting !== 'all');
  const listed = $derived(rows.filter((row) => row.status !== 'current'));
  const upToDate = $derived(rows.filter((row) => row.status === 'current').length);
  const writable = $derived(rows.filter((row) => row.status === 'added' || row.status === 'modified'));
  const count = $derived(writable.filter((row) => checked[row.id]).length);
  const someModified = $derived(!single && writable.some((row) => row.status === 'modified'));
  const all = $derived(writable.length > 0 && count === writable.length);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
  const WHY = 'Check at least one host.';

  $effect(() => {
    if (!element) return;
    if (ssh.exporting !== null && !element.open) {
      void load();
      element.showModal();
    } else if (ssh.exporting === null && element.open) {
      element.close();
    }
  });

  async function load() {
    rows = [];
    checked = {};
    loaded = false;
    failure = '';
    tried = false;
    const ids = ssh.exporting === 'all' ? null : [ssh.exporting];
    try {
      rows = await invoke<Row[]>('ssh_config_preview', { ids });
      // The hosts to add are checked; a host to change only when that very host was asked for.
      checked = Object.fromEntries(rows.map((row) => [row.id, row.status === 'added' || (single && row.status === 'modified')]));
    } catch (e) {
      failure = typeof e === 'string' ? e : String(e);
    }
    loaded = true;
    // The focus goes to the first checkbox, otherwise to the button of the last line (guidelines, 9.10).
    await tick();
    focusWithRing(element?.querySelector<HTMLElement>('input[type="checkbox"], footer .btn:last-child'));
  }

  function checkAll(value: boolean) {
    for (const row of writable) checked[row.id] = value;
  }

  const close = () => (ssh.exporting = null);

  $effect(() => {
    if (count) tried = false;
  });

  /** Enter on a checkbox starts the export, as in a field; with nothing checked, says why. */
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !(e.target instanceof HTMLInputElement)) return;
    e.preventDefault();
    if (!count) {
      tried = true;
      return;
    }
    byKeyboard = true;
    (e.currentTarget as HTMLFormElement).requestSubmit();
  }

  async function exportChosen(e: SubmitEvent) {
    e.preventDefault();
    if (!count || busy) return;
    busy = true;
    failure = '';
    const ids = writable.filter((row) => checked[row.id]).map((row) => row.id);
    const alias = writable.find((row) => checked[row.id])?.alias ?? '';
    const keyboard = byKeyboard;
    byKeyboard = false;
    try {
      const done = await invoke<Exported>('ssh_config_export', { ids });
      close();
      // The dialog is closed: the notification is no longer under its veil, and offers to undo.
      toasts.ok(
        done.count === 1 ? `${alias} exported to ~/.ssh/config` : `${plural(done.count, 'host')} exported to ~/.ssh/config`,
        done.backup ? 'Old file: ~/.ssh/config.ramure.bak' : '~/.ssh/config was created.',
        { label: 'Undo', run: () => void undo(done), focus: keyboard },
      );
    } catch (err) {
      failure = typeof err === 'string' ? err : String(err);
    } finally {
      busy = false;
    }
  }

  async function undo(done: Exported) {
    try {
      await invoke('ssh_config_undo', { written: done.written, hadBackup: done.backup !== null });
      toasts.ok(done.backup ? '~/.ssh/config is back as it was before the export' : '~/.ssh/config was removed');
    } catch (err) {
      toasts.err('The export could not be undone', typeof err === 'string' ? err : String(err));
    }
  }
</script>

<dialog
  bind:this={element}
  aria-labelledby="export-title"
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
  <form class="box" onsubmit={exportChosen} onkeydown={onKey}>
    <header>
      <h2 id="export-title">Export to ~/.ssh/config</h2>
      <p>Ramure adds or updates these hosts. The rest of the file is left untouched, and the old file is copied to <code>~/.ssh/config.ramure.bak</code>.</p>
      {#if someModified}
        <p>A changed entry keeps the version of the file until you check it.</p>
      {/if}
    </header>

    <div class="list">
      {#if loaded && listed.length}
        {#if writable.length > 1}
          <label class="all">
            <input type="checkbox" checked={all} indeterminate={count > 0 && !all} onchange={(e) => checkAll(e.currentTarget.checked)} />
            <span>Check all</span>
          </label>
        {/if}
        <ul>
          {#each listed as row (row.id)}
            <li class:blocked={row.status === 'blocked'}>
              {#if row.status === 'blocked'}
                <div class="row">
                  <span class="mark"><Icon name="warning" size={14} /></span>
                  <span class="text">
                    <span class="line"><b>{row.alias}</b> <span class="state">{STATE[row.status]}</span></span>
                    <small>{row.detail}</small>
                  </span>
                </div>
              {:else}
                <label class="row">
                  <input type="checkbox" bind:checked={checked[row.id]} />
                  <span class="text">
                    <span class="line"><b>{row.alias}</b> <span class="state">{STATE[row.status]}</span></span>
                    <small class="mono">{row.detail}</small>
                  </span>
                </label>
              {/if}
            </li>
          {/each}
        </ul>
        {#if upToDate}
          <p class="count">{plural(upToDate, 'host')} {upToDate === 1 ? 'is' : 'are'} already up to date in the file.</p>
        {/if}
      {:else if loaded && !failure}
        <p class="none">
          {#if !rows.length}
            No hosts: nothing to export.
          {:else if single}
            {rows[0].alias} is already up to date in ~/.ssh/config.
          {:else}
            All hosts are already up to date in ~/.ssh/config.
          {/if}
        </p>
      {/if}
    </div>

    <!-- The way out, then the export, on the right; on the left, why it is disabled, or what failed. -->
    <footer>
      <div class="result">
        {#if failure}
          <p class="bad" role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>
        {:else if writable.length && !count && tried}
          <p class="tried" role="alert"><Icon name="warning" size={14} /><span>{WHY}</span></p>
        {:else if writable.length && !count}
          <p class="why">{WHY}</p>
        {:else if loaded && listed.length && !writable.length}
          <p class="why">None of these hosts can be exported.</p>
        {/if}
      </div>
      <!-- The last line shows once the file is read: its buttons do not change under the pointer. -->
      {#if loaded && writable.length}
        <button class="btn" type="button" disabled={busy} onclick={close}>Cancel</button>
        <button
          class="btn primary"
          type="submit"
          disabled={!count || busy}
          title={count ? undefined : WHY}
          onclick={(e) => (byKeyboard = e.detail === 0)}
        >
          {busy ? 'Exporting…' : count ? `Export ${plural(count, 'host')}` : 'Export'}
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
  code {
    font-family: var(--f-mono);
    font-size: var(--fs-code);
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
  .all {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 0;
  }
  /* The checkbox and the icon of a host that is not exported take the same column, aligned with the name. */
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
  li.blocked .state {
    color: var(--warn);
  }
  small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .count,
  .none {
    margin: 0;
    padding: 8px 0;
    color: var(--muted);
  }
  .none {
    padding: 12px 0 4px;
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
