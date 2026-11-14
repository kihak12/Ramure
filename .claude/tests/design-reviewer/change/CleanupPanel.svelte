<script lang="ts">
  // "Clean up branches" panel: lists the local branches already merged and offers to delete them.
  import Icon from '../ui/Icon.svelte';
  import { confirmTitle, failure, summary } from './cleanup';
  import type { Repo } from './repo.svelte';

  interface Merged {
    name: string;
    sha: string;
    merged: boolean;
    commits: number;
  }

  let { repo, branches, onclose }: { repo: Repo; branches: Merged[]; onclose: () => void } = $props();

  let filter = $state('');
  let selected = $state<string[]>([]);
  let active = $state(0);
  let loading = $state(false);
  let confirming = $state(false);
  let error = $state('');

  const shown = $derived(branches.filter((b) => b.name.includes(filter.trim())));

  function toggle(name: string) {
    selected = selected.includes(name) ? selected.filter((n) => n !== name) : [...selected, name];
  }

  // The list keeps the focus and handles the arrow keys and the space bar itself.
  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = Math.min(shown.length - 1, active + 1);
    else if (e.key === 'ArrowUp') active = Math.max(0, active - 1);
    else if (e.key === ' ' && shown[active]) toggle(shown[active].name);
    else return;
    e.preventDefault();
  }

  function onEscape(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !confirming) return;
    e.preventDefault();
    confirming = false;
  }

  async function fetchAll() {
    loading = true;
    await repo.sync('fetch');
    loading = false;
  }

  async function remove() {
    confirming = false;
    error = '';
    for (const name of selected) {
      const ok = await repo.refOp({ op: 'branch-delete', name, force: false }, `Branch ${name} deleted`);
      if (!ok) {
        error = failure();
        break;
      }
    }
    selected = [];
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="cleanup" aria-label="Clean up branches" onkeydown={onEscape}>
  <header>
    <h3>Merged branches</h3>
    <span class="count">{branches.length}</span>
    <button class="btn primary small" type="button" disabled={loading} onclick={() => void fetchAll()}>
      {#if loading}<span class="spin"><Icon name="refresh" size={14} /></span>{/if}
      {loading ? 'Retrieving…' : 'Retrieve'}
    </button>
    <button class="btn ghost small" type="button" aria-label="Close panel" title="Close panel" onclick={onclose}>
      <Icon name="x" size={14} />
    </button>
  </header>

  <p class="help">Check the branches to delete, then confirm with “Delete.” A merged branch has no commits of its own left.</p>

  <label class="filter">
    <Icon name="search" size={14} />
    <input type="text" placeholder="Filter branches" aria-label="Filter branches" bind:value={filter} />
    {#if filter}
      <button class="clear" type="button" aria-label="Clear filter" title="Clear filter" onclick={() => (filter = '')}>
        <Icon name="x" size={14} />
      </button>
    {/if}
  </label>

  <div class="columns">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="list" role="listbox" aria-label="Merged branches" aria-multiselectable="true" tabindex="0" onkeydown={onKey}>
      {#each shown as branch, i (branch.name)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          class:sel={selected.includes(branch.name)}
          class:active={i === active}
          role="option"
          tabindex="-1"
          aria-selected={selected.includes(branch.name)}
          onclick={() => toggle(branch.name)}
        >
          <input type="checkbox" tabindex="-1" checked={selected.includes(branch.name)} aria-hidden="true" />
          <span class="state" class:merged={branch.merged}></span>
          <span class="name" title={branch.name}>{branch.name}</span>
          <span class="sha">{branch.sha.slice(0, 7)}</span>
          <span class="commits">{branch.commits} commit{branch.commits === 1 ? '' : 's'}</span>
        </div>
      {/each}
    </div>

    <aside class="summary">
      <span class="label">Selection</span>
      <p>{summary(selected.length)}</p>
      <p class="note">The current branch and HEAD are never offered.</p>
    </aside>
  </div>

  {#if error}<p class="error" role="alert">⚠️ {error}</p>{/if}

  <footer>
    <button class="btn" type="button" onclick={onclose}>Close</button>
    <button class="btn primary" type="button" tabindex="1" disabled={!selected.length} onclick={() => (confirming = true)}>
      <Icon name="trash" size={15} /> Delete Selection…
    </button>
  </footer>

  {#if confirming}
    <div class="confirmation" role="alertdialog" aria-labelledby="cleanup-confirm">
      <h4 id="cleanup-confirm">{confirmTitle(selected.length)}</h4>
      <div class="actions">
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn danger" type="button" autofocus onclick={() => void remove()}>Delete</button>
        <button class="btn" type="button" onclick={() => (confirming = false)}>Cancel</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .cleanup {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: var(--radius-card);
    background: var(--panel);
    box-shadow: var(--shadow-soft);
    animation: enter var(--t-med) var(--ease);
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(48px);
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h3 {
    flex: 1;
    margin: 0;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--accent);
  }
  .count {
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--sel);
    font-size: var(--fs-small);
  }
  .spin {
    display: grid;
    animation: spin var(--t-spin) linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .help {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .filter:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    padding: 6px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .filter input::placeholder {
    color: var(--faint);
  }
  .clear {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .clear:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 200px;
    gap: 12px;
    min-height: 0;
  }
  .list {
    overflow-y: auto;
    max-height: 260px;
    padding-right: 12px;
    border-right: 1px solid var(--line-soft);
  }
  /* The focus reads on the active row, not as a frame around the list. */
  .list:focus-visible {
    outline: none;
  }
  .list:focus-visible .row.active {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover {
    background: var(--sel);
  }
  .row.sel {
    background: var(--sel);
  }
  .row input {
    appearance: auto;
    width: 12px;
    height: 12px;
    margin: 0;
  }
  .state {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--warn);
  }
  .state.merged {
    background: var(--ok);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sha {
    color: var(--muted);
    font-size: var(--fs-code);
  }
  .commits {
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
  }
  .summary {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border-radius: var(--radius-card);
    background: var(--raised);
  }
  .summary p {
    margin: 0;
  }
  .note {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .error {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--bad-soft);
    color: var(--bad);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 10px;
    border-top: 2px solid var(--ok);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .confirmation {
    position: absolute;
    right: 12px;
    bottom: 56px;
    z-index: 5;
    display: grid;
    gap: 10px;
    width: min(360px, calc(100% - 24px));
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    animation: pop-in var(--t-med) var(--ease);
  }
  h4 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
