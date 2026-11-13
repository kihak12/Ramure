<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import { fly } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import { prefs, savePrefs, type DiffMode } from './prefs.svelte';
  import type { Diff } from './types';
  import { motion } from '../ui/motion';

  let {
    title,
    subtitle = '',
    diff,
    loading = false,
    error = '',
    selectable = null,
    busy = false,
    onapply,
    onclose,
  }: {
    title: string;
    subtitle?: string;
    diff: Diff | null;
    loading?: boolean;
    error?: string;
    /** Partial staging: the changed lines can be selected to be staged or unstaged. */
    selectable?: 'stage' | 'unstage' | null;
    busy?: boolean;
    /** Receives the indices of the picked lines, in `diff.text.split('\n')`. */
    onapply?: (lines: number[]) => void;
    onclose: () => void;
  } = $props();

  const MAX_LINES = 5000;

  interface Line {
    /** Position in the text of the diff: it is what the Rust core expects. */
    index: number;
    type: 'add' | 'del' | 'ctx' | 'hunk' | 'meta';
    old: number | null;
    new: number | null;
    text: string;
    /** Number of the hunk the line belongs to. */
    hunk: number;
  }

  function parse(text: string): { lines: Line[]; cut: boolean; added: number; removed: number } {
    const lines: Line[] = [];
    let oldNo = 0;
    let newNo = 0;
    let inHunk = false;
    let hunk = -1;
    let added = 0;
    let removed = 0;
    const raw = text.split('\n');
    if (raw.at(-1) === '') raw.pop();
    for (const [index, line] of raw.entries()) {
      if (lines.length >= MAX_LINES) return { lines, cut: true, added, removed };
      const header = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
      if (header) {
        oldNo = Number(header[1]);
        newNo = Number(header[2]);
        inHunk = true;
        hunk++;
        lines.push({ index, type: 'hunk', old: null, new: null, text: line, hunk });
      } else if (!inHunk || line.startsWith('diff --git ')) {
        // File header: paths, modes, index.
        inHunk = false;
        lines.push({ index, type: 'meta', old: null, new: null, text: line, hunk });
      } else if (line.startsWith('+')) {
        added++;
        lines.push({ index, type: 'add', old: null, new: newNo++, text: line.slice(1), hunk });
      } else if (line.startsWith('-')) {
        removed++;
        lines.push({ index, type: 'del', old: oldNo++, new: null, text: line.slice(1), hunk });
      } else if (line.startsWith('\\')) {
        // “\ No newline at end of file”
        lines.push({ index, type: 'meta', old: null, new: null, text: line, hunk });
      } else {
        lines.push({ index, type: 'ctx', old: oldNo++, new: newNo++, text: line.slice(1), hunk });
      }
    }
    return { lines, cut: false, added, removed };
  }

  const parsed = $derived(diff ? parse(diff.text) : null);
  // A cut diff cannot give a reliable patch.
  const canSelect = $derived(!!selectable && !!parsed && !parsed.cut && !diff?.truncated);
  const changes = $derived(parsed?.lines.filter((l) => l.type === 'add' || l.type === 'del') ?? []);
  const verb = $derived(selectable === 'unstage' ? 'Unstage' : 'Stage');
  const SIGN = { add: '+', del: '−', ctx: '', hunk: '', meta: '' } as const;

  const selected = new SvelteSet<number>();
  let anchor = -1;

  // A new diff resets the selection and the cursor.
  $effect(() => {
    void diff;
    selected.clear();
    anchor = -1;
    cursor = -1;
  });

  function toggle(line: Line, event: MouseEvent) {
    if (!canSelect || (line.type !== 'add' && line.type !== 'del')) return;
    if (event.shiftKey && anchor >= 0) {
      // Shift + click: the whole range from the last line clicked.
      const [from, to] = [Math.min(anchor, line.index), Math.max(anchor, line.index)];
      for (const change of changes) if (change.index >= from && change.index <= to) selected.add(change.index);
    } else if (selected.has(line.index)) {
      selected.delete(line.index);
    } else {
      selected.add(line.index);
    }
    anchor = line.index;
    cursor = line.index;
  }

  // --- Keyboard -------------------------------------------------------------------------------

  /** Changed line that carries the keyboard cursor, by its position in the text of the diff. */
  let cursor = $state(-1);
  let linesEl = $state<HTMLDivElement>();

  /** Moves the cursor from one changed line to another; `extend` picks the lines it crosses. */
  function move(to: number, extend: boolean) {
    if (!changes.length) return;
    const from = changes.findIndex((l) => l.index === cursor);
    const next = Math.max(0, Math.min(changes.length - 1, to));
    if (extend) {
      if (from >= 0) selected.add(changes[from].index);
      selected.add(changes[next].index);
    }
    cursor = changes[next].index;
    anchor = cursor;
    // Once the line is marked, bring it into view.
    requestAnimationFrame(() => linesEl?.querySelector('.cursor')?.scrollIntoView({ block: 'nearest' }));
  }

  // The area of the lines keeps the focus and handles the keys itself, like the other lists.
  function onKey(event: KeyboardEvent) {
    if (!canSelect || event.target !== linesEl) return;
    const at = changes.findIndex((l) => l.index === cursor);
    const line = at >= 0 ? changes[at] : null;
    switch (event.key) {
      case 'ArrowDown':
        move(at < 0 ? 0 : at + 1, event.shiftKey);
        break;
      case 'ArrowUp':
        move(at < 0 ? changes.length - 1 : at - 1, event.shiftKey);
        break;
      case 'Home':
        move(0, event.shiftKey);
        break;
      case 'End':
        move(changes.length - 1, event.shiftKey);
        break;
      case ' ':
        if (!line) return;
        if (selected.has(line.index)) selected.delete(line.index);
        else selected.add(line.index);
        break;
      case 'Enter':
        if (busy) return;
        if (selected.size) applySelection();
        else if (line) applyHunk(line.hunk);
        else return;
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  // On taking the focus without a cursor, it goes to the first changed line.
  function onFocus() {
    if (canSelect && cursor < 0 && changes.length) cursor = changes[0].index;
  }

  // --- Side-by-side layout -----------------------------------------------------------------

  /** A row of the two-column table: old on the left, new on the right. */
  interface Pair {
    key: number;
    /** File or hunk header, across the whole width. */
    full: Line | null;
    left: Line | null;
    right: Line | null;
    /** Part really changed on each side, when the two lines look alike. */
    inline: { pre: string; old: string; new: string; post: string } | null;
  }

  /** What changes between two lines: their common start and end frame the difference. */
  function inlineChange(before: string, after: string): Pair['inline'] {
    let pre = 0;
    const max = Math.min(before.length, after.length);
    while (pre < max && before[pre] === after[pre]) pre++;
    let post = 0;
    while (post < max - pre && before[before.length - 1 - post] === after[after.length - 1 - post]) post++;
    // Too little in common: highlighting nearly the whole line would tell nothing.
    if (pre + post < Math.min(before.length, after.length) * 0.3) return null;
    return {
      pre: before.slice(0, pre),
      old: before.slice(pre, before.length - post),
      new: after.slice(pre, after.length - post),
      post: before.slice(before.length - post),
    };
  }

  /** Lines up the removals and additions of one change on the same rows of the table. */
  function pairUp(lines: Line[]): Pair[] {
    const out: Pair[] = [];
    let removed: Line[] = [];
    let added: Line[] = [];
    const flush = () => {
      const count = Math.max(removed.length, added.length);
      for (let k = 0; k < count; k++) {
        const left = removed[k] ?? null;
        const right = added[k] ?? null;
        out.push({
          key: (left ?? right)!.index,
          full: null,
          left,
          right,
          inline: left && right ? inlineChange(left.text, right.text) : null,
        });
      }
      removed = [];
      added = [];
    };
    for (const line of lines) {
      if (line.type === 'del') {
        // A removal after additions starts a new change.
        if (added.length) flush();
        removed.push(line);
      } else if (line.type === 'add') {
        added.push(line);
      } else {
        flush();
        if (line.type === 'ctx') out.push({ key: line.index, full: null, left: line, right: line, inline: null });
        else out.push({ key: line.index, full: line, left: null, right: null, inline: null });
      }
    }
    flush();
    return out;
  }

  const split = $derived(prefs.diffMode === 'split');
  const allPairs = $derived(parsed ? pairUp(parsed.lines) : []);
  const pairs = $derived(split ? allPairs : []);

  // A modified line is a removal and an addition that go together. Picking only one half leaves
  // the old version next to the new one, or removes the line without replacing it. Only the pairs
  // whose two lines look alike count: the others are two separate changes that chance placed side
  // by side.
  const halves = $derived.by(() => {
    const missing: number[] = [];
    if (!canSelect || !selected.size) return missing;
    for (const pair of allPairs) {
      if (!pair.inline || !pair.left || !pair.right) continue;
      const old = selected.has(pair.left.index);
      if (old !== selected.has(pair.right.index)) missing.push(old ? pair.right.index : pair.left.index);
    }
    return missing;
  });
  let root = $state<HTMLElement>();
  const pickHalves = () => {
    for (const index of halves) selected.add(index);
    // The button goes away with the warning: the focus goes back to the lines.
    root?.querySelector<HTMLElement>('.lines')?.focus();
  };
  const setMode = (mode: DiffMode) => savePrefs({ diffMode: mode });

  const applySelection = () => onapply?.([...selected]);
  const applyHunk = (hunk: number) => onapply?.(changes.filter((l) => l.hunk === hunk).map((l) => l.index));
</script>

<section class="diff" bind:this={root} aria-label="Diff of {title}" in:fly={{ y: 14, duration: motion.med }} out:fly={{ y: 10, duration: motion.fast }}>
  <header>
    <div class="names">
      <b class="mono">{title}</b>
      {#if subtitle}<small>{subtitle}</small>{/if}
    </div>
    {#if canSelect && selected.size}
      <button class="btn small" type="button" onclick={() => selected.clear()}>Clear selection</button>
      <button class="btn primary small" type="button" disabled={busy} onclick={applySelection} title="Enter, in the list of lines">
        {verb} {selected.size} line{selected.size === 1 ? '' : 's'}
      </button>
    {:else if canSelect}
      <small class="hint" title="From the keyboard: ↑ and ↓ to move from one line to the next, Home and End to go to the ends, Space to pick a line or let go of it, Shift to extend, Enter to confirm">Click lines to {selectable === 'unstage' ? 'unstage' : 'stage'} only some of them</small>
    {/if}
    {#if parsed && (parsed.added || parsed.removed)}
      <span class="count mono"><span class="plus">+{parsed.added}</span> <span class="minus">−{parsed.removed}</span></span>
    {/if}
    <div class="modes" role="group" aria-label="Diff layout">
      <button type="button" aria-pressed={!split} onclick={() => setMode('unified')} title="One column">Unified</button>
      <button type="button" aria-pressed={split} onclick={() => setMode('split')} title="Old on the left, new on the right">Side by side</button>
    </div>
    <button class="btn" type="button" onclick={onclose} title="Close the diff (Escape)"><Icon name="x" size={14} /> Close</button>
  </header>

  {#if error}
    <p class="note err">{error}</p>
  {:else if loading && !parsed}
    <p class="note">Loading the diff…</p>
  {:else if parsed && parsed.lines.length === 0}
    <p class="note">No content changes to show: the file is empty, or only its mode changed.</p>
  {:else if parsed && split}
    <!-- The area keeps the focus and handles the keys, like the lists of the application. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_static_element_interactions -->
    <div class="lines mono" class:selectable={canSelect} class:has-cursor={cursor >= 0} tabindex="0" bind:this={linesEl} onkeydown={onKey} onfocus={onFocus}>
      <table class="split">
        <colgroup><col class="c-no" /><col class="c-sign" /><col /><col class="c-no" /><col class="c-sign" /><col /></colgroup>
        <tbody>
          {#each pairs as pair (pair.key)}
            {#if pair.full}
              <tr class="t-{pair.full.type}">
                <td class="code" colspan="6">
                  {pair.full.text}{#if canSelect && pair.full.type === 'hunk'}
                    <button class="btn small hunk-btn" type="button" disabled={busy} onclick={() => applyHunk(pair.full!.hunk)}>{verb} hunk</button>
                  {/if}
                </td>
              </tr>
            {:else}
              {@const left = pair.left}
              {@const right = pair.right}
              {@const leftPick = canSelect && left?.type === 'del'}
              {@const rightPick = canSelect && right?.type === 'add'}
              <tr>
                <td class="no">{left?.old ?? ''}</td>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <td
                  class="half"
                  colspan="2"
                  class:c-del={left?.type === 'del'}
                  class:c-void={!left}
                  class:pickable={leftPick}
                  class:cursor={leftPick && left?.index === cursor}
                  class:picked={left ? selected.has(left.index) : false}
                  onclick={(e) => left && toggle(left, e)}
                >
                  <span class="sign">{#if left?.type === 'del'}−{/if}</span>{#if left}{#if pair.inline && left.type === 'del'}{pair.inline.pre}<mark>{pair.inline.old}</mark>{pair.inline.post}{:else}{left.text}{/if}{/if}
                </td>
                <td class="no">{right?.new ?? ''}</td>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <td
                  class="half"
                  colspan="2"
                  class:c-add={right?.type === 'add'}
                  class:c-void={!right}
                  class:pickable={rightPick}
                  class:cursor={rightPick && right?.index === cursor}
                  class:picked={right ? selected.has(right.index) : false}
                  onclick={(e) => right && toggle(right, e)}
                >
                  <span class="sign">{#if right?.type === 'add'}+{/if}</span>{#if right}{#if pair.inline && right.type === 'add'}{pair.inline.pre}<mark>{pair.inline.new}</mark>{pair.inline.post}{:else}{right.text}{/if}{/if}
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
      {#if parsed.cut || diff?.truncated}
        <p class="note">This diff is too long: only its beginning is shown.</p>
      {/if}
    </div>
  {:else if parsed}
    <!-- The area keeps the focus and handles the keys, like the lists of the application. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_static_element_interactions -->
    <div class="lines mono" class:selectable={canSelect} class:has-cursor={cursor >= 0} tabindex="0" bind:this={linesEl} onkeydown={onKey} onfocus={onFocus}>
      <table>
        <tbody>
          {#each parsed.lines as line (line.index)}
            {@const pickable = canSelect && (line.type === 'add' || line.type === 'del')}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <tr
              class="t-{line.type}"
              class:pickable
              class:cursor={pickable && line.index === cursor}
              class:picked={selected.has(line.index)}
              aria-selected={pickable ? selected.has(line.index) : undefined}
              onclick={(e) => toggle(line, e)}
            >
              <td class="no">{line.old ?? ''}</td>
              <td class="no">{line.new ?? ''}</td>
              <td class="sign">{SIGN[line.type]}</td>
              <td class="code">
                {line.text}{#if canSelect && line.type === 'hunk'}
                  <button class="btn small hunk-btn" type="button" disabled={busy} onclick={() => applyHunk(line.hunk)}>{verb} hunk</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if parsed.cut || diff?.truncated}
        <p class="note">This diff is too long: only its beginning is shown.</p>
      {/if}
    </div>
  {/if}

  <!-- Under the lines, so as not to move them under the pointer while they are being picked. -->
  {#if halves.length}
    <p class="lonely" role="status">
      <Icon name="warning" size={14} />
      <span>
        You picked only one half of {halves.length === 1 ? 'a modified line' : `${halves.length} modified lines`}: the old version and
        the new one will not be {selectable === 'unstage' ? 'unstaged' : 'staged'} together.
      </span>
      <button class="btn small" type="button" onclick={pickHalves}>Pick the other half</button>
    </p>
  {/if}
</section>

<style>
  .diff > .lonely {
    grid-row: 3;
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 12px;
    padding: 6px 8px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    font-size: var(--fs-small);
  }
  .lonely :global(svg) {
    flex: none;
    color: var(--warn);
  }
  .lonely span {
    flex: 1;
    min-width: 0;
  }
  .diff {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: grid;
    /* The header, the lines, which take the room, then the warning when there is one. */
    grid-template-rows: auto minmax(0, 1fr) auto;
    background: var(--panel);
  }
  .diff > :global(*) {
    grid-row: 2;
  }
  .diff > header {
    grid-row: 1;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px 6px 14px;
    border-bottom: 1px solid var(--line-soft);
    min-width: 0;
  }
  .names {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.3;
  }
  .names b,
  .names small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .names b {
    font-size: var(--fs-body);
  }
  .names small,
  .hint {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .hint {
    white-space: nowrap;
  }
  .count {
    font-size: var(--fs-label);
    white-space: nowrap;
  }
  /* Toggle between the two layouts */
  .modes {
    display: flex;
    padding: 2px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
  }
  .modes button {
    padding: 2px 9px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    font-size: var(--fs-label);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .modes button:hover {
    color: var(--ink);
  }
  .modes button[aria-pressed='true'] {
    background: var(--raised);
    color: var(--ink);
    font-weight: var(--fw-strong);
    box-shadow: var(--shadow-soft);
  }
  .plus {
    color: var(--ok);
  }
  .minus {
    color: var(--bad);
  }
  .note {
    margin: 0;
    padding: 18px;
    color: var(--muted);
  }
  .note.err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .lines {
    overflow: auto;
    font-size: var(--fs-code);
    line-height: 18px;
    min-height: 0;
  }
  .lines:focus-visible {
    outline-offset: -2px;
  }
  /* The keyboard cursor shows on its line, as an inner ring: no frame around the list
     (guidelines, section 9.6). Without a cursor, the frame stays, so that the focus always shows. */
  .lines.selectable.has-cursor:focus-visible {
    outline: none;
  }
  .lines:focus-visible .cursor {
    outline: 1.5px solid var(--accent);
    outline-offset: -1.5px;
  }
  table {
    border-collapse: collapse;
    min-width: 100%;
  }
  td {
    padding: 0;
    white-space: pre;
    vertical-align: top;
  }
  .no {
    width: 1%;
    min-width: 44px;
    padding: 0 8px;
    text-align: right;
    color: var(--muted);
    user-select: none;
    opacity: 0.75;
  }
  .sign {
    width: 16px;
    text-align: center;
    user-select: none;
  }
  .code {
    padding-right: 16px;
  }
  .t-add {
    background: color-mix(in srgb, var(--ok) 14%, transparent);
  }
  .t-add .sign {
    color: var(--ok);
  }
  .t-del {
    background: color-mix(in srgb, var(--bad) 13%, transparent);
  }
  .t-del .sign {
    color: var(--bad);
  }
  .t-hunk {
    background: var(--sel);
    color: var(--muted);
  }
  .t-meta {
    color: var(--muted);
  }
  .t-hunk .code,
  .t-meta .code {
    padding-left: 4px;
  }
  /* Side by side: two halves of equal width, long lines wrap. */
  .split {
    width: 100%;
    table-layout: fixed;
  }
  .split .c-no {
    width: 46px;
  }
  .split .c-sign {
    width: 18px;
  }
  .split .half {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    padding-right: 10px;
    border-right: 1px solid var(--line-soft);
  }
  .split .half .sign {
    display: inline-block;
    width: 18px;
    text-align: center;
  }
  .split .no {
    width: auto;
    min-width: 0;
    padding: 0 6px;
  }
  .c-del {
    background: color-mix(in srgb, var(--bad) 13%, transparent);
  }
  .c-del .sign {
    color: var(--bad);
  }
  .c-add {
    background: color-mix(in srgb, var(--ok) 14%, transparent);
  }
  .c-add .sign {
    color: var(--ok);
  }
  /* Side with no matching line. */
  .c-void {
    background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--muted) 9%, transparent) 6px 12px);
  }
  mark {
    border-radius: var(--radius-xs);
    color: inherit;
  }
  .c-del mark {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
  }
  .c-add mark {
    background: color-mix(in srgb, var(--ok) 36%, transparent);
  }
  .c-del.pickable:hover {
    background: color-mix(in srgb, var(--bad) 22%, transparent);
  }
  .c-add.pickable:hover {
    background: color-mix(in srgb, var(--ok) 24%, transparent);
  }
  .c-del.picked,
  .c-del.picked:hover {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
  }
  .c-add.picked,
  .c-add.picked:hover {
    background: color-mix(in srgb, var(--ok) 38%, transparent);
  }

  /* Picking lines: a picked line shows by its stronger tint and its bold sign, with no colored bar
     on the edge. */
  .pickable {
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .pickable.t-add:hover {
    background: color-mix(in srgb, var(--ok) 24%, transparent);
  }
  .pickable.t-del:hover {
    background: color-mix(in srgb, var(--bad) 22%, transparent);
  }
  .picked.t-add,
  .picked.t-add:hover {
    background: color-mix(in srgb, var(--ok) 38%, transparent);
  }
  .picked.t-del,
  .picked.t-del:hover {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
  }
  .picked .sign {
    font-weight: var(--fw-strong);
  }
  .hunk-btn {
    margin-left: 12px;
    padding: 0 8px;
    line-height: 16px;
    font-family: var(--f-ui);
    font-size: var(--fs-small);
    vertical-align: top;
  }
</style>
