<script lang="ts">
  import { slide } from 'svelte/transition';
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import { menu } from '../ui/menu.svelte';
  import { dropMenu, labelMenu, stashMenu, switchToRemote } from './actions';
  import { accepts, drag } from './drag.svelte';
  import { bare, type Label } from './labels';
  import type { Repo } from './repo.svelte';
  import { motion } from '../ui/motion';

  let { repo }: { repo: Repo } = $props();

  type Section = 'local' | 'remote' | 'tags' | 'stashes';
  const STORAGE_KEY = 'ramure.sidebar';
  // Au-delà, une section n'affiche que le début de sa liste : le filtre sert à trouver le reste.
  const MAX_ITEMS = 200;

  function readClosed(): Section[] {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '["tags"]');
    } catch {
      return ['tags'];
    }
  }

  let closed = $state<Section[]>(readClosed());
  let filter = $state('');

  const refs = $derived(repo.refs);
  const needle = $derived(filter.trim().toLowerCase());
  const match = (name: string) => !needle || name.toLowerCase().includes(needle);
  const branches = $derived((refs?.branches ?? []).filter((b) => match(b.name)));
  const remotes = $derived((refs?.remotes ?? []).filter((r) => match(r.name)));
  // Les tags les plus récents d'abord : l'ordre inverse du tri par nom s'en approche.
  const tags = $derived((refs?.tags ?? []).filter((t) => match(t.name)).reverse());
  const stashes = $derived((refs?.stashes ?? []).filter((s) => match(s.message)));

  function toggle(section: Section) {
    closed = closed.includes(section) ? closed.filter((s) => s !== section) : [...closed, section];
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(closed));
    } catch {
      /* stockage indisponible */
    }
  }
  // Pendant une recherche, toutes les sections sont dépliées.
  const isOpen = (section: Section) => needle !== '' || !closed.includes(section);

  const localLabel = (name: string, current: boolean): Label => ({
    name,
    kind: current ? 'head' : 'local',
    local: name,
    remotes: (refs?.remotes ?? []).filter((r) => bare(r.name) === name && r.sha === refs?.branches.find((b) => b.name === name)?.sha).map((r) => r.name),
    tag: null,
    title: name,
  });
  const remoteLabel = (name: string): Label => ({ name, kind: 'remote', local: null, remotes: [name], tag: null, title: name });
  const tagLabel = (name: string): Label => ({ name, kind: 'tag', local: null, remotes: [], tag: name, title: name });

  // Branche de la barre latérale survolée pendant un glissement.
  let dropOn = $state('');

  /** Gestionnaires de glisser-déposer d'une entrée de branche. */
  function dnd(label: Label) {
    return {
      draggable: true,
      ondragstart: (e: DragEvent) => {
        drag.label = label;
        e.dataTransfer?.setData('text/plain', label.name);
      },
      ondragend: () => {
        drag.label = null;
        dropOn = '';
      },
      ondragover: (e: DragEvent) => {
        if (!accepts(label)) return;
        e.preventDefault();
        dropOn = label.name;
      },
      ondragleave: () => (dropOn = ''),
      ondrop: (e: DragEvent) => {
        const source = drag.label;
        dropOn = '';
        drag.label = null;
        if (!source) return;
        e.preventDefault();
        menu.open(e, dropMenu(repo, source, label));
      },
    };
  }

  function switchLocal(name: string) {
    if (!repo.locked && name !== repo.branch) void repo.refOp({ op: 'switch', name }, `Basculé sur ${name}`, 'Impossible de basculer');
  }
  function switchRemote(name: string) {
    void switchToRemote(repo, name);
  }
</script>

{#snippet heading(section: Section, icon: IconName, title: string, count: number)}
  <button class="heading" type="button" aria-expanded={isOpen(section)} onclick={() => toggle(section)}>
    <span class="caret" class:open={isOpen(section)}><Icon name="chevron" size={12} /></span>
    <Icon name={icon} size={14} />
    <span class="title">{title}</span>
    <span class="count">{count}</span>
  </button>
{/snippet}

{#snippet rest(total: number)}
  {#if total > MAX_ITEMS}
    <li class="none">… et {total - MAX_ITEMS} autres : utilise le filtre</li>
  {/if}
{/snippet}

<aside class="sidebar" aria-label="Branches, tags et remises">
  <label class="search">
    <Icon name="search" size={14} />
    <input type="search" placeholder="Filtrer" aria-label="Filtrer les branches, les tags et les remises" bind:value={filter} />
  </label>

  <div class="scroll">
    {@render heading('local', 'laptop', 'Local', branches.length)}
    {#if isOpen('local')}
      <ul transition:slide={{ duration: motion.med }}>
        {#each branches.slice(0, MAX_ITEMS) as branch (branch.name)}
          <li>
            <button
              type="button"
              class="item"
              class:current={branch.current}
              class:drop={dropOn === branch.name}
              {...dnd(localLabel(branch.name, branch.current))}
              title="{branch.name}{branch.upstream ? ` → ${branch.upstream}` : ' (jamais publiée)'} — double-clic pour basculer, glisser sur une autre branche pour fusionner ou rebaser"
              onclick={() => repo.selectSha(branch.sha)}
              ondblclick={() => switchLocal(branch.name)}
              oncontextmenu={(e) => menu.open(e, labelMenu(repo, localLabel(branch.name, branch.current), branch.sha))}
            >
              <span class="mark">{#if repo.switching === branch.name}<span class="spinner" aria-hidden="true"></span>{:else if branch.current}<Icon name="check" size={13} />{/if}</span>
              <span class="name">{branch.name}</span>
              {#if branch.gone}<span class="chip gone" title="La branche distante suivie n'existe plus">orpheline</span>{/if}
              {#if branch.ahead}<span class="chip" title="{branch.ahead} commit(s) à pousser"><Icon name="up" size={10} />{branch.ahead}</span>{/if}
              {#if branch.behind}<span class="chip" title="{branch.behind} commit(s) à tirer"><Icon name="down" size={10} />{branch.behind}</span>{/if}
            </button>
          </li>
        {:else}
          <li class="none">Aucune branche</li>
        {/each}
        {@render rest(branches.length)}
      </ul>
    {/if}

    {@render heading('remote', 'cloud', 'Distant', remotes.length)}
    {#if isOpen('remote')}
      <ul transition:slide={{ duration: motion.med }}>
        {#each remotes.slice(0, MAX_ITEMS) as remote (remote.name)}
          <li>
            <button
              type="button"
              class="item"
              class:drop={dropOn === remote.name}
              {...dnd(remoteLabel(remote.name))}
              title="{remote.name} — double-clic pour basculer sur la branche locale correspondante"
              onclick={() => repo.selectSha(remote.sha)}
              ondblclick={() => switchRemote(remote.name)}
              oncontextmenu={(e) => menu.open(e, labelMenu(repo, remoteLabel(remote.name), remote.sha))}
            >
              <span class="mark">{#if repo.switching === remote.name}<span class="spinner" aria-hidden="true"></span>{/if}</span>
              <span class="name">{remote.name}</span>
            </button>
          </li>
        {:else}
          <li class="none">{refs?.remoteNames.length ? 'Aucune branche distante' : 'Aucun dépôt distant'}</li>
        {/each}
        {@render rest(remotes.length)}
      </ul>
    {/if}

    {@render heading('tags', 'tag', 'Tags', tags.length)}
    {#if isOpen('tags')}
      <ul transition:slide={{ duration: motion.med }}>
        {#each tags.slice(0, MAX_ITEMS) as tag (tag.name)}
          <li>
            <button
              type="button"
              class="item"
              title={tag.name}
              onclick={() => repo.selectSha(tag.sha)}
              oncontextmenu={(e) => menu.open(e, labelMenu(repo, tagLabel(tag.name), tag.sha))}
            >
              <span class="mark"></span>
              <span class="name">{tag.name}</span>
            </button>
          </li>
        {:else}
          <li class="none">Aucun tag</li>
        {/each}
        {@render rest(tags.length)}
      </ul>
    {/if}

    {@render heading('stashes', 'stash', 'Remises', stashes.length)}
    {#if isOpen('stashes')}
      <ul transition:slide={{ duration: motion.med }}>
        {#each stashes as stash (stash.sha)}
          <li>
            <button
              type="button"
              class="item"
              class:current={repo.openFile?.key === `stash:${stash.index}`}
              title="{stash.message} — clic pour voir le contenu, clic droit pour l'appliquer"
              onclick={() => repo.openStashDiff(stash.index, stash.message)}
              oncontextmenu={(e) => menu.open(e, stashMenu(repo, stash))}
            >
              <span class="mark mono">{stash.index}</span>
              <span class="name">{stash.message}</span>
            </button>
          </li>
        {:else}
          <li class="none">Aucune remise</li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 12px 12px 8px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    padding: 5px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .scroll {
    overflow-y: auto;
    min-height: 0;
    padding-bottom: 8px;
  }
  .heading {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 8px 12px 6px;
    border: 0;
    background: none;
    cursor: pointer;
    color: var(--ink);
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    transition: color var(--t-fast) var(--ease);
  }
  .heading:hover {
    color: var(--accent);
  }
  .caret {
    display: grid;
    transition: transform var(--t-med) var(--ease);
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .title {
    flex: 1;
    text-align: left;
  }
  .count {
    font-variant-numeric: tabular-nums;
    font-weight: var(--fw-regular);
    color: var(--muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 6px 4px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    padding: 5px 10px 5px 6px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    cursor: pointer;
    text-align: left;
    transition: background var(--t-fast) var(--ease);
  }
  .item:hover {
    background: var(--hover);
  }
  .item.current {
    background: var(--sel);
    font-weight: var(--fw-strong);
  }
  .item.drop {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    background: var(--accent-soft);
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    color: var(--accent);
    font-size: var(--fs-micro);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip {
    background: var(--panel);
    border: 1px solid var(--line);
    font-weight: var(--fw-regular);
  }
  .gone {
    color: var(--warn);
  }
  .none {
    padding: 3px 8px 3px 28px;
    color: var(--faint);
    font-size: var(--fs-label);
  }
</style>
