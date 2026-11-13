<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import CommitPanel from './CommitPanel.svelte';
  import ConflictEditor from './ConflictEditor.svelte';
  import DiffView from './DiffView.svelte';
  import Graph from './Graph.svelte';
  import { SIDES, TERMINAL, fit, fitTerminal, layout, saveLayout } from './layout.svelte';
  import OperationBanner from './OperationBanner.svelte';
  import RebaseDialog from './RebaseDialog.svelte';
  import type { Repo } from './repo.svelte';
  import Sidebar from './Sidebar.svelte';
  import StartPage from './StartPage.svelte';
  import StatusBar from './StatusBar.svelte';
  import TerminalPanel from './TerminalPanel.svelte';
  import Toolbar from './Toolbar.svelte';
  import WipStrip from './WipStrip.svelte';
  import WorkPanel from './WorkPanel.svelte';
  import { workspace } from './workspace.svelte';
  import { motion } from '../ui/motion';
  import Splitter from '../ui/Splitter.svelte';

  /** `active`: this tab is the one being looked at. */
  let { repo, active = true }: { repo: Repo; active?: boolean } = $props();

  const current = $derived(repo.current);
  // A new tab, or a folder that could not be opened: the start page replaces the graph.
  const empty = $derived(!repo.result && !repo.loading);
  const pick = () => void workspace.pick();

  // Widths of the side cards: those chosen, brought down to what the window allows.
  let bodyWidth = $state(0);
  const sides = $derived(fit(bodyWidth));
  // A hidden card has no column left, nor its handle: the center goes to the edge of the view.
  const columns = $derived(
    [layout.sidebarHidden ? '' : 'var(--sidebar) 8px', 'minmax(0, 1fr)', layout.detailHidden ? '' : '8px var(--detail)'].join(' ').trim(),
  );

  // Terminal of the repository, under the graph: its height, the one chosen brought down to the room there is.
  const terminals = $derived(repo.terminals);
  let middleHeight = $state(0);
  const terminalSize = $derived(fitTerminal(middleHeight));
  // The graph hidden: the terminal holds the center alone. Never without a terminal to show.
  const graphHidden = $derived(terminals.alone && terminals.shown && !!repo.result);

  // If the focus was in the card being hidden, it moves to the graph: it must not be lost.
  let sidebarEl = $state<HTMLElement>();
  let detailEl = $state<HTMLElement>();
  let centerEl = $state<HTMLElement>();
  let terminalEl = $state<HTMLElement>();
  $effect.pre(() => {
    const leaving = [
      layout.sidebarHidden ? sidebarEl : undefined,
      layout.detailHidden ? detailEl : undefined,
      terminals.shown ? undefined : terminalEl,
      graphHidden ? centerEl : undefined,
    ];
    untrack(() => {
      const focused = document.activeElement;
      if (!active || !leaving.some((card) => card?.contains(focused))) return;
      // Without the graph, the terminal stays in the center: it takes the keyboard.
      void tick().then(() => {
        if (graphHidden) terminals.focusAsk++;
        else centerEl?.querySelector<HTMLElement>('[role="listbox"]')?.focus();
      });
    });
  });

  // The detail follows the selection.
  $effect(() => {
    void repo.currentKey;
    untrack(() => repo.loadDetail());
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && repo.openFile) {
      e.preventDefault();
      repo.closeDiff();
    }
  }

  onMount(() => {
    const stopAutoFetch = repo.startAutoFetch();
    // Back in the window: the files may have changed in the editor or the terminal.
    const onFocus = () => {
      if (active && repo.result && !repo.locked) {
        void repo.refreshStatus();
        void repo.refreshRefs();
      }
    };
    window.addEventListener('focus', onFocus);
    return () => {
      stopAutoFetch();
      window.removeEventListener('focus', onFocus);
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="view" onkeydown={onKey}>
  {#if empty}
    <StartPage {repo} />
  {:else}
    <Toolbar {repo} onpick={pick} />
    <div class="progress" class:on={repo.locked} aria-hidden="true"><span></span></div>
    <OperationBanner {repo} />

    <div
      class="body"
      class:bare={!repo.result}
      bind:clientWidth={bodyWidth}
      style:--sidebar="{sides.sidebar}px"
      style:--detail="{sides.detail}px"
      style:--columns={columns}
    >
      {#if repo.result}
        <!-- A hidden card stays mounted: its filter, its position and the message being written are kept. -->
        <div class="side" bind:this={sidebarEl} hidden={layout.sidebarHidden} inert={layout.sidebarHidden}>
          <Sidebar {repo} />
        </div>
        {#if !layout.sidebarHidden}
          <Splitter
            label="Width of the branch list"
            value={sides.sidebar}
            min={SIDES.sidebar.min}
            max={sides.maxSidebar}
            initial={SIDES.sidebar.initial}
            onchange={(width) => (layout.sidebar = width)}
            oncommit={saveLayout}
          />
        {/if}
      {/if}

      <!-- The graph, and under it the terminal of the repository, as wide as it (guidelines, section 9.20). -->
      <div class="middle" bind:clientHeight={middleHeight} style:--terminal="{terminalSize.value}px">
        <!-- Hidden, the graph stays mounted: its position and its selection are kept. -->
        <div class="center" bind:this={centerEl} hidden={graphHidden} inert={graphHidden}>
          <Graph {repo} />
          {#if repo.openFile}
            <DiffView
              title={repo.openFile.title}
              subtitle={repo.openFile.subtitle}
              diff={repo.diff}
              loading={repo.diffLoading}
              error={repo.diffError}
              selectable={repo.openFile.selectable}
              busy={repo.locked}
              onapply={(lines) => void repo.applyLines(lines)}
              onclose={() => repo.closeDiff()}
            />
          {/if}
        </div>
        {#if repo.result && terminals.sessions.length}
          {#if terminals.shown && !graphHidden}
            <Splitter
              orientation="horizontal"
              label="Height of the terminal"
              value={terminalSize.value}
              min={TERMINAL.min}
              max={terminalSize.max}
              initial={terminalSize.initial}
              direction={-1}
              onchange={(height) => (layout.terminal = height)}
              oncommit={saveLayout}
            />
          {/if}
          <!-- Hidden, it stays mounted: its shells keep running. -->
          <div class="terminal" class:alone={graphHidden} bind:this={terminalEl} hidden={!terminals.shown} inert={!terminals.shown}>
            <TerminalPanel {repo} active={active && terminals.shown} />
          </div>
        {/if}
      </div>

      {#if repo.result}
        {#if !layout.detailHidden}
          <Splitter
            label="Width of the detail panel"
            value={sides.detail}
            min={SIDES.detail.min}
            max={sides.maxDetail}
            initial={SIDES.detail.initial}
            direction={-1}
            onchange={(width) => (layout.detail = width)}
            oncommit={saveLayout}
          />
        {/if}
        <aside class="detail" class:working={current?.wip} bind:this={detailEl} hidden={layout.detailHidden} inert={layout.detailHidden}>
          {#key repo.currentKey}
            <div class="pane" in:fade={{ duration: motion.med }}>
              {#if current?.wip}
                {#if repo.status}
                  <WorkPanel {repo} />
                {:else}
                  <p class="muted">Reading the state of the repository…</p>
                {/if}
              {:else if current}
                <CommitPanel {repo} commit={current} />
              {:else}
                {#if repo.wipCount > 0}<WipStrip {repo} />{/if}
                <span class="label">Detail</span>
                <p class="muted">Click a commit, or use the arrow keys in the list. A right-click opens its actions.</p>
              {/if}
            </div>
          {/key}
        </aside>
      {/if}

      {#if repo.conflictFile}
        <ConflictEditor {repo} file={repo.conflictFile} />
      {/if}
    </div>

    <StatusBar {repo} />
  {/if}
  <RebaseDialog {repo} />
</div>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  /* Activity line under the toolbar, visible while git works. */
  .progress {
    flex: none;
    height: 2px;
    position: relative;
    overflow: hidden;
    background: transparent;
  }
  .progress span {
    position: absolute;
    inset: 0 auto 0 0;
    width: 35%;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    opacity: 0;
    transition: opacity var(--t-med) var(--ease);
  }
  .progress.on span {
    opacity: 1;
    animation: sweep var(--t-wait) var(--ease) infinite;
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  /* Sidebar, center and detail: three cards, separated by the window background. */
  .body {
    position: relative;
    flex: 1;
    display: grid;
    /* Between the cards, two 8 px handles: they stand for the gap. */
    grid-template-columns: var(--columns);
    padding: 0 8px 8px;
    min-height: 0;
  }
  .body.bare {
    grid-template-columns: minmax(0, 1fr);
  }
  .side {
    display: grid;
    min-width: 0;
    min-height: 0;
  }
  /* A hidden card takes no room; `hidden` alone would yield to `display` above. */
  .side[hidden],
  .detail[hidden],
  .center[hidden],
  .terminal[hidden] {
    display: none;
  }
  .center,
  .detail {
    min-width: 0;
    min-height: 0;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  .center {
    position: relative;
    flex: 1;
    overflow: hidden;
  }
  .middle {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .middle > :global(.splitter) {
    flex: none;
    height: 8px;
  }
  .terminal {
    flex: none;
    height: var(--terminal);
    min-height: 0;
  }
  /* Without the graph, the terminal holds the whole column. */
  .terminal.alone {
    flex: 1;
    height: auto;
  }
  .detail {
    overflow: auto;
  }
  .detail.working {
    overflow: hidden;
  }
  .pane {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    min-height: 100%;
  }
  .working .pane {
    height: 100%;
    align-items: stretch;
  }
  .pane p {
    margin: 0;
  }
  .muted {
    color: var(--muted);
  }
</style>
