<script lang="ts">
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import type { ChangeKind, FileChange } from './types';

  interface Action {
    label: string;
    /** Icon of the button; `label` stays the accessible name. */
    icon: IconName;
    run: (file: FileChange) => void;
    danger?: boolean;
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

  function split(path: string) {
    const cut = path.lastIndexOf('/');
    return cut < 0 ? { dir: '', name: path } : { dir: path.slice(0, cut + 1), name: path.slice(cut + 1) };
  }
</script>

<ul class="files">
  {#each files as file (file.path)}
    {@const parts = split(file.path)}
    <li class:active={file.path === active}>
      <button class="open" type="button" onclick={() => onopen(file)} title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}>
        <span class="kind k-{file.kind}" title={WORD[file.kind]} aria-label={WORD[file.kind]}>{LETTER[file.kind]}</span>
        <span class="path"><span class="dir">{parts.dir}{parts.dir ? '\u200e' : ''}</span><span class="name">{parts.name}</span></span>
        {#if partial?.has(file.path)}
          <span class="part" title="Only some of the changes in this file are staged">partly</span>
        {/if}
      </button>
      {#each actions as action (action.label)}
        <button
          class="act"
          class:danger={action.danger}
          type="button"
          title={action.label}
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
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 3px 6px;
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
    color: var(--c);
    background: color-mix(in srgb, var(--c) 16%, transparent);
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
