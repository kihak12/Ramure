<script lang="ts">
  import { tick } from 'svelte';
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import type { ChangeKind, FileChange } from './types';

  interface Action {
    label: string;
    /** Icon of the button; `label` stays the accessible name. */
    icon: IconName;
    /** What the keyboard waits for before giving the row that takes its place the focus. */
    run: (file: FileChange) => unknown;
    danger?: boolean;
    /** The key that runs it on the current row, from the keyboard (guidelines, 9.6). */
    key?: 'Space' | 'Delete';
  }

  let {
    files,
    partial = null,
    active = null,
    actions = [],
    disabled = false,
    onopen,
  }: {
    files: FileChange[];
    /** Paths of the files that are only partly staged. */
    partial?: Set<string> | null;
    /** Path of the file whose diff is open. */
    active?: string | null;
    actions?: Action[];
    disabled?: boolean;
    onopen: (file: FileChange) => void;
  } = $props();

  const LETTER: Record<ChangeKind, string> = {
    added: 'A',
    modified: 'M',
    deleted: 'D',
    renamed: 'R',
    copied: 'C',
    typechange: 'T',
    untracked: 'U',
    conflict: '!',
  };
  const WORD: Record<ChangeKind, string> = {
    added: 'Added',
    modified: 'Modified',
    deleted: 'Deleted',
    renamed: 'Renamed',
    copied: 'Copied',
    typechange: 'Type changed',
    untracked: 'Untracked',
    conflict: 'Conflict',
  };

  const numFmt = new Intl.NumberFormat('en-US');
  /** "+12 −3", without the side that is zero; "binary" for a binary file (guidelines, 9.6). */
  function counted(lines: NonNullable<FileChange['lines']>) {
    if (lines.binary) return 'binary';
    const parts = [];
    if (lines.added) parts.push(`+${numFmt.format(lines.added)}`);
    if (lines.removed) parts.push(`−${numFmt.format(lines.removed)}`);
    return parts.join(' ');
  }

  // The list is one stop of the tab order: the arrows go through its rows (guidelines, 9.6).
  let listEl = $state<HTMLUListElement>();
  let current = $state<string | null>(null);
  const focusable = $derived(
    files.some((f) => f.path === current) ? current : files.some((f) => f.path === active) ? active : (files[0]?.path ?? null),
  );

  function focusRow(index: number, ring = false) {
    const rows = listEl?.querySelectorAll<HTMLElement>('button.open');
    const row = rows?.[Math.max(0, Math.min(rows.length - 1, index))];
    if (!row) return;
    current = files[Math.max(0, Math.min(files.length - 1, index))]?.path ?? null;
    if (ring) focusWithRing(row);
    else row.focus();
  }

  /**
   * Runs the action of a key, then gives the keyboard back if the file took it away when it left
   * the list: to the row that takes its place, or, the list being empty, to the next list of the
   * panel, the previous one, or the commit message (guidelines, 9.6).
   */
  async function runKey(action: Action, file: FileChange, at: number) {
    const scope = listEl?.closest<HTMLElement>('[data-file-lists]');
    const lists = scope ? [...scope.querySelectorAll<HTMLElement>('ul.files')] : [];
    const mine = listEl ? lists.indexOf(listEl) : -1;
    running = true;
    try {
      await action.run(file);
    } finally {
      running = false;
    }
    await tick();
    const now = document.activeElement;
    if (now && now !== document.body && now.isConnected) return;
    if (listEl?.isConnected && files.length) return focusRow(Math.min(at, files.length - 1), true);
    const next = [...lists.slice(mine + 1), ...lists.slice(0, Math.max(0, mine)).reverse()].find(
      (list) => list.isConnected && list.querySelector('button.open'),
    );
    focusWithRing(next?.querySelector<HTMLElement>('button.open[tabindex="0"]') ?? scope?.querySelector<HTMLElement>('textarea'));
  }
  let running = false;

  /** ↑ ↓ Home End move; Space and Delete run the action that has that key, the keyboard staying in the list. */
  function rowKey(e: KeyboardEvent, file: FileChange) {
    const at = files.findIndex((f) => f.path === file.path);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      focusRow(at + (e.key === 'ArrowDown' ? 1 : -1));
      return;
    }
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      focusRow(e.key === 'Home' ? 0 : files.length - 1);
      return;
    }
    const key = e.key === ' ' ? 'Space' : e.key === 'Delete' ? 'Delete' : null;
    const action = key && actions.find((a) => a.key === key);
    // Where no action has the key, Space opens the row, as on any button.
    if (!action) return;
    e.preventDefault();
    // While the changes are being written, the key does nothing: it neither runs twice nor opens.
    if (disabled || running || e.repeat) return;
    void runKey(action, file, at);
  }

  function split(path: string) {
    const cut = path.lastIndexOf('/');
    return cut < 0 ? { dir: '', name: path } : { dir: path.slice(0, cut + 1), name: path.slice(cut + 1) };
  }
</script>

<ul class="files" bind:this={listEl}>
  {#each files as file (file.path)}
    {@const parts = split(file.path)}
    <li class:active={file.path === active}>
      <button
        class="open"
        type="button"
        tabindex={file.path === focusable ? 0 : -1}
        onclick={(e) => {
          // A click makes the row current and gives the list the keyboard, with no ring, as in the
          // other lists (9.6): WebKit does not focus a button it clicks.
          (e.currentTarget as HTMLElement).focus({ preventScroll: true });
          onopen(file);
        }}
        onfocus={() => (current = file.path)}
        onkeydown={(e) => rowKey(e, file)}
        title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
      >
        <span class="kind k-{file.kind}" title={WORD[file.kind]} aria-label={WORD[file.kind]}>{LETTER[file.kind]}</span>
        <span class="path"><span class="dir">{parts.dir}{parts.dir ? '\u200e' : ''}</span><span class="name">{parts.name}</span></span>
        {#if partial?.has(file.path)}
          <span class="part" title="Only some of the changes in this file are staged">partly</span>
        {/if}
        {#if file.lines}
          {@const count = counted(file.lines)}
          {#if count}<span class="lines">{count}</span>{/if}
        {/if}
      </button>
      {#each actions as action (action.label)}
        <button
          class="act"
          class:danger={action.danger}
          type="button"
          tabindex={action.key ? -1 : 0}
          title={action.key ? `${action.label} (${action.key})` : action.label}
          aria-label="{action.label}: {file.path}"
          {disabled}
          onclick={() => action.run(file)}><Icon name={action.icon} size={14} /></button
        >
      {/each}
    </li>
  {/each}
</ul>

<style>
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    width: 100%;
  }
  li {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    min-width: 0;
    transition: background var(--t-fast) var(--ease);
  }
  /* The background follows the pointer, and the keyboard focus: a click leaves no hover behind (guidelines, 9.6). */
  li:hover,
  li:has(:focus-visible) {
    background: var(--hover);
  }
  li.active {
    background: var(--sel);
  }
  /* The row of the keyboard: an inner ring on the whole row, its buttons included, never cut by
     the list that scrolls (guidelines, 9.6). */
  .open:focus-visible,
  .open:global(.focus-ring) {
    outline: none;
  }
  li:has(.open:focus-visible),
  li:has(.open:global(.focus-ring)) {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding: 2px 6px;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
  }
  .kind {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: var(--radius-xs);
    display: grid;
    place-items: center;
    font-size: var(--fs-micro);
    font-weight: var(--fw-strong);
    font-family: var(--f-mono);
    /* The letter is halfway between its color and --ink, so that it reads on its tint on a row at
       rest, hovered or selected (guidelines, 2.7). */
    color: color-mix(in srgb, var(--c) 50%, var(--ink));
    background: color-mix(in srgb, var(--c) var(--tint-letter), transparent);
    --c: var(--warn);
  }
  .k-added,
  .k-untracked {
    --c: var(--ok);
  }
  .k-deleted,
  .k-conflict {
    --c: var(--bad);
  }
  .k-renamed,
  .k-copied {
    --c: var(--accent);
  }
  /* The name of the file stays visible: the folder is what gets cut, from the left. */
  .path {
    display: flex;
    min-width: 0;
    white-space: nowrap;
  }
  .dir {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    flex-shrink: 1;
    min-width: 0;
  }
  .name {
    flex: none;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .part {
    flex: none;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  /* At the right of the row: the folder is cut before them (guidelines, 9.6). */
  .lines {
    flex: none;
    margin-left: auto;
    padding-left: 6px;
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .act {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-right: 2px;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  /* The list scrolls and would cut a ring drawn around the button: it is drawn inside (11). */
  .act:focus-visible {
    outline-offset: -2px;
  }
  .act:active:not(:disabled) {
    transform: scale(0.97);
  }
  li:hover .act,
  li:has(:focus-visible) .act {
    opacity: 1;
  }
  .act:hover:not(:disabled) {
    border-color: var(--line);
    background: var(--panel);
    color: var(--ink);
  }
  .act.danger:hover:not(:disabled) {
    color: var(--bad);
  }
  li:hover .act:disabled,
  li:has(:focus-visible) .act:disabled,
  .act:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
