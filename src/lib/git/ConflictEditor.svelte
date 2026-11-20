<script lang="ts">
  // The conflict editor: side 1, the result and side 2 in three columns, aligned and scrolling as
  // one; a side of a conflict is kept with its arrow toward the result (guidelines, 9.26).
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import CodeEditor, { type Mark } from '../ui/CodeEditor.svelte';
  import { dialogs } from '../ui/dialogs.svelte';
  import Icon from '../ui/Icon.svelte';
  import ConflictResult, { type RegionInfo } from './ConflictResult.svelte';
  import ConflictSide, { type SideConflict } from './ConflictSide.svelte';
  import {
    choose,
    emptyPick,
    isDecided,
    keepSide,
    locate,
    parseConflicts,
    resolved,
    setAside,
    sideStatus,
    splitLines,
    type Choice,
    type ConflictBlock,
    type Pick,
    type Side,
  } from './conflicts';
  import type { Repo } from './repo.svelte';
  import { align, mergedChanges } from './threeway';
  import type { ConflictFile, FileChange } from './types';
  import { motion } from '../ui/motion';

  let { repo, file }: { repo: Repo; file: FileChange } = $props();

  let data = $state.raw<ConflictFile | null>(null);
  let error = $state('');
  /** Text of the result, kept up to date by the result. */
  let text = $state('');
  let index = $state(0);
  let showBase = $state(false);

  /** The conflicts of the file as it opened, and what is kept of each. */
  let blocks = $state.raw<ConflictBlock[]>([]);
  let picks = $state.raw<Pick[]>([]);
  let edited = $state.raw<boolean[]>([]);
  /** What each choice changed, to undo it: the state of each conflict before it. */
  let undo: { block: number; pick: Pick; edited: boolean; text: string }[][] = [];
  let lastPickAt = 0;
  let lastTypeAt = 0;

  let result = $state<ReturnType<typeof ConflictResult>>();
  let oursView = $state<ReturnType<typeof ConflictSide>>();
  let theirsView = $state<ReturnType<typeof ConflictSide>>();
  let baseView = $state<ReturnType<typeof CodeEditor>>();

  // During a rebase, git swaps the roles: "ours" is upstream, "theirs" the commit being replayed.
  const rebase = $derived(repo.status?.operation === 'rebase');
  const names = $derived(
    rebase
      ? { ours: 'Upstream', theirs: 'Your commit', keepOurs: 'Keep upstream', keepTheirs: 'Keep your commit' }
      : { ours: 'Your version', theirs: 'The incoming version', keepOurs: 'Keep your version', keepTheirs: 'Keep the incoming version' },
  );

  // Side 1, on the left, is the user's in every operation: in a rebase, the commit being replayed,
  // which git calls "theirs" (guidelines, 9.26). Its chip, its color and the order of "Keep both"
  // follow its place.
  const first = $derived<Side>(rebase ? 'theirs' : 'ours');
  const second = $derived<Side>(rebase ? 'ours' : 'theirs');
  const keepName = (side: Side) => (side === 'ours' ? names.keepOurs : names.keepTheirs);

  const separator = $derived(data?.merged?.includes('\r\n') ? '\r\n' : '\n');
  const editable = $derived(!!data && !data.binary && data.merged !== null && data.ours !== null && data.theirs !== null);
  const modified = $derived(!!data && text !== (data.merged ?? ''));
  /** Markers still in the file: a conflict still to decide. Saving waits for none. */
  const markersLeft = $derived(parseConflicts(splitLines(text, separator)).length);
  const open = $derived(blocks.filter((_, i) => !edited[i] && !isDecided(picks[i])).length);
  const current = $derived<ConflictBlock | null>(blocks[Math.min(index, blocks.length - 1)] ?? null);

  // Where each conflict is in each side, and in the common ancestor.
  const oursLines = $derived(splitLines(data?.ours ?? '', separator));
  const theirsLines = $derived(splitLines(data?.theirs ?? '', separator));
  const baseLines = $derived(splitLines(data?.base ?? '', separator));
  const oursAt = $derived(blocks.map((b) => (b.ours.length ? locate(oursLines, b.ours, b.start) : -1)));
  const theirsAt = $derived(blocks.map((b) => (b.theirs.length ? locate(theirsLines, b.theirs, b.start) : -1)));
  const baseAt = $derived(blocks.map((b) => (b.base?.length ? locate(baseLines, b.base, b.start) : -1)));

  // The changes that do not conflict, which git has already merged into the result: tinted in the
  // sides, with their chip in the result (guidelines, 9.26).
  const changes = $derived.by(() => {
    if (!data || !editable) return null;
    const span = (at: number, count: number) => ({ from: at, to: at + count });
    // The conflicts of a side, or null when one of them is not found there.
    const spans = (side: Side, at: number[]) =>
      blocks.some((b, i) => b[side].length && at[i] < 0) ? null : blocks.flatMap((b, i) => (at[i] >= 0 ? [span(at[i], b[side].length)] : []));
    return mergedChanges(
      data.base === null ? null : baseLines,
      oursLines,
      theirsLines,
      splitLines(data.merged ?? '', separator),
      spans('ours', oursAt),
      spans('theirs', theirsAt),
      blocks.map((b) => span(b.start, b.end - b.start + 1)),
    );
  });

  const sideOf = (side: Side): SideConflict[] =>
    blocks.map((b, i) => ({
      index: i,
      at: (side === 'ours' ? oursAt : theirsAt)[i],
      count: b[side].length,
      status: picks[i] ? sideStatus(picks[i], side) : 'open',
      edited: !!edited[i],
    }));
  const oursConflicts = $derived(sideOf('ours'));
  const theirsConflicts = $derived(sideOf('theirs'));

  const info = $derived<RegionInfo[]>(
    blocks.map((b, i) => ({
      id: i,
      state: edited[i] ? 'edited' : isDecided(picks[i]) ? 'kept' : 'open',
      origins: resolved(b, picks[i]).map((r) => r.side),
      band: `Conflict ${i + 1} of ${blocks.length}: keep a side with its arrow`,
    })),
  );
  const baseMarks = $derived<Mark[]>(current && baseAt[index] >= 0 ? [{ from: baseAt[index] + 1, to: baseAt[index] + (current.base?.length ?? 0), kind: 'focus' }] : []);

  // Loading the versions of the file, and the conflicts as it opens.
  $effect(() => {
    const target = file;
    data = null;
    error = '';
    repo
      .readConflict(target)
      .then((loaded) => {
        if (target !== file) return;
        const sep = loaded.merged?.includes('\r\n') ? '\r\n' : '\n';
        const parsed = parseConflicts(splitLines(loaded.merged ?? '', sep));
        blocks = parsed;
        picks = parsed.map(() => emptyPick());
        edited = parsed.map(() => false);
        undo = [];
        text = loaded.merged ?? '';
        index = 0;
        data = loaded;
      })
      .catch((e: unknown) => {
        if (target === file) error = e instanceof Error ? e.message : String(e);
      });
  });

  // The current conflict comes into view in every pane, when it changes. Never while typing.
  $effect(() => {
    void index;
    void data;
    untrack(() => {
      const i = Math.min(index, blocks.length - 1);
      if (i < 0) return;
      queueMicrotask(() => {
        // The sides follow the result: the three columns scroll as one.
        result?.scrollToRegion(i);
        if (baseAt[i] >= 0) baseView?.scrollToLine(baseAt[i] + 1);
        oursView?.follow(i);
        theirsView?.follow(i);
      });
    });
  });

  // --- The three columns, aligned and scrolling as one (guidelines, 9.26) ----------------------

  /** Lines up the rows of the three columns: blanks where a column has fewer rows for a change. */
  function realign() {
    const [left, right] = first === 'ours' ? [oursView, theirsView] : [theirsView, oursView];
    if (!result || !left || !right) return;
    const out = align(left.rows(), result.rows(), right.rows());
    left.setSpacers(out.left);
    result.setSpacers(out.result);
    right.setSpacers(out.right);
  }

  // Again after each change of the result, a pick or typing, once the panes have drawn it.
  let alignFrame = 0;
  $effect(() => {
    void text;
    void info;
    void oursConflicts;
    void theirsConflicts;
    if (!result || !oursView || !theirsView) return;
    cancelAnimationFrame(alignFrame);
    alignFrame = requestAnimationFrame(realign);
    return () => cancelAnimationFrame(alignFrame);
  });

  // A scroll of one column moves the three. The others' scrolls that this sets are not followed in
  // turn: the column that leads keeps the lead until the next frame.
  $effect(() => {
    const panes = [oursView?.scroller(), result?.scroller(), theirsView?.scroller()];
    if (panes.some((p) => !p)) return;
    const scrollers = panes as HTMLElement[];
    let leader: HTMLElement | null = null;
    let release = 0;
    const offs = scrollers.map((pane) => {
      const onScroll = () => {
        if (leader && leader !== pane) return;
        leader = pane;
        cancelAnimationFrame(release);
        release = requestAnimationFrame(() => (leader = null));
        for (const other of scrollers) if (other !== pane && other.scrollTop !== pane.scrollTop) other.scrollTop = pane.scrollTop;
      };
      pane.addEventListener('scroll', onScroll, { passive: true });
      return () => pane.removeEventListener('scroll', onScroll);
    });
    return () => {
      cancelAnimationFrame(release);
      for (const off of offs) off();
    };
  });

  /** The text a conflict takes in the result: the lines kept, or, still to decide, its markers. */
  function regionOf(i: number, pick: Pick): { text: string; empty: boolean } {
    const brk = separator;
    if (!isDecided(pick)) return { text: splitLines(data?.merged ?? '', brk).slice(blocks[i].start, blocks[i].end + 1).join(brk), empty: false };
    const lines = resolved(blocks[i], pick).map((r) => r.line);
    return { text: lines.join(brk), empty: lines.length === 0 };
  }

  /**
   * Applies new picks to some conflicts, as one choice to undo. A conflict typed in by hand asks
   * first: its lines give way to those kept, and Ctrl + Z brings them back.
   */
  async function apply(changes: { block: number; pick: Pick }[]) {
    if (!result || !changes.length) return;
    const typed = changes.filter((c) => edited[c.block]);
    if (typed.length) {
      const which = typed.length === 1 ? `conflict ${typed[0].block + 1}` : `${typed.length} conflicts`;
      const sure = await dialogs.confirm({
        title: 'Replace your edits?',
        body: `The lines typed in ${which} give way to the lines kept. Ctrl + Z brings them back.`,
        confirmLabel: 'Replace edits',
      });
      if (!sure) return;
    }
    const before = changes.map((c) => ({ block: c.block, pick: picks[c.block], edited: edited[c.block], text: result!.regionText(c.block) }));
    const nextPicks = [...picks];
    const nextEdited = [...edited];
    for (const c of changes) {
      nextPicks[c.block] = c.pick;
      nextEdited[c.block] = false;
      const region = regionOf(c.block, c.pick);
      result.setRegionText(c.block, region.text, region.empty);
    }
    picks = nextPicks;
    edited = nextEdited;
    undo.push(before);
    lastPickAt = performance.now();
  }

  /** Ctrl + Z: the last choice, when it came after the last typing. */
  function undoPick(): boolean {
    const last = undo.at(-1);
    if (!last || !result || lastTypeAt > lastPickAt) return false;
    undo.pop();
    const nextPicks = [...picks];
    const nextEdited = [...edited];
    for (const item of last) {
      nextPicks[item.block] = item.pick;
      nextEdited[item.block] = item.edited;
      // A conflict typed in by hand gets its typing back; any other, the text of its picks.
      if (item.edited) result.setRegionText(item.block, item.text, item.text === '');
      else {
        const region = regionOf(item.block, item.pick);
        result.setRegionText(item.block, region.text, region.empty);
      }
    }
    picks = nextPicks;
    edited = nextEdited;
    lastPickAt = undo.length ? performance.now() : 0;
    return true;
  }

  /** The arrow of a side, in the header of a conflict: the side kept whole, after one already kept. */
  function keep(side: Side, block: number) {
    void apply([{ block, pick: keepSide(picks[block], side) }]);
    index = block;
  }

  /** The cross of a side: its lines go; both set aside, the conflict is decided with no line. */
  function aside(side: Side, block: number) {
    void apply([{ block, pick: setAside(picks[block], side) }]);
    index = block;
  }

  /** A button of the bar, on the current conflict. */
  function take(choice: Choice) {
    const i = Math.min(index, blocks.length - 1);
    if (i >= 0) void apply([{ block: i, pick: choose(choice, first) }]);
  }

  /** "Start over": the current conflict as it opened, its markers back. */
  function startOver() {
    const i = Math.min(index, blocks.length - 1);
    if (i >= 0) void apply([{ block: i, pick: emptyPick() }]);
  }

  /** "Keep all": a side in every conflict left to decide. */
  function keepAll(side: Side) {
    void apply(blocks.flatMap((_, i) => (!edited[i] && !isDecided(picks[i]) ? [{ block: i, pick: choose(side, first) }] : [])));
  }

  /** Typing in a conflict: edited by hand from now on. */
  function onEdit(ids: number[]) {
    lastTypeAt = performance.now();
    const next = [...edited];
    for (const id of ids) next[id] = true;
    edited = next;
  }

  const step = (by: number) => {
    if (!blocks.length) return;
    index = (Math.min(index, blocks.length - 1) + by + blocks.length) % blocks.length;
  };

  async function close() {
    if (modified) {
      const sure = await dialogs.confirm({
        title: 'Close without saving?',
        body: 'The choices and corrections made in this editor will be lost for good. The file keeps its conflict markers.',
        confirmLabel: 'Close without saving',
        danger: true,
      });
      if (!sure) return;
    }
    repo.conflictFile = null;
  }

  const canSave = $derived(editable && markersLeft === 0 && !repo.locked);
  const save = () => {
    if (canSave) void repo.saveConflict(file, result?.getText() ?? text);
  };

  /** The keys of the editor, wherever the focus is in it (guidelines, 9.26). */
  function onKey(e: KeyboardEvent) {
    if (document.querySelector('dialog[open]')) return;
    if (e.key === 'F8') {
      e.preventDefault();
      step(e.shiftKey ? -1 : 1);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      save();
    } else if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'z' && !(e.target instanceof Element && e.target.closest('.result'))) {
      // In the result, the editor itself asks first whether a choice comes before the typing.
      if (undoPick()) e.preventDefault();
    } else if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      void close();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="conflict"
  aria-label="Conflict editor: {file.path}"
  onkeydown={onKey}
  in:fly={{ y: 10, duration: motion.med }}
  out:fly={{ y: 10, duration: motion.fast }}
>
  <header>
    <span class="glyph"><Icon name="merge" size={18} /></span>
    <div class="names">
      <b class="mono">{file.path}</b>
      <small>
        {#if !data && !error}
          Reading the versions…
        {:else if editable && open}
          {open} conflict{open === 1 ? '' : 's'} to decide
        {:else if editable && markersLeft === 0}
          No conflict left: the file can be saved
        {/if}
      </small>
    </div>
    <button class="btn small" type="button" onclick={close} title="Close (Escape)"><Icon name="x" size={14} /> Close</button>
    <button
      class="btn primary small"
      type="button"
      disabled={!canSave}
      onclick={save}
      title={canSave
        ? 'Write the file and mark it as resolved (Ctrl + S)'
        : !editable
          ? 'Write the file and mark it as resolved'
          : markersLeft
            ? 'Decide every conflict first.'
            : 'Git is working: wait for it to finish.'}
    >
      <Icon name="check" size={14} /> Save and mark resolved
    </button>
  </header>

  {#if error}
    <p class="note err" role="alert">{error}</p>
  {:else if !data}
    <p class="note">Reading the versions of the file…</p>
  {:else if !editable}
    <div class="note">
      <p>
        {#if data.binary}
          This file is not text, or it is too large for the editor.
        {:else}
          One of the two sides deleted this file: there are not two versions to merge.
        {/if}
        Resolve it by keeping one side.
      </p>
      <div class="pick">
        <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.resolve(file, first)}>{keepName(first)}</button>
        <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.resolve(file, second)}>{keepName(second)}</button>
      </div>
    </div>
  {:else}
    <div class="choose">
      {#if current}
        <div class="nav">
          <button class="btn small square" type="button" aria-label="Previous conflict" title="Previous conflict (Shift + F8)" disabled={blocks.length < 2} onclick={() => step(-1)}><Icon name="up" size={14} /></button>
          <span class="count">Conflict <b>{Math.min(index, blocks.length - 1) + 1}</b> of {blocks.length}</span>
          <button class="btn small square" type="button" aria-label="Next conflict" title="Next conflict (F8)" disabled={blocks.length < 2} onclick={() => step(1)}><Icon name="down" size={14} /></button>
        </div>
        <button class="btn small take first" type="button" onclick={() => take(first)}>{keepName(first)}</button>
        <button class="btn small take second" type="button" onclick={() => take(second)}>{keepName(second)}</button>
        <button class="btn small" type="button" onclick={() => take('both')} title="{names[first]}, then {names[second].toLowerCase()}">Keep both</button>
        <button class="btn small" type="button" onclick={() => take('both-reversed')} title="{names[second]}, then {names[first].toLowerCase()}">Keep both, reversed</button>
        <button class="btn small" type="button" onclick={() => take('neither')} title="Keep no line of this conflict">Keep neither</button>
        <button class="btn ghost small" type="button" onclick={startOver} title="Put this conflict back as it was">Start over</button>
      {/if}
      {#if data.base !== null}
        <label class="base-toggle"><input type="checkbox" bind:checked={showBase} /> Show the common ancestor</label>
      {/if}
    </div>

    <!-- Side 1, the result, side 2: three columns, aligned row by row, that scroll as one. -->
    <div class="columns">
      {@render column(first, 1)}
      <div class="side result">
        <div class="bar">
          <b>Result</b>
          <small title="Keep a side of each conflict with its arrow; it stays editable.">Keep a side of each conflict with its arrow; it stays editable.</small>
        </div>
        <ConflictResult
          bind:this={result}
          value={data.merged ?? ''}
          regions={blocks.map((b, i) => ({ id: i, start: b.start, end: b.end }))}
          merged={changes?.result ?? []}
          {first}
          {info}
          lineSeparator={separator}
          label="Result of the resolution"
          onchange={(next) => (text = next)}
          onedit={onEdit}
          ontype={() => (lastTypeAt = performance.now())}
          onundo={undoPick}
        />
      </div>
      {@render column(second, 2)}
    </div>

    {#if showBase && data.base !== null}
      <div class="side base">
        <div class="bar"><b>Common ancestor</b><small>Before both changes.</small></div>
        <CodeEditor bind:this={baseView} value={data.base} readonly marks={baseMarks} lineSeparator={separator} label="Common ancestor, read-only" />
      </div>
    {/if}
  {/if}
</section>

<!-- A side, in its column: its number is its place, 1 on the left, and takes its color. -->
{#snippet column(side: Side, n: 1 | 2)}
  {@const commit = (side === 'ours' ? current?.oursLabel : current?.theirsLabel) ?? ''}
  <div class="side" class:first={n === 1} class:second={n === 2}>
    <div class="bar">
      <span class="side-chip" aria-hidden="true">{n}</span>
      <b>{names[side]}</b>
      <small class="mono" title={commit}>{commit}</small>
      <button class="btn small" type="button" disabled={!open} onclick={() => keepAll(side)} title="Keep this side in every conflict left" aria-label="Keep all of {names[side]}">Keep all</button>
    </div>
    {#if side === 'ours'}
      <ConflictSide
        bind:this={oursView}
        value={data?.ours ?? ''}
        lineSeparator={separator}
        label={names.ours}
        place={n === 1 ? 'left' : 'right'}
        changes={changes?.ours ?? []}
        conflicts={oursConflicts}
        current={index}
        onkeep={(block) => keep('ours', block)}
        onaside={(block) => aside('ours', block)}
      />
    {:else}
      <ConflictSide
        bind:this={theirsView}
        value={data?.theirs ?? ''}
        lineSeparator={separator}
        label={names.theirs}
        place={n === 1 ? 'left' : 'right'}
        changes={changes?.theirs ?? []}
        conflicts={theirsConflicts}
        current={index}
        onkeep={(block) => keep('theirs', block)}
        onaside={(block) => aside('theirs', block)}
      />
    {/if}
  </div>
{/snippet}

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

  /* Side 1, the result, side 2: the result is the widest, it is what is saved. Below 240 px a
     column no longer shrinks, and the card scrolls sideways (guidelines, 9.26). */
  .columns {
    flex: 1 1 0;
    display: grid;
    grid-template-columns: minmax(240px, 1fr) minmax(240px, 1.25fr) minmax(240px, 1fr);
    min-height: 0;
    overflow-x: auto;
  }
  /* The common ancestor, under the three columns, across their width. */
  .base {
    flex: 0 0 30%;
    border-top: 1px solid var(--line-soft);
  }
  .side {
    display: grid;
    /* One column of the width given: the content does not widen it. */
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    --c: var(--muted);
  }
  .side + .side {
    border-left: 1px solid var(--line-soft);
  }
  /* Side 1, on the left, and side 2 take the first two colors of the graph (guidelines, 2.2 and
     9.26), whichever version of git each is. */
  .first {
    --c: var(--l0);
  }
  .second {
    --c: var(--l1);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    /* As high in the three columns, the result's included, which has no button: their rows start
       at the same height (guidelines, 9.26). A small button, 24 px, and the padding and the line. */
    min-height: 33px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--line-soft);
    background: color-mix(in srgb, var(--c) 10%, var(--panel));
    min-width: 0;
  }
  .result .bar,
  .base .bar {
    background: var(--panel);
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
  /* The number of a side: the letter of a file state, on the tint of its side (guidelines, 9.6). */
  .side-chip {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: var(--radius-xs);
    background: color-mix(in srgb, var(--c) 30%, var(--panel));
    font-family: var(--f-mono);
    font-size: var(--fs-micro);
    font-weight: var(--fw-strong);
  }

  .choose {
    flex: none;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line-soft);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: 6px;
  }
  .btn.square {
    min-width: 24px;
    padding: 2px 4px;
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
  .take.first {
    --c: var(--l0);
  }
  .take.second {
    --c: var(--l1);
  }
  .base-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    color: var(--muted);
  }
</style>
