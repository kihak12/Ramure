<script lang="ts">
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import CloneDialog from './CloneDialog.svelte';
  import RepoView from './RepoView.svelte';
  import { workspace } from './workspace.svelte';

  /** Vrai quand le module Git est à l'écran. */
  let { active = true }: { active?: boolean } = $props();
</script>

<div class="workspace">
  <div class="tabs" role="tablist" aria-label="Dépôts ouverts">
    {#each workspace.tabs as tab (tab.id)}
      {@const current = tab.id === workspace.activeId}
      <div class="tab" class:current animate:flip={{ duration: 200 }} in:scale={{ duration: 160, start: 0.9 }}>
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
            <span class="alert" title="Une opération attend d'être terminée"><Icon name="warning" size={14} /></span>
          {:else}
            <Icon name={tab.repo.path ? 'branch' : 'plus'} size={14} />
          {/if}
          <span class="name">{workspace.title(tab)}</span>
        </button>
        <button class="close" type="button" aria-label="Fermer l'onglet {workspace.title(tab)}" title="Fermer l'onglet" onclick={() => workspace.closeTab(tab.id)}>
          <Icon name="x" size={12} />
        </button>
      </div>
    {/each}
    <button class="add" type="button" aria-label="Nouvel onglet" title="Nouvel onglet" onclick={() => workspace.newTab()}>
      <Icon name="plus" size={15} />
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
  }
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    padding: 5px 8px 0;
    background: var(--side);
    border-bottom: 1px solid var(--line);
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    position: relative;
    flex: 0 1 190px;
    display: flex;
    align-items: center;
    min-width: 90px;
    margin-bottom: -1px;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: 8px 8px 0 0;
    color: var(--muted);
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .tab:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .tab.current {
    background: var(--panel);
    border-color: var(--line);
    color: var(--ink);
  }
  /* Filet de couleur sur l'onglet actif. */
  .tab::before {
    content: '';
    position: absolute;
    left: 8px;
    right: 8px;
    top: -1px;
    height: 2px;
    border-radius: 0 0 2px 2px;
    background: var(--accent);
    transform: scaleX(0);
    transition: transform var(--t-med) var(--ease);
  }
  .tab.current::before {
    transform: scaleX(1);
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
    font-weight: 500;
  }
  .current .name {
    font-weight: 650;
  }
  .close,
  .add {
    flex: none;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      opacity var(--t-fast) var(--ease);
  }
  .close {
    width: 20px;
    height: 20px;
    margin-right: 5px;
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
    margin: 0 0 3px 2px;
  }
  .close:hover,
  .add:hover {
    background: var(--line);
    color: var(--ink);
  }
  .spinner {
    flex: none;
    width: 12px;
    height: 12px;
    margin: 1px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-top-color: var(--accent);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
