<script lang="ts">
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import CloneDialog from './CloneDialog.svelte';
  import RepoView from './RepoView.svelte';
  import { workspace } from './workspace.svelte';
  import { motion } from '../ui/motion';

  /** Vrai quand le module Git est à l'écran. */
  let { active = true }: { active?: boolean } = $props();
</script>

<div class="workspace">
  <div class="tabs" role="tablist" aria-label="Dépôts ouverts">
    {#each workspace.tabs as tab (tab.id)}
      {@const current = tab.id === workspace.activeId}
      <div class="tab" class:current animate:flip={{ duration: motion.med }} in:scale={{ duration: motion.med, start: 0.9 }}>
        <button
          class="open"
          type="button"
          role="tab"
          aria-selected={current}
          title={tab.repo.path || 'Nouvel onglet'}
          onclick={() => workspace.activate(tab.id)}
          onauxclick={(e) => {
            // Clic du milieu : ferme l'onglet.
            if (e.button === 1) workspace.closeTab(tab.id);
          }}
        >
          {#if tab.repo.locked}
            <span class="spinner" aria-label="Opération en cours"></span>
          {:else if tab.repo.status?.operation}
            <span class="alert" title="Une opération attend d’être terminée"><Icon name="warning" size={14} /></span>
          {:else}
            <Icon name={tab.repo.path ? 'branch' : 'plus'} size={14} />
          {/if}
          <span class="name">{workspace.title(tab)}</span>
        </button>
        <button class="close" type="button" aria-label="Fermer l’onglet {workspace.title(tab)}" title="Fermer l’onglet" onclick={() => workspace.closeTab(tab.id)}>
          <Icon name="x" size={14} />
        </button>
      </div>
    {/each}
    <button class="add" type="button" aria-label="Nouvel onglet" title="Nouvel onglet" onclick={() => workspace.newTab()}>
      <Icon name="plus" size={16} />
    </button>
  </div>

  <div class="views">
    {#each workspace.tabs as tab (tab.id)}
      {@const current = tab.id === workspace.activeId}
      <section class="view" class:hidden={!current} inert={!current} role="tabpanel">
        <RepoView repo={tab.repo} active={active && current} />
      </section>
    {/each}
  </div>
</div>

<CloneDialog />

<style>
  .workspace {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
    background: var(--side);
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    /* 4 px sous les onglets, rendus par une marge négative : la place de l'anneau de focus, que le
       défilement horizontal rognerait sinon. */
    padding: 8px 8px 4px;
    margin-bottom: -4px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    flex: 0 1 190px;
    display: flex;
    align-items: center;
    min-width: 90px;
    border-radius: var(--radius);
    color: var(--muted);
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .tab:hover {
    background: var(--hover);
    color: var(--ink);
  }
  /* L'onglet actif est une petite carte, de la même matière que les panneaux : ni cadre ni trait. */
  .tab.current {
    background: var(--panel);
    color: var(--ink);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 4px 6px 10px;
    border: 0;
    background: none;
    cursor: pointer;
    color: inherit;
    text-align: left;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--fw-regular);
  }
  .current .name {
    font-weight: var(--fw-strong);
  }
  .close,
  .add {
    flex: none;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      opacity var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  /* À l'appui, un bouton se resserre à 97 % (charte, sections 8 et 9.1). */
  .close:active,
  .add:active {
    transform: scale(0.97);
  }
  .close {
    width: 24px;
    height: 24px;
    margin-right: 4px;
    opacity: 0;
  }
  .tab:hover .close,
  .tab.current .close,
  .close:focus-visible {
    opacity: 1;
  }
  .add {
    width: 28px;
    height: 28px;
    margin-left: 2px;
  }
  /* La croix est sur un onglet survolé ou actif, où --hover ne se verrait pas : elle prend --line.
     Le « + » est sur le fond de la fenêtre : --hover (charte, section 9.7). */
  .close:hover {
    background: var(--line);
    color: var(--ink);
  }
  .add:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .spinner {
    margin: 1px;
  }
  .alert {
    display: grid;
    color: var(--warn);
  }
  .views {
    position: relative;
    min-height: 0;
  }
  .view {
    position: absolute;
    inset: 0;
    transition:
      opacity var(--t-med) var(--ease),
      visibility 0s;
  }
  .view.hidden {
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition:
      opacity var(--t-med) var(--ease),
      visibility 0s var(--t-med);
  }
</style>
