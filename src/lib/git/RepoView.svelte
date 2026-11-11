<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
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
  import WorkPanel from './WorkPanel.svelte';
  import { workspace } from './workspace.svelte';
  import { motion } from '../ui/motion';
  import Splitter from '../ui/Splitter.svelte';

  /** `active` : cet onglet est celui qu'on regarde. */
  let { repo, active = true }: { repo: Repo; active?: boolean } = $props();

  const current = $derived(repo.current);
  // Onglet neuf, ou dossier qui n'a pas pu être ouvert : la page d'accueil remplace le graphe.
  const empty = $derived(!repo.result && !repo.loading);
  const pick = () => void workspace.pick();

  // Largeurs des cartes latérales : celles qu'on a choisies, ramenées à ce que la fenêtre permet.
  let bodyWidth = $state(0);
  const sides = $derived(fit(bodyWidth));
  // Une carte masquée n'a plus de colonne, ni sa poignée : le centre va jusqu'au bord de la vue.
  const columns = $derived(
    [layout.sidebarHidden ? '' : 'var(--sidebar) 8px', 'minmax(0, 1fr)', layout.detailHidden ? '' : '8px var(--detail)'].join(' ').trim(),
  );

  // Terminal du dépôt, sous le graphe : sa hauteur, celle qu'on a choisie ramenée à la place qu'il y a.
  const terminals = $derived(repo.terminals);
  let middleHeight = $state(0);
  const terminalSize = $derived(fitTerminal(middleHeight));
  // Le graphe masqué : le terminal tient le centre à lui seul. Jamais sans terminal à montrer.
  const graphHidden = $derived(terminals.alone && terminals.shown && !!repo.result);

  // Si le focus était dans la carte qu'on masque, il passe au graphe : il ne doit pas se perdre.
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
      // Sans graphe, c'est le terminal qui reste au centre : il prend le clavier.
      void tick().then(() => {
        if (graphHidden) terminals.focusAsk++;
        else centerEl?.querySelector<HTMLElement>('[role="listbox"]')?.focus();
      });
    });
  });

  // Le détail suit la sélection.
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
    // Retour dans la fenêtre : les fichiers ont pu changer dans l'éditeur ou le terminal.
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
        <!-- Une carte masquée reste montée : son filtre, sa position et le message en cours sont gardés. -->
        <div class="side" bind:this={sidebarEl} hidden={layout.sidebarHidden} inert={layout.sidebarHidden}>
          <Sidebar {repo} />
        </div>
        {#if !layout.sidebarHidden}
          <Splitter
            label="Largeur de la liste des branches"
            value={sides.sidebar}
            min={SIDES.sidebar.min}
            max={sides.maxSidebar}
            initial={SIDES.sidebar.initial}
            onchange={(width) => (layout.sidebar = width)}
            oncommit={saveLayout}
          />
        {/if}
      {/if}

      <!-- Le graphe, et sous lui le terminal du dépôt, aussi large que lui (charte, section 9.20). -->
      <div class="middle" bind:clientHeight={middleHeight} style:--terminal="{terminalSize.value}px">
        <!-- Masqué, le graphe reste monté : sa position et sa sélection sont gardées. -->
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
              label="Hauteur du terminal"
              value={terminalSize.value}
              min={TERMINAL.min}
              max={terminalSize.max}
              initial={terminalSize.initial}
              direction={-1}
              onchange={(height) => (layout.terminal = height)}
              oncommit={saveLayout}
            />
          {/if}
          <!-- Masqué, il reste monté : ses shells continuent de tourner. -->
          <div class="terminal" class:alone={graphHidden} bind:this={terminalEl} hidden={!terminals.shown} inert={!terminals.shown}>
            <TerminalPanel {repo} active={active && terminals.shown} />
          </div>
        {/if}
      </div>

      {#if repo.result}
        {#if !layout.detailHidden}
          <Splitter
            label="Largeur du panneau de détail"
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
                  <p class="muted">Lecture de l’état du dépôt…</p>
                {/if}
              {:else if current}
                <CommitPanel {repo} commit={current} />
              {:else}
                <span class="label">Détail</span>
                <p class="muted">Clique un commit, ou utilise les flèches du clavier dans la liste. Un clic droit ouvre ses actions.</p>
                {#if repo.wipCount > 0}
                  <button class="btn" type="button" onclick={() => repo.showWip()}>
                    <Icon name="pencil" size={14} />
                    {repo.wipText}
                  </button>
                {/if}
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
  /* Filet d'activité sous la barre d'outils, visible tant que git travaille. */
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
  /* Barre latérale, centre et détail : trois cartes, séparées par le fond de la fenêtre. */
  .body {
    position: relative;
    flex: 1;
    display: grid;
    /* Entre les cartes, deux poignées de 8 px : elles tiennent lieu d'écart. */
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
  /* Une carte masquée ne prend plus de place ; `hidden` seul céderait à `display` ci-dessus. */
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
  /* Sans le graphe, le terminal tient toute la colonne. */
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
