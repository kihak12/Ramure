<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu } from '../ui/menu.svelte';
  import { commitMenu, dropMenu, labelMenu, switchBranch, switchToRemote } from './actions';
  import { accepts, drag, isBranch } from './drag.svelte';
  import { bare, labelsOf, type Label } from './labels';
  import { revealDetail } from './layout.svelte';
  import { theme } from '../settings/theme.svelte';
  import { prefs } from './prefs.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo }: { repo: Repo } = $props();
  // Hidden tabs stay mounted: the ids of the rows belong to each one.
  const uid = $props.id();

  // Two renderings of the graph, chosen in the settings (guidelines, section 10).
  // “Ribbon”: author dots on the graph, soft curves, the message in a capsule of the color of the
  // branch. “Classic”: points and curves, more compact.
  const STYLES = {
    ribbon: { row: 34, lane: 24, pad: 20, node: 10, merge: 5, line: 3, max: 420 },
    classic: { row: 28, lane: 16, pad: 14, node: 5, merge: 4, line: 2, max: 340 },
  } as const;

  const LABELS = { max: 220, min: 164 }; // “Branch / tag” column
  const MESSAGE = { wanted: 200, min: 120 }; // message column
  const GRAPH_MIN = 72; // the graph is never narrower than its header
  const ROW_END = 10; // right margin of a row
  const BUFFER = 24; // rows rendered above and below the visible area
  const STEP = 12; // the render window only moves in steps of 12 rows
  const BAND = 24; // height of the band that links a commit to the capsule of its message
  const BAND_ALPHA = 0.15; // same opacity as the CSS capsule that extends it in the message column

  let scrollTop = $state(0);
  let viewH = $state(600);
  // The canvas reads its colors from the tokens: it is drawn again when the theme changes.
  const dark = $derived(theme.key);
  let fontsReady = $state(false); // the canvas must be drawn again once the embedded font is loaded
  // True just after an opening: the rows come in one after the other.
  let fresh = $state(false);

  let scroller = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();

  const ribbon = $derived(prefs.graphStyle !== 'classic');
  const metrics = $derived(STYLES[ribbon ? 'ribbon' : 'classic']);
  const ROW = $derived(metrics.row);

  const commits = $derived(repo.commits);
  // Width that all the lanes would ask for, never less than the column header.
  const lanesW = $derived(
    Math.max(GRAPH_MIN, Math.min(metrics.max, metrics.pad * 2 + Math.max(0, (repo.result?.maxLanes ?? 1) - 1) * metrics.lane + 6)),
  );

  // --- Columns (guidelines, section 10.1) ---------------------------------------------------
  // The message comes first. When it would get less than 200 px, the other columns give way, in
  // order: the SHA, the name of the author, the date, then “Branch / tag” down to 164 px, then the
  // graph, whose lanes on the right are cut, until the message has 120 px. The computation starts
  // from the room left after the graph: a repository with many lanes shows fewer columns.
  let cardW = $state(0);
  const columns = $derived.by(() => {
    // Columns to the right of the message, from the fullest to the most reduced. `who`: the name
    // of the author is written; the “Classic” rendering keeps the room of its initials.
    const initials = ribbon ? 0 : 34;
    const levels = [
      { author: 150, who: true, date: 92, sha: 66 },
      { author: 150, who: true, date: 92, sha: 0 },
      { author: initials, who: false, date: 92, sha: 0 },
      { author: initials, who: false, date: 0, sha: 0 },
    ];
    const rest = (level: (typeof levels)[number]) => level.author + level.date + level.sha;
    if (cardW <= 0) return { ...levels[0], labels: LABELS.max, graph: lanesW };
    const room = cardW - ROW_END;
    const level = levels.find((l) => room - rest(l) - LABELS.max - lanesW >= MESSAGE.wanted) ?? levels[levels.length - 1];
    const left = room - rest(level);
    const labels = Math.max(LABELS.min, Math.min(LABELS.max, left - lanesW - MESSAGE.wanted));
    let graph = Math.max(GRAPH_MIN, Math.min(lanesW, left - labels - MESSAGE.min));
    // Cut graph: it stops just after the last whole lane, so as not to show a lane without its
    // points. The room gained goes to the message.
    if (graph < lanesW) {
      const { pad, node, lane } = metrics;
      const whole = Math.max(1, Math.floor((graph - pad - node - 4) / lane) + 1);
      graph = Math.max(GRAPH_MIN, pad + (whole - 1) * lane + node + 4);
    }
    return { ...level, labels, graph };
  });
  const graphW = $derived(columns.graph);
  // Lanes that fit in the column, the point and the ring of the current commit included. Beyond,
  // they are cut: their commits have no point, and their band starts at the edge of the column.
  const shownLanes = $derived(Math.max(1, Math.floor((graphW - metrics.pad - metrics.node - 4) / metrics.lane) + 1));
  // Render window: only these rows exist in the DOM and on the canvas.
  const winStart = $derived(Math.max(0, Math.floor((scrollTop / ROW - BUFFER) / STEP) * STEP));
  const winEnd = $derived(Math.min(commits.length, winStart + Math.ceil(viewH / ROW) + 2 * BUFFER + STEP));
  const rows = $derived(commits.slice(winStart, winEnd));
  // Row of the current commit, for the animated halo laid on its node.
  const headIndex = $derived(repo.head ? commits.indexOf(repo.head) : -1);

  // Breakdown of the working changes, for the WIP row.
  const wip = $derived.by(() => {
    const status = repo.status;
    if (!status) return null;
    const kinds = new Map<string, string>();
    for (const file of [...status.unstaged, ...status.staged]) kinds.set(file.path, file.kind);
    const count = (...wanted: string[]) => [...kinds.values()].filter((k) => wanted.includes(k)).length;
    return {
      modified: count('modified', 'renamed', 'copied', 'typechange'),
      added: count('added', 'untracked'),
      deleted: count('deleted'),
      conflicts: count('conflict'),
    };
  });

  const dateFmt = new Intl.DateTimeFormat('en-US', { day: 'numeric', month: 'short', year: 'numeric' });

  const initials = (author: string) =>
    author
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]!.toUpperCase())
      .join('');
  /** Color of an author in the classic rendering: always the same, taken from the palette of the lanes. */
  const hue = (author: string) => [...author].reduce((sum, ch) => sum + ch.charCodeAt(0), 0) % 8;

  // --- Drawing the graph ------------------------------------------------------------------

  function draw() {
    if (!canvas) return;
    const css = getComputedStyle(document.documentElement);
    const palette = Array.from({ length: 8 }, (_, i) => css.getPropertyValue(`--l${i}`).trim());
    const background = css.getPropertyValue('--panel').trim(); // the graph sits on a card
    const onColor = css.getPropertyValue('--on-accent').trim();
    const family = getComputedStyle(document.body).fontFamily;
    const initialSize = parseFloat(css.getPropertyValue('--fs-micro'));
    const initialWeight = css.getPropertyValue('--fw-strong').trim();
    const color = (lane: number) => palette[lane % palette.length];
    const { row, lane: laneW, pad, node, merge, line } = metrics;

    const dpr = window.devicePixelRatio || 1;
    const width = graphW;
    const height = Math.max(1, (winEnd - winStart) * row);
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';

    const x = (lane: number) => pad + lane * laneW;
    const y = (index: number) => (index - winStart) * row + row / 2;

    // Ribbon: a band of the color of the branch leaves the commit and runs to its message.
    if (ribbon) {
      ctx.globalAlpha = BAND_ALPHA;
      for (let i = winStart; i < winEnd; i++) {
        const commit = commits[i];
        const cx = commit.lane < shownLanes ? x(commit.lane) : width - pad;
        ctx.fillStyle = color(commit.lane);
        ctx.fillRect(cx, y(i) - BAND / 2, width - cx, BAND);
      }
      ctx.globalAlpha = 1;
    }

    // Line that links a label, in the left column, to the commit it points to.
    ctx.lineWidth = ribbon ? 2 : 1.5;
    ctx.globalAlpha = ribbon ? 0.6 : 0.45;
    for (let i = winStart; i < winEnd; i++) {
      const commit = commits[i];
      if (!commit.refs.length) continue;
      ctx.strokeStyle = color(commit.lane);
      ctx.beginPath();
      ctx.moveTo(0, y(i));
      ctx.lineTo(x(commit.lane) - node - 3, y(i));
      ctx.stroke();
    }
    ctx.globalAlpha = 1;

    // Segments: from row i to row i + 1. Start one row higher to draw what enters the window
    // from the top too.
    ctx.lineWidth = line;
    for (let i = Math.max(0, winStart - 1); i < winEnd; i++) {
      const commit = commits[i];
      const y1 = y(i);
      const y2 = y1 + row;
      for (const [from, to, lane] of commit.edges) {
        const [x1, x2] = [x(from), x(to)];
        ctx.strokeStyle = color(lane);
        ctx.setLineDash(commit.wip && lane === commit.lane ? [3, 3] : []);
        ctx.beginPath();
        ctx.moveTo(x1, y1);
        if (from === to) {
          ctx.lineTo(x2, y2);
        } else {
          // Change of column: a soft curve, with no corner.
          ctx.bezierCurveTo(x1, y1 + row * 0.6, x2, y2 - row * 0.6, x2, y2);
        }
        ctx.stroke();
      }
    }
    ctx.setLineDash([]);

    // Nodes, over the segments.
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (let i = winStart; i < winEnd; i++) {
      const commit = commits[i];
      if (commit.lane >= shownLanes) continue;
      const cx = x(commit.lane);
      const cy = y(i);
      const tint = color(commit.lane);
      const isHead = commit.refs.some((r) => r.kind === 'head' || r.kind === 'detached');
      const isMerge = commit.parents.length > 1;
      const radius = isMerge ? merge : node;

      if (isHead) {
        ctx.beginPath();
        ctx.arc(cx, cy, radius + 3.5, 0, Math.PI * 2);
        ctx.strokeStyle = tint;
        ctx.globalAlpha = 0.55;
        ctx.lineWidth = 1.5;
        ctx.stroke();
        ctx.globalAlpha = 1;
      }

      ctx.beginPath();
      ctx.arc(cx, cy, commit.wip ? node - 1.5 : radius, 0, Math.PI * 2);
      ctx.lineWidth = 2;
      if (commit.wip) {
        ctx.fillStyle = background;
        ctx.strokeStyle = tint;
        ctx.setLineDash([2.5, 2]);
      } else {
        ctx.fillStyle = tint;
        ctx.strokeStyle = background;
      }
      ctx.fill();
      ctx.stroke();
      ctx.setLineDash([]);

      // Initials of the author in the dot.
      if (ribbon && !commit.wip && !isMerge) {
        const letters = initials(commit.author);
        ctx.fillStyle = onColor;
        ctx.font = `${initialWeight} ${initialSize}px ${family}`;
        // Two initials too wide for the dot: keep only the first, without shrinking the text.
        const room = 2 * Math.sqrt(Math.max(0, node * node - (initialSize * 0.36) ** 2)) - 3;
        ctx.fillText(ctx.measureText(letters).width > room ? letters.slice(0, 1) : letters, cx, cy + 0.5);
      }
    }
  }

  $effect(() => {
    // Draws again when the window, the data, the width, the style or the theme change.
    void dark;
    void fontsReady;
    draw();
  });

  // Staggered entrance after an opening or a full reload.
  $effect(() => {
    if (repo.loadCount === 0) return;
    fresh = true;
    const timer = window.setTimeout(() => (fresh = false), 700);
    return () => window.clearTimeout(timer);
  });

  // --- Scrolling and smoothness measurement ----------------------------------------------

  let raf = 0;
  let lastFrame = 0;
  let lastScroll = 0;
  let samples: number[] = [];

  function measure(now: number) {
    samples.push(now - lastFrame);
    lastFrame = now;
    if (now - lastScroll < 250) {
      raf = requestAnimationFrame(measure);
      return;
    }
    raf = 0;
    // The last measurement covers the pause at the end of the scroll: drop it.
    samples.pop();
    if (samples.length >= 10) {
      const sorted = [...samples].sort((a, b) => a - b);
      repo.frames = {
        avg: samples.reduce((a, b) => a + b, 0) / samples.length,
        worst: sorted[Math.floor(sorted.length * 0.99)],
      };
    }
    samples = [];
  }

  function onScroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    lastScroll = performance.now();
    if (!raf) {
      lastFrame = lastScroll;
      raf = requestAnimationFrame(measure);
    }
  }

  function reveal(index: number) {
    if (!scroller) return;
    const top = index * ROW;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW > scroller.scrollTop + viewH) scroller.scrollTop = top + ROW - viewH;
  }

  /** Click on a row: it is selected, and the focus stays on the list, which handles the keyboard.
   *  A row that kept the focus would lose it on leaving the render window. */
  function select(index: number) {
    repo.selected = index;
    scroller?.focus({ preventScroll: true });
    // The row of the working changes: one comes to it to stage or commit, so the detail comes back.
    if (commits[index]?.wip) revealDetail();
  }

  /** Keyboard focus: the list always has a selected and visible commit, which carries the marker. */
  function onFocus() {
    if (!commits.length || !scroller?.matches(':focus-visible')) return;
    if (repo.selected < 0) repo.selected = 0;
    reveal(repo.selected);
  }

  function onKey(e: KeyboardEvent) {
    if (!commits.length) return;
    // Enter asks for the detail of the selected commit: it comes back if it was hidden. The arrows
    // only move the selection (guidelines, section 9.17).
    if (e.key === 'Enter' && repo.selected >= 0) {
      e.preventDefault();
      revealDetail();
      return;
    }
    const page = Math.max(1, Math.floor(viewH / ROW) - 1);
    const moves: Record<string, number> = {
      ArrowDown: repo.selected + 1,
      ArrowUp: repo.selected - 1,
      PageDown: repo.selected + page,
      PageUp: repo.selected - page,
      Home: 0,
      End: commits.length - 1,
    };
    if (!(e.key in moves)) return;
    e.preventDefault();
    repo.selected = Math.max(0, Math.min(commits.length - 1, moves[e.key]));
    reveal(repo.selected);
  }

  // --- Labels -------------------------------------------------------------------------------

  /** Double-click on a label: switch to the branch. */
  function switchTo(label: Label) {
    if (repo.locked) return;
    if (label.local) {
      void switchBranch(repo, label.local);
    } else if (label.remotes[0]) {
      // A remote branch alone on its commit: switch to the local branch and update it.
      void switchToRemote(repo, label.remotes[0]);
    }
  }

  // Label hovered during a drag, to highlight it.
  let dropOn = $state('');
  const labelKey = (label: Label, commit: CommitRow) => `${commit.sha}:${label.kind}:${label.name}`;

  function onDrop(event: DragEvent, target: Label) {
    const source = drag.label;
    dropOn = '';
    drag.label = null;
    if (!source || !isBranch(target)) return;
    event.preventDefault();
    menu.open(event, dropMenu(repo, source, target));
  }

  function openCommitMenu(event: MouseEvent, commit: CommitRow, index: number) {
    repo.selected = index;
    menu.open(event, commitMenu(repo, commit));
  }

  onMount(() => {
    void document.fonts.ready.then(() => (fontsReady = true));

    const observer = new ResizeObserver(() => {
      if (scroller && scroller.clientHeight > 0) viewH = scroller.clientHeight;
    });
    if (scroller) observer.observe(scroller);

    repo.reveal = reveal;
    repo.resetScroll = () => {
      scrollTop = 0;
      if (scroller) scroller.scrollTop = 0;
    };

    return () => {
      observer.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  });
</script>

{#snippet chip(label: Label, commit: CommitRow)}
  {@const waiting = repo.switching !== null && (label.local === repo.switching || label.remotes.includes(repo.switching))}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="ref k-{label.kind}"
    class:local={label.local !== null}
    class:drop={dropOn === labelKey(label, commit)}
    style:--c="var(--l{commit.lane % 8})"
    title="{label.title} — double-click to switch, right-click for actions, drag onto another branch to merge or rebase"
    draggable={isBranch(label) ? 'true' : undefined}
    ondragstart={(e) => {
      drag.label = label;
      e.dataTransfer?.setData('text/plain', label.name);
      if (e.dataTransfer) e.dataTransfer.effectAllowed = 'link';
    }}
    ondragend={() => {
      drag.label = null;
      dropOn = '';
    }}
    ondragover={(e) => {
      if (!accepts(label)) return;
      e.preventDefault();
      dropOn = labelKey(label, commit);
    }}
    ondragleave={() => (dropOn = '')}
    ondrop={(e) => onDrop(e, label)}
    ondblclick={(e) => {
      e.stopPropagation();
      switchTo(label);
    }}
    oncontextmenu={(e) => menu.open(e, labelMenu(repo, label, commit.sha))}
  >
    {#if label.kind === 'head'}<Icon name="check" size={14} />{/if}
    <span class="ref-name">{label.name}</span>
    <!-- During the switch, the ring takes the place of an icon: the label keeps its size. -->
    {#if label.local}{#if waiting}<span class="wait" aria-hidden="true"><span class="spinner"></span></span>{:else}<Icon name="laptop" size={14} />{/if}{/if}
    {#if label.remotes.length}{#if waiting && !label.local}<span class="wait" aria-hidden="true"><span class="spinner"></span></span>{:else}<Icon name="cloud" size={14} />{/if}{/if}
    {#if label.tag}<Icon name="tag" size={14} />{/if}
  </span>
{/snippet}

<div
  class="graph"
  class:ribbon
  class:no-who={!columns.who}
  class:no-date={!columns.date}
  class:no-sha={!columns.sha}
  style:--labels="{columns.labels}px"
  style:--graph="{graphW}px"
  style:--author="{columns.author}px"
  style:--date="{columns.date}px"
  style:--sha="{columns.sha}px"
  style:--row="{ROW}px"
  style:--band="{BAND}px"
>
  <div class="head">
    <span>Branch / tag</span><span>Graph</span><span>Message</span><span>Author</span><span>Date</span><span>SHA</span>
  </div>

  {#if repo.result && commits.length === 0}
    <div class="empty">
      <span class="art"><Icon name="commit" size={28} /></span>
      <p>This repository has no commits yet.</p>
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="scroller"
    bind:this={scroller}
    class:asked={!!scroller && menu.current?.origin === scroller}
    bind:clientWidth={cardW}
    onscroll={onScroll}
    onkeydown={onKey}
    onfocus={onFocus}
    tabindex="0"
    role="listbox"
    aria-label="Commit history"
    aria-activedescendant={repo.current ? `${uid}-commit-${repo.selected}` : undefined}
  >
    <div class="spacer" class:fresh style:height="{commits.length * ROW}px">
      <canvas
        bind:this={canvas}
        class="lanes"
        style:top="{winStart * ROW}px"
        style:width="{graphW}px"
        style:height="{(winEnd - winStart) * ROW}px"
        aria-hidden="true"
      ></canvas>
      {#if ribbon && headIndex >= winStart && headIndex < winEnd && repo.head && repo.head.lane < shownLanes - 1}
        <!-- Animated halo on the current commit: a CSS element, so as not to draw the canvas again at every frame. -->
        <span
          class="pulse"
          aria-hidden="true"
          style:top="{headIndex * ROW + ROW / 2}px"
          style:--x="{metrics.pad + repo.head.lane * metrics.lane}px"
          style:--c="var(--l{repo.head.lane % 8})"
        ></span>
      {/if}
      {#each rows as commit, i (commit.sha)}
        {@const index = winStart + i}
        {@const labels = commit.wip ? [] : labelsOf(commit, repo.shortRemotes)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          class:sel={index === repo.selected}
          id="{uid}-commit-{index}"
          role="option"
          tabindex="-1"
          aria-selected={index === repo.selected}
          style:top="{index * ROW}px"
          style:--i={Math.min(i, 30)}
          style:--c="var(--l{commit.lane % 8})"
          onclick={() => select(index)}
          ondblclick={revealDetail}
          oncontextmenu={(e) => openCommitMenu(e, commit, index)}
        >
          <span class="labels">
            {#if labels.length}
              {@render chip(labels[0], commit)}
              {#if labels.length > 1}
                <span class="more">+{labels.length - 1}</span>
                <span class="all">
                  {#each labels as label (label.kind + label.name)}{@render chip(label, commit)}{/each}
                </span>
              {/if}
              <span class="wire"></span>
            {/if}
          </span>
          <span>
            {#if ribbon && !commit.wip && commit.lane < shownLanes}
              <!-- The point is drawn on the canvas, which does not receive the pointer: this
                   transparent dot, laid over it, carries the name of the author, that of a merge
                   included (guidelines, 10.2). -->
              <span
                class="dot"
                style:--x="{metrics.pad + commit.lane * metrics.lane}px"
                title={commit.author}
                aria-hidden="true"
              ></span>
            {/if}
          </span>
          <span class="msg">
            {#if commit.wip}
              <span class="wip-tag mono">// WIP</span>
              {#if wip}
                {#if wip.conflicts}<span class="stat bad" title="Files in conflict"><Icon name="warning" size={14} />{wip.conflicts}</span>{/if}
                {#if wip.modified}<span class="stat warn" title="Modified files"><Icon name="pencil" size={14} />{wip.modified}</span>{/if}
                {#if wip.added}<span class="stat ok" title="New files"><Icon name="plus" size={14} />{wip.added}</span>{/if}
                {#if wip.deleted}<span class="stat bad" title="Deleted files"><Icon name="minus" size={14} />{wip.deleted}</span>{/if}
              {:else}
                <span class="subject muted">{repo.wipCount} changed file{repo.wipCount === 1 ? '' : 's'}</span>
              {/if}
            {:else}
              <!-- Cut lane: the point and the initials are missing, the author is in the tooltip. -->
              <span class="subject" title={commit.lane < shownLanes ? commit.subject : `${commit.subject} — ${commit.author}`}>{commit.subject}</span>
            {/if}
          </span>
          <span class="author">
            {#if !commit.wip}
              {#if !ribbon}
                <span class="avatar" style:--c="var(--l{hue(commit.author)})" title={commit.author} aria-hidden="true">{initials(commit.author)}</span>
              {/if}
              <!-- A column that gave way: the name is no longer seen, but the row still says it (guidelines, 11). -->
              <span class="who" class:spoken={!columns.who} title={commit.author}>{commit.author}</span>
            {/if}
          </span>
          <span class="date"><span class:spoken={!columns.date}>{commit.wip ? '' : dateFmt.format(commit.time * 1000)}</span></span>
          <span class="sha mono">{commit.wip ? '' : commit.sha.slice(0, 7)}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .graph {
    display: grid;
    grid-template-rows: 28px minmax(0, 1fr);
    height: 100%;
    min-width: 0;
    min-height: 0;
    position: relative;
  }
  /* Widths computed in the script (guidelines, section 10.1). A column that gives way keeps its
     place in the grid, at 0 px, and its content is no longer seen: the name of the author and the
     date are still read by a screen reader (see below). */
  .head,
  .row {
    display: grid;
    grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) var(--author) var(--date) var(--sha);
    align-items: center;
  }
  .head span:nth-child(n + 4) {
    overflow: hidden;
    white-space: nowrap;
  }
  /* `visibility`, not `display`: a removed cell would shift the next ones by one column. */
  .no-who .head span:nth-child(4),
  .no-date .head span:nth-child(5),
  .no-sha .head span:nth-child(6),
  .no-sha .sha {
    visibility: hidden;
  }
  .head {
    border-bottom: 1px solid var(--line-soft);
    font-size: var(--fs-label);
    color: var(--muted);
    font-weight: var(--fw-strong);
    overflow: hidden;
    scrollbar-gutter: stable;
    padding-right: 10px;
    white-space: nowrap;
  }
  .head span {
    padding-left: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .head span:nth-child(2) {
    padding-left: 8px;
  }
  .scroller {
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    min-height: 0;
  }
  /* From the keyboard, the focus shows on the selected commit, not as a frame around the list. */
  .scroller:focus-visible,
  .scroller:global(.focus-ring):focus,
  .row:focus-visible {
    outline: none;
  }
  /* `.focus-ring`: the focus given back to the list by the script, when a menu asked for from the
     keyboard closes (lib/ui/focus.ts). It shows like that of Tab, on the selected row. */
  .scroller:focus-visible .row.sel::before,
  .scroller:global(.focus-ring):focus .row.sel::before,
  .scroller.asked .row.sel::before,
  .scroller:has(.row:focus-visible) .row.sel::before {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  /* The rows, the canvas and the halo stack among themselves, apart from the rest of the card. */
  .spacer {
    position: relative;
    isolation: isolate;
  }
  .lanes {
    position: absolute;
    left: var(--labels);
    pointer-events: none;
    z-index: 2;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--row);
    padding-right: 10px;
    cursor: pointer;
    white-space: nowrap;
    /* A double-click switches branches: it must not select text on the way. */
    user-select: none;
    /* The hover and selection background goes under the content of the row. */
    isolation: isolate;
  }
  /* Hover and selection: a rounded background, inset from the edges of the card. No bar on the edge. */
  .row::before {
    content: '';
    position: absolute;
    inset: 1px 6px;
    z-index: -1;
    border-radius: var(--radius);
    background: var(--hover);
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  .row:hover::before {
    opacity: 1;
  }
  .row.sel::before {
    opacity: 1;
    background: var(--sel);
  }
  .fresh .row {
    animation: row-in var(--t-slow) var(--ease) both;
    animation-delay: calc(var(--i) * var(--t-step));
  }
  .fresh .lanes {
    animation: fade-in var(--t-slow) var(--ease) both;
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateX(-10px);
    }
  }

  /* Laid on the point of a commit, in “Ribbon”: it shows nothing, it carries the tooltip. */
  .dot {
    position: absolute;
    top: 50%;
    left: calc(var(--labels) + var(--x));
    width: 20px;
    height: 20px;
    margin: -10px 0 0 -10px;
    border-radius: 50%;
  }

  /* Halo of the current commit */
  .pulse {
    position: absolute;
    left: calc(var(--labels) + var(--x));
    z-index: 2;
    width: 20px;
    height: 20px;
    margin: -10px 0 0 -10px;
    border-radius: 50%;
    border: 2px solid var(--c);
    pointer-events: none;
    animation: pulse var(--t-halo) var(--ease) infinite;
  }
  @keyframes pulse {
    from {
      transform: scale(1);
      opacity: 0.7;
    }
    70%,
    to {
      transform: scale(2.3);
      opacity: 0;
    }
  }

  /* Column of the branches and tags */
  .labels {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    height: 100%;
    padding-left: 10px;
  }
  .wire {
    flex: 1;
    min-width: 6px;
    height: 1.5px;
    background: var(--c);
    opacity: 0.45;
  }
  .ribbon .wire {
    height: 2px;
    opacity: 0.6;
  }
  /* The 12 px ring is centered in the 14 px box of the icon it replaces. */
  .ref .wait {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
  }
  .ref .spinner {
    --spinner: currentColor;
  }
  .ref {
    flex: 0 1 auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-small);
    line-height: 20px;
    padding: 0 8px;
    border-radius: var(--radius-s);
    border: 1px solid var(--c, var(--line));
    background: color-mix(in srgb, var(--c, var(--line)) 16%, var(--panel));
    font-weight: var(--fw-regular);
    /* A label never changes size: not on click, not on hover, not on drop. */
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  .ref:hover {
    background: color-mix(in srgb, var(--c, var(--line)) 28%, var(--panel));
  }
  .ref[draggable='true'] {
    cursor: grab;
  }
  /* Branch onto which the dragged one is about to be dropped. */
  .ref.drop,
  .ribbon .ref.drop {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .ref-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* `k-` prefix: the current branch has the type `head`, which must not inherit from the `.head` header. */
  .ref.k-head,
  .ref.k-detached {
    font-weight: var(--fw-strong);
    background: color-mix(in srgb, var(--c) 28%, var(--panel));
  }
  .ref.k-remote {
    border-style: dashed;
    font-weight: var(--fw-regular);
  }
  .ref.k-tag {
    font-weight: var(--fw-regular);
  }
  .ref.local {
    font-weight: var(--fw-strong);
  }
  /* “Ribbon” rendering: every label has the same background, tinted with the color of its row.
     The icon, the border and the weight say whether it is a local branch, a remote one or a tag.
     The current branch is solid. */
  .ribbon .ref:not(.k-detached) {
    background: color-mix(in srgb, var(--c, var(--line)) 24%, var(--panel));
  }
  /* No more than 28%: beyond, the text falls under 7:1 in some dark themes (guidelines, section 2.4). */
  .ribbon .ref:not(.k-detached):hover {
    background: color-mix(in srgb, var(--c, var(--line)) 28%, var(--panel));
  }
  .ribbon .ref.local {
    border-color: color-mix(in srgb, var(--c) 55%, transparent);
    font-weight: var(--fw-strong);
  }
  .ribbon .ref.k-head {
    background: var(--c);
    border-color: var(--c);
    color: var(--on-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 28%, transparent);
  }
  /* On hover, the background moves toward the color of the body text: darker in light, lighter in
     dark. The text of the current branch gains contrast there in both themes. */
  .ribbon .ref.k-head:hover {
    background: color-mix(in srgb, var(--c) 90%, var(--ink));
    border-color: color-mix(in srgb, var(--c) 90%, var(--ink));
  }
  .more {
    flex: none;
    font-size: var(--fs-micro);
    padding: 0 5px;
    line-height: 16px;
    border-radius: var(--radius-pill);
    background: var(--side);
    border: 1px solid var(--line);
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  /* All the labels of a commit, unfolded on hover of the column. */
  .all {
    position: absolute;
    left: 6px;
    top: 2px;
    z-index: 4;
    display: none;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 3px 8px 6px 3px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    /* It stays within its column, 6 px from each side: the canvas, laid over the rows, would
       otherwise cover the part that overflows onto the graph. A long name is cut there, whole in
       the tooltip of its label (guidelines, 9.4). */
    max-width: calc(100% - 12px);
  }
  .all .ref {
    max-width: 100%;
  }
  .labels:hover .all {
    display: flex;
    animation: pop-in var(--t-fast) var(--ease);
  }
  /* The row whose labels are hovered goes in front of the next ones: otherwise the unfolded list
     would stay under them, visible but out of reach of the pointer, and would close as soon as it
     moves down onto it. It stays under the canvas, which keeps its lines over its background. */
  .row:has(.labels:hover) {
    z-index: 1;
  }

  .msg {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    padding-left: 10px;
  }
  /* The band drawn on the canvas goes on here as a capsule: as wide as the text, rounded on the right. */
  .ribbon .msg {
    width: fit-content;
    max-width: 100%;
    height: var(--band);
    padding-right: 14px;
    border-radius: 0 var(--radius-pill) var(--radius-pill) 0;
    background: color-mix(in srgb, var(--c) 15%, transparent);
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .wip-tag {
    font-size: var(--fs-small);
    line-height: 18px;
    padding: 0 7px;
    border-radius: var(--radius-s);
    border: 1px dashed var(--muted);
    color: var(--muted);
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    color: var(--c);
  }
  /* Two counters are further apart than the icon is from its number. */
  .stat + .stat {
    margin-left: 4px;
  }
  .stat.ok {
    --c: var(--ok);
  }
  .stat.warn {
    --c: var(--warn);
  }
  .stat.bad {
    --c: var(--bad);
  }
  .author {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding-left: 10px;
    color: var(--muted);
    font-size: var(--fs-label);
    overflow: hidden;
  }
  .who {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    font-size: var(--fs-micro);
    font-weight: var(--fw-strong);
    letter-spacing: -0.02em;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 20%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
  }
  .date {
    padding-left: 10px;
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sha {
    padding-left: 10px;
    overflow: hidden;
    color: var(--muted);
    font-size: var(--fs-code);
  }
  .muted {
    color: var(--muted);
  }

  .empty {
    position: absolute;
    inset: 28px 0 0 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 14px;
    text-align: center;
    padding: 24px;
    z-index: 2;
    background: var(--panel);
    animation: fade-in var(--t-slow) var(--ease);
  }
  .empty p {
    margin: 0;
    max-width: 52ch;
    color: var(--muted);
    font-size: var(--fs-body);
  }
  .art {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: var(--radius-card);
    color: var(--accent);
    background: var(--accent-soft);
  }
</style>
