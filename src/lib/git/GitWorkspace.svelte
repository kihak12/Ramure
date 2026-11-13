<script lang="ts">
  import { tick } from 'svelte';
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import { display, keys } from '../ui/keys.svelte';
  import CloneDialog from './CloneDialog.svelte';
  import RepoView from './RepoView.svelte';
  import { workspace } from './workspace.svelte';
  import { motion } from '../ui/motion';

  /** True when the Git module is on screen. */
  let { active = true }: { active?: boolean } = $props();

  // The active tab stays on screen when the tabs overflow: the bar scrolls to it, and the scroll
  // margin keeps it to the left of the “+”.
  let tabsEl = $state<HTMLElement>();
  $effect(() => {
    void workspace.activeId;
    void tick().then(() => tabsEl?.querySelector('.tab.current')?.scrollIntoView({ block: 'nearest', inline: 'nearest' }));
  });

  // The “+” says what it opens, with its shortcut (guidelines, section 9.7).
  const newTab = $derived(`New tab${keys.binding('tab-new') ? ` (${display(keys.binding('tab-new'))})` : ''}`);
</script>

<div class="workspace">
  <div class="tabs" role="tablist" aria-label="Repository tabs" bind:this={tabsEl}>
    {#each workspace.tabs as tab (tab.id)}
      {@const current = tab.id === workspace.activeId}
      <div class="tab" class:current animate:flip={{ duration: motion.med }} in:scale={{ duration: motion.med, start: 0.9 }}>
        <button
          class="open"
          type="button"
          role="tab"
          aria-selected={current}
          title={tab.repo.path || 'New tab'}
          onclick={() => workspace.activate(tab.id)}
          onauxclick={(e) => {
            // Middle click: closes the tab.
            if (e.button === 1) workspace.closeTab(tab.id);
          }}
        >
          {#if tab.repo.locked}
            <span class="spinner" aria-label="Working"></span>
          {:else if tab.repo.status?.operation}
            <span class="alert" title="An operation is waiting to be finished"><Icon name="warning" size={14} /></span>
          {:else}
            <Icon name={tab.repo.path ? 'branch' : 'plus'} size={14} />
          {/if}
          <span class="name">{workspace.title(tab)}</span>
        </button>
        <button class="close" type="button" aria-label="Close tab {workspace.title(tab)}" title="Close tab" onclick={() => workspace.closeTab(tab.id)}>
          <Icon name="x" size={14} />
        </button>
      </div>
    {/each}
    <button class="add" type="button" aria-label={newTab} title={newTab} onclick={() => workspace.newTab()}>
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
    /* 4 px under the tabs, given back by a negative margin: room for the focus ring, which the
       horizontal scrolling would clip otherwise. */
    padding: 8px 8px 4px;
    margin-bottom: -4px;
    min-width: 0;
    overflow-x: auto;
    /* A tab that takes the focus scrolls until it is whole to the left of the “+”, which sticks to the edge. */
    scroll-padding-inline-end: 40px;
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
    background: var(--hover-side);
    color: var(--ink);
  }
  /* The active tab is a small card, of the same material as the panels: no frame, no line. */
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
    /* The corners of the tab: the focus ring follows them. */
    border-radius: var(--radius);
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
  /* When pressed, a button shrinks to 97% (guidelines, sections 8 and 9.1). */
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
  /* It stays visible when the tabs overflow (guidelines, section 9.7). */
  .add {
    position: sticky;
    right: 0;
    width: 28px;
    height: 28px;
    margin-left: 2px;
    background: var(--side);
    /* The 8 px of margin on its right keep the background of the bar: no tab shows past it. */
    box-shadow: 8px 0 0 var(--side);
  }
  /* The cross sits on a hovered tab (--hover-side) or on the active one (--panel): a veil of --ink
     shows on both, in light and in dark. The “+” sits on the window background: --hover-side
     (guidelines, section 9.7). */
  .close:hover {
    background: color-mix(in srgb, var(--ink) 12%, transparent);
    color: var(--ink);
  }
  .add:hover {
    background: var(--hover-side);
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
