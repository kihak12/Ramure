<script lang="ts">
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import CodeEditor, { type Mark } from '../ui/CodeEditor.svelte';
  import { dialogs } from '../ui/dialogs.svelte';
  import Icon from '../ui/Icon.svelte';
  import { locate, parseConflicts, splitLines, type ConflictBlock } from './conflicts';
  import type { Repo } from './repo.svelte';
  import type { ConflictFile, FileChange } from './types';
  import { motion } from '../ui/motion';

  let { repo, file }: { repo: Repo; file: FileChange } = $props();

  let data = $state.raw<ConflictFile | null>(null);
  let error = $state('');
  // Text of the result, kept up to date by the bottom editor.
  let text = $state('');
  let index = $state(0);
  let showBase = $state(false);

  let result = $state<ReturnType<typeof CodeEditor>>();
  let oursView = $state<ReturnType<typeof CodeEditor>>();
  let theirsView = $state<ReturnType<typeof CodeEditor>>();

  // During a rebase, git swaps the roles: “ours” is the base, “theirs” the commit being replayed.
  const rebase = $derived(repo.status?.operation === 'rebase');
  const names = $derived(
    rebase ? { ours: 'The base', theirs: 'Your commit' } : { ours: 'Your version', theirs: 'The incoming version' },
  );

  const separator = $derived(data?.merged?.includes('\r\n') ? '\r\n' : '\n');
  const lines = $derived(splitLines(text, separator));
  const blocks = $derived(parseConflicts(lines));
  const current = $derived<ConflictBlock | null>(blocks[Math.min(index, blocks.length - 1)] ?? null);
  const editable = $derived(!!data && !data.binary && data.merged !== null && data.ours !== null && data.theirs !== null);
  const modified = $derived(!!data && text !== (data.merged ?? ''));

  const oursLines = $derived(splitLines(data?.ours ?? '', separator));
  const theirsLines = $derived(splitLines(data?.theirs ?? '', separator));
  // Where the passage of the current conflict is, in each version.
  const oursAt = $derived(current ? locate(oursLines, current.ours, current.start) : -1);
  const theirsAt = $derived(current ? locate(theirsLines, current.theirs, current.start) : -1);

  const resultMarks = $derived.by(() => {
    const marks: Mark[] = [];
    for (const block of blocks) {
      const middle = block.theirsAt - 1;
      const oursEnd = block.baseAt >= 0 ? block.baseAt : middle;
      marks.push({ from: block.start + 1, to: block.start + 1, kind: 'marker' });
      if (block.ours.length) marks.push({ from: block.oursAt + 1, to: oursEnd, kind: 'ours' });
      if (block.baseAt >= 0) {
        marks.push({ from: block.baseAt + 1, to: block.baseAt + 1, kind: 'marker' });
        if (block.base?.length) marks.push({ from: block.baseAt + 2, to: middle, kind: 'base' });
      }
      marks.push({ from: middle + 1, to: middle + 1, kind: 'marker' });
      if (block.theirs.length) marks.push({ from: block.theirsAt + 1, to: block.end, kind: 'theirs' });
      marks.push({ from: block.end + 1, to: block.end + 1, kind: 'marker' });
    }
    return marks;
  });
  const sideMarks = (at: number, length: number): Mark[] => (at >= 0 ? [{ from: at + 1, to: at + length, kind: 'focus' }] : []);
  const oursMarks = $derived(current ? sideMarks(oursAt, current.ours.length) : []);
  const theirsMarks = $derived(current ? sideMarks(theirsAt, current.theirs.length) : []);

  // Loading the versions of the file.
  $effect(() => {
    const target = file;
    data = null;
    error = '';
    repo
      .readConflict(target)
      .then((loaded) => {
        if (target !== file) return;
        data = loaded;
        text = loaded.merged ?? '';
        index = 0;
      })
      .catch((e: unknown) => {
        if (target === file) error = e instanceof Error ? e.message : String(e);
      });
  });

  // The current conflict is brought into view in the three panes: when it changes, or when one
  // fewer is left. Never during plain typing in the result.
  $effect(() => {
    void index;
    void blocks.length;
    void data;
    untrack(() => {
      const block = current;
      if (!block) return;
      const [mine, theirs] = [oursAt, theirsAt];
      // After the documents are updated, not during.
      queueMicrotask(() => {
        result?.scrollToLine(block.start + 1 + Math.floor((block.end - block.start) / 2));
        if (mine >= 0) oursView?.scrollToLine(mine + 1);
        if (theirs >= 0) theirsView?.scrollToLine(theirs + 1);
      });
    });
  });

  type Choice = 'ours' | 'theirs' | 'both' | 'both-reversed';

  const chosen = (block: ConflictBlock, choice: Choice) =>
    choice === 'ours'
      ? block.ours
      : choice === 'theirs'
        ? block.theirs
        : choice === 'both'
          ? [...block.ours, ...block.theirs]
          : [...block.theirs, ...block.ours];

  function take(choice: Choice) {
    if (current) result?.replaceLines(current.start + 1, current.end + 1, chosen(current, choice));
  }

  /** Applies the same choice to all the remaining conflicts, from the last to the first: the positions stay right. */
  function takeAll(choice: 'ours' | 'theirs') {
    for (const block of [...blocks].reverse()) result?.replaceLines(block.start + 1, block.end + 1, chosen(block, choice));
  }

  const step = (by: number) => (index = (Math.min(index, blocks.length - 1) + by + blocks.length) % blocks.length);

  async function close() {
    if (modified) {
      const sure = await dialogs.confirm({
        title: 'Close without saving?',
        body: 'The choices and corrections made in this editor will be lost. The file keeps its conflict markers.',
        confirmLabel: 'Close without saving',
        danger: true,
      });
      if (!sure) return;
    }
    repo.conflictFile = null;
  }

  const save = () => void repo.saveConflict(file, result?.getText() ?? text);
</script>

<section class="conflict" aria-label="Conflict editor: {file.path}" in:fly={{ y: 16, duration: motion.med }} out:fly={{ y: 10, duration: motion.fast }}>
  <header>
    <span class="glyph"><Icon name="merge" size={18} /></span>
    <div class="names">
      <b class="mono">{file.path}</b>
      <small>
        {#if !data && !error}
          Reading the versions…
        {:else if blocks.length}
          {blocks.length} conflict{blocks.length === 1 ? '' : 's'} to resolve
        {:else if editable}
          No conflict markers left: the file can be saved
        {/if}
      </small>
    </div>
    <button class="btn" type="button" onclick={close}><Icon name="x" size={14} /> Close</button>
    <button class="btn primary" type="button" disabled={!editable || blocks.length > 0 || repo.locked} onclick={save} title="Write the file and mark it as resolved (git add)">
      <Icon name="check" size={14} /> Save and mark resolved
    </button>
  </header>

  {#if error}
    <p class="note err">{error}</p>
  {:else if !data}
    <p class="note">Reading the versions of the file…</p>
  {:else if !editable}
    <div class="note">
      <p>
        {#if data.binary}
          This file is not text, or it is too large for the editor.
        {:else}
          One of the two sides deleted this file: there are not two texts to bring together.
        {/if}
        Resolve it by keeping one side.
      </p>
      <div class="pick">
        <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.resolve(file, 'ours')}>Keep {names.ours.toLowerCase()}</button>
        <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.resolve(file, 'theirs')}>Keep {names.theirs.toLowerCase()}</button>
      </div>
    </div>
  {:else}
    <div class="sides">
      <div class="side ours">
        <div class="bar">
          <span class="dot"></span>
          <b>{names.ours}</b>
          {#if current?.oursLabel}<small class="mono">{current.oursLabel}</small>{/if}
          <button class="btn small" type="button" disabled={!blocks.length} onclick={() => takeAll('ours')} title="Take this side for all the remaining conflicts">Take all</button>
        </div>
        <CodeEditor bind:this={oursView} value={data.ours ?? ''} readonly marks={oursMarks} lineSeparator={separator} label="{names.ours}, read-only" />
      </div>
      <div class="side theirs">
        <div class="bar">
          <span class="dot"></span>
          <b>{names.theirs}</b>
          {#if current?.theirsLabel}<small class="mono">{current.theirsLabel}</small>{/if}
          <button class="btn small" type="button" disabled={!blocks.length} onclick={() => takeAll('theirs')} title="Take this side for all the remaining conflicts">Take all</button>
        </div>
        <CodeEditor bind:this={theirsView} value={data.theirs ?? ''} readonly marks={theirsMarks} lineSeparator={separator} label="{names.theirs}, read-only" />
      </div>
    </div>

    <div class="choose">
      {#if current}
        <div class="nav">
          <button class="btn small" type="button" aria-label="Previous conflict" disabled={blocks.length < 2} onclick={() => step(-1)}><Icon name="up" size={14} /></button>
          <span class="count">Conflict <b>{Math.min(index, blocks.length - 1) + 1}</b> of {blocks.length}</span>
          <button class="btn small" type="button" aria-label="Next conflict" disabled={blocks.length < 2} onclick={() => step(1)}><Icon name="down" size={14} /></button>
        </div>
        <button class="btn small take ours" type="button" onclick={() => take('ours')}>Take {names.ours.toLowerCase()}</button>
        <button class="btn small take theirs" type="button" onclick={() => take('theirs')}>Take {names.theirs.toLowerCase()}</button>
        <button class="btn small" type="button" onclick={() => take('both')} title="{names.ours}, then {names.theirs.toLowerCase()}">Take both</button>
        <button class="btn small" type="button" onclick={() => take('both-reversed')} title="{names.theirs}, then {names.ours.toLowerCase()}">Take both, reversed</button>
        {#if data.base !== null}
          <label class="base-toggle"><input type="checkbox" bind:checked={showBase} /> Show the common ancestor</label>
        {/if}
      {:else}
        <span class="done"><Icon name="check" size={16} /> All conflicts are resolved. Read the result again, correct it if needed, then save.</span>
        {#if data.base !== null}
          <label class="base-toggle"><input type="checkbox" bind:checked={showBase} /> Show the common ancestor</label>
        {/if}
      {/if}
    </div>

    <div class="bottom" class:split={showBase && data.base !== null}>
      <div class="side result">
        <div class="bar">
          <span class="dot"></span>
          <b>Result</b>
          <small>editable: the choices above apply to it, and you can correct it by hand</small>
        </div>
        <CodeEditor bind:this={result} value={data.merged ?? ''} marks={resultMarks} lineSeparator={separator} label="Result of the resolution" onchange={(next) => (text = next)} />
      </div>
      {#if showBase && data.base !== null}
        <div class="side base">
          <div class="bar"><span class="dot"></span><b>Common ancestor</b><small>before both changes</small></div>
          <CodeEditor value={data.base} readonly lineSeparator={separator} label="Common ancestor, read-only" />
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  /* One card, in place of the three of the view. */
  .conflict {
    position: absolute;
    inset: 0 8px 8px;
    z-index: 6;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--panel);
    min-height: 0;
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-soft);
    min-width: 0;
  }
  .glyph {
    display: grid;
    color: var(--warn);
  }
  .names {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .names b,
  .names small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .names small {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .note {
    margin: 0;
    padding: 20px;
    color: var(--muted);
  }
  .note p {
    margin: 0 0 12px;
    max-width: 70ch;
  }
  .note.err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .pick {
    display: flex;
    gap: 8px;
  }

  .sides {
    flex: 1 1 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    min-height: 0;
  }
  .bottom {
    flex: 1.15 1 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
  }
  .bottom.split {
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
  }
  .side {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    --c: var(--muted);
  }
  .side + .side {
    border-left: 1px solid var(--line-soft);
  }
  .ours {
    --c: var(--l0);
  }
  .theirs {
    --c: var(--l1);
  }
  .result {
    --c: var(--ok);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--line-soft);
    background: color-mix(in srgb, var(--c) 9%, var(--panel));
    min-width: 0;
  }
  .bar small {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    font-size: var(--fs-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar b {
    white-space: nowrap;
  }
  .bar b:only-of-type:last-child {
    flex: 1;
  }
  .dot {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
  }

  .choose {
    flex: none;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 12px;
    border-top: 1px solid var(--line-soft);
    border-bottom: 1px solid var(--line-soft);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: 6px;
  }
  .count {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .take {
    border-color: color-mix(in srgb, var(--c) 55%, var(--line));
    background: color-mix(in srgb, var(--c) 12%, var(--raised));
  }
  .take:hover:not(:disabled) {
    background: color-mix(in srgb, var(--c) 24%, var(--raised));
    border-color: var(--c);
  }
  .take.ours {
    --c: var(--l0);
  }
  .take.theirs {
    --c: var(--l1);
  }
  .done {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--ok);
  }
  .base-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    color: var(--muted);
  }
</style>
