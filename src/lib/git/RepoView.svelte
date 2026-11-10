<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import CommitPanel from './CommitPanel.svelte';
  import DiffView from './DiffView.svelte';
  import Graph from './Graph.svelte';
  import OperationBanner from './OperationBanner.svelte';
  import RebaseDialog from './RebaseDialog.svelte';
  import { Repo } from './repo.svelte';
  import Sidebar from './Sidebar.svelte';
  import StatusBar from './StatusBar.svelte';
  import Toolbar from './Toolbar.svelte';
  import WorkPanel from './WorkPanel.svelte';

  const repo = new Repo();
  const current = $derived(repo.current);

  async function pick() {
    const dir = await open({ directory: true, multiple: false, title: 'Ouvrir un dépôt Git' });
    if (typeof dir === 'string') await repo.open(dir);
  }

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
    repo.restore();
    const stopAutoFetch = repo.startAutoFetch();
    // Retour dans la fenêtre : les fichiers ont pu changer dans l'éditeur ou le terminal.
    const onFocus = () => {
      if (repo.result && !repo.locked) {
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
  <Toolbar {repo} onpick={pick} />
  <div class="progress" class:on={repo.locked} aria-hidden="true"><span></span></div>
  <OperationBanner {repo} />

  <div class="body" class:bare={!repo.result}>
    {#if repo.result}<Sidebar {repo} />{/if}

    <div class="center">
      <Graph {repo} onpick={pick} />
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
      <aside class="detail" class:working={current?.wip}>
        {#key repo.currentKey}
          <div class="pane" in:fade={{ duration: 170 }}>
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
                  Voir les {repo.wipCount} fichier{repo.wipCount > 1 ? 's' : ''} modifié{repo.wipCount > 1 ? 's' : ''}
                </button>
              {/if}
            {/if}
          </div>
        {/key}
      </aside>
    {/if}
  </div>

  <StatusBar {repo} />
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
    animation: sweep 1.1s var(--ease) infinite;
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  .body {
    flex: 1;
    display: grid;
    grid-template-columns: 204px minmax(0, 1fr) 324px;
    min-height: 0;
  }
  .body.bare {
    grid-template-columns: minmax(0, 1fr);
  }
  .center {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  .detail {
    border-left: 1px solid var(--line);
    background: var(--panel);
    min-height: 0;
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
    padding: 14px;
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
