<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import CommitPanel from './CommitPanel.svelte';
  import ConflictEditor from './ConflictEditor.svelte';
  import DiffView from './DiffView.svelte';
  import Graph from './Graph.svelte';
  import { SIDES, fit, layout, saveLayout } from './layout.svelte';
  import OperationBanner from './OperationBanner.svelte';
  import RebaseDialog from './RebaseDialog.svelte';
  import type { Repo } from './repo.svelte';
  import Sidebar from './Sidebar.svelte';
  import StartPage from './StartPage.svelte';
  import StatusBar from './StatusBar.svelte';
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
  const GUTTERS = 32; // marges de la vue et les deux poignées
  let bodyWidth = $state(0);
  const sides = $derived(fit(bodyWidth, GUTTERS));

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

    <div class="body" class:bare={!repo.result} bind:clientWidth={bodyWidth} style:--sidebar="{sides.sidebar}px" style:--detail="{sides.detail}px">
      {#if repo.result}
        <Sidebar {repo} />
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

      <div class="center">
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

      {#if repo.result}
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
        <aside class="detail" class:working={current?.wip}>
          {#key repo.currentKey}
            <div class="pane" in:fade={{ duration: motion.med }}>
              {#if current?.wip}
                {#if repo.status}
                  <WorkPanel {repo} />
                {:else}
                  <p class="muted">Lecture de l'état du dépôt…</p>
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
    grid-template-columns: var(--sidebar) 8px minmax(0, 1fr) 8px var(--detail);
    padding: 0 8px 8px;
    min-height: 0;
  }
  .body.bare {
    grid-template-columns: minmax(0, 1fr);
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
    overflow: hidden;
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
