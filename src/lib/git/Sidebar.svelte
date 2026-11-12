<script lang="ts">
  import { tick } from 'svelte';
  import { slide } from 'svelte/transition';
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { menu } from '../ui/menu.svelte';
  import { dropMenu, labelMenu, stashMenu, switchBranch, switchToRemote } from './actions';
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

  // --- Clavier : une liste à sections, un arbre (charte, 9.6) ---------------------------------
  // Le focus reste sur la liste, qui désigne sa ligne courante (`aria-activedescendant`) : les
  // flèches la déplacent, titres de section compris, et elle n'est qu'un arrêt de tabulation.
  // Les onglets masqués restent montés : les identifiants sont propres à chaque liste.
  const uid = $props.id();
  type Node = { id: string; section: Section; key: string; heading: boolean };
  let tree = $state<HTMLDivElement>();
  let cursor = $state<{ section: Section; key: string } | null>(null);
  let lastIndex = 0;

  const headingId = (section: Section) => `${uid}-${section}`;
  const rowId = (section: Section, i: number) => `${uid}-${section}-${i}`;
  const nodes = $derived.by(() => {
    const out: Node[] = [];
    const add = (section: Section, keys: string[]) => {
      out.push({ id: headingId(section), section, key: '', heading: true });
      if (isOpen(section)) keys.forEach((key, i) => out.push({ id: rowId(section, i), section, key, heading: false }));
    };
    add('local', branches.slice(0, MAX_ITEMS).map((b) => b.name));
    add('remote', remotes.slice(0, MAX_ITEMS).map((r) => r.name));
    add('tags', tags.slice(0, MAX_ITEMS).map((t) => t.name));
    add('stashes', stashes.map((s) => s.sha));
    return out;
  });
  const cursorIndex = $derived(cursor ? nodes.findIndex((n) => n.section === cursor?.section && n.key === cursor?.key) : -1);
  const cursorNode = $derived(cursorIndex >= 0 ? nodes[cursorIndex] : undefined);

  // La ligne courante disparaît (branche supprimée, renommée, filtrée, section repliée) : sa
  // voisine prend sa place, et le focus reste sur la liste.
  $effect(() => {
    if (cursorIndex >= 0) lastIndex = cursorIndex;
    else if (cursor && nodes.length) {
      const next = nodes[Math.min(lastIndex, nodes.length - 1)];
      cursor = { section: next.section, key: next.key };
    }
  });

  /** Fait de `node` la ligne courante, et la montre si la liste a défilé. */
  function point(node: Node | undefined) {
    if (!node) return;
    cursor = { section: node.section, key: node.key };
    void tick().then(() => document.getElementById(node.id)?.scrollIntoView({ block: 'nearest' }));
  }

  /** Un clic sur une ligne ou un titre : il devient la ligne courante, et la liste garde le focus. */
  function pick(section: Section, key: string) {
    cursor = { section, key };
    tree?.focus({ preventScroll: true });
  }

  function onKey(event: KeyboardEvent) {
    const node = cursorNode;
    const i = cursorIndex;
    switch (event.key) {
      case 'ArrowDown':
        point(nodes[Math.min(nodes.length - 1, i + 1)]);
        break;
      case 'ArrowUp':
        point(nodes[Math.max(0, i - 1)]);
        break;
      case 'Home':
        point(nodes[0]);
        break;
      case 'End':
        point(nodes[nodes.length - 1]);
        break;
      case 'ArrowRight':
        // Un titre replié se déplie ; déplié, on descend à sa première ligne.
        if (node?.heading && !isOpen(node.section)) toggle(node.section);
        else if (node?.heading && nodes[i + 1] && !nodes[i + 1].heading) point(nodes[i + 1]);
        break;
      case 'ArrowLeft':
        // Une ligne remonte à son titre ; un titre déplié se replie (pas pendant une recherche).
        if (node && !node.heading) point(nodes.find((n) => n.heading && n.section === node.section));
        else if (node && isOpen(node.section) && !needle) toggle(node.section);
        break;
      case 'Enter':
      case ' ':
        // Comme un clic : un titre se replie ou se déplie, une ligne montre son commit.
        if (node) document.getElementById(node.id)?.click();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  // La liste reçoit le focus sans ligne courante : la première le devient (charte, 9.6).
  function onFocus() {
    if (!cursorNode && nodes.length) point(nodes[0]);
  }

  function switchLocal(name: string) {
    void switchBranch(repo, name);
  }
  function switchRemote(name: string) {
    void switchToRemote(repo, name);
  }
</script>

{#snippet heading(section: Section, icon: IconName, title: string, count: number)}
  <button
    class="heading"
    class:cursor={cursorNode?.id === headingId(section)}
    type="button"
    id={headingId(section)}
    role="treeitem"
    aria-level={1}
    aria-selected={cursorNode?.id === headingId(section)}
    aria-expanded={isOpen(section)}
    tabindex="-1"
    onclick={() => {
      pick(section, '');
      toggle(section);
    }}
  >
    <span class="caret" class:open={isOpen(section)}><Icon name="chevron" size={14} /></span>
    <Icon name={icon} size={14} />
    <span class="title">{title}</span>
    <span class="count">{count}</span>
  </button>
{/snippet}

{#snippet rest(total: number)}
  {#if total > MAX_ITEMS}
    <li class="none" role="none">… et {total - MAX_ITEMS} autres : utilise le filtre</li>
  {/if}
{/snippet}

<aside class="sidebar" aria-label="Branches, tags et remises">
  <label class="search">
    <Icon name="search" size={14} />
    <!-- ↓ passe du filtre à la liste ; pendant une recherche, à la première ligne trouvée. Le focus
         donné par le script montre l'anneau, comme celui de Tab. -->
    <input
      type="search"
      placeholder="Filtrer"
      aria-label="Filtrer les branches, les tags et les remises"
      bind:value={filter}
      onkeydown={(e) => {
        if (e.key !== 'ArrowDown') return;
        e.preventDefault();
        if (needle) point(nodes.find((n) => !n.heading) ?? nodes[0]);
        focusWithRing(tree);
      }}
    />
  </label>

  <div
    class="scroll"
    role="tree"
    tabindex="0"
    aria-label="Branches, tags et remises"
    aria-activedescendant={cursorNode?.id}
    bind:this={tree}
    onkeydown={onKey}
    onfocus={onFocus}
  >
    {@render heading('local', 'laptop', 'Local', branches.length)}
    {#if isOpen('local')}
      <ul role="group" transition:slide={{ duration: motion.med }}>
        {#each branches.slice(0, MAX_ITEMS) as branch, i (branch.name)}
          <li role="none">
            <button
              type="button"
              class="item"
              class:current={branch.current}
              class:cursor={cursorNode?.id === rowId('local', i)}
              class:drop={dropOn === branch.name}
              id={rowId('local', i)}
              role="treeitem"
              aria-level={2}
              aria-selected={cursorNode?.id === rowId('local', i)}
              tabindex="-1"
              {...dnd(localLabel(branch.name, branch.current))}
              title="{branch.name}{branch.upstream ? ` → ${branch.upstream}` : ' (jamais publiée)'} — double-clic pour basculer, glisser sur une autre branche pour fusionner ou rebaser"
              onclick={() => {
                pick('local', branch.name);
                repo.selectSha(branch.sha);
              }}
              ondblclick={() => switchLocal(branch.name)}
              oncontextmenu={(e) => {
                pick('local', branch.name);
                menu.open(e, labelMenu(repo, localLabel(branch.name, branch.current), branch.sha));
              }}
            >
              <span class="mark">{#if repo.switching === branch.name}<span class="spinner" aria-hidden="true"></span>{:else if branch.current}<Icon name="check" size={14} />{/if}</span>
              <span class="name">{branch.name}</span>
              {#if branch.gone}<span class="chip gone" title="La branche distante suivie n’existe plus">orpheline</span>{/if}
              {#if branch.ahead}<span class="chip" title="{branch.ahead} commit(s) à pousser"><Icon name="up" size={10} />{branch.ahead}</span>{/if}
              {#if branch.behind}<span class="chip" title="{branch.behind} commit(s) à tirer"><Icon name="down" size={10} />{branch.behind}</span>{/if}
            </button>
          </li>
        {:else}
          <li class="none" role="none">Aucune branche</li>
        {/each}
        {@render rest(branches.length)}
      </ul>
    {/if}

    {@render heading('remote', 'cloud', 'Distant', remotes.length)}
    {#if isOpen('remote')}
      <ul role="group" transition:slide={{ duration: motion.med }}>
        {#each remotes.slice(0, MAX_ITEMS) as remote, i (remote.name)}
          <li role="none">
            <button
              type="button"
              class="item"
              class:cursor={cursorNode?.id === rowId('remote', i)}
              class:drop={dropOn === remote.name}
              id={rowId('remote', i)}
              role="treeitem"
              aria-level={2}
              aria-selected={cursorNode?.id === rowId('remote', i)}
              tabindex="-1"
              {...dnd(remoteLabel(remote.name))}
              title="{remote.name} — double-clic pour basculer sur la branche locale correspondante"
              onclick={() => {
                pick('remote', remote.name);
                repo.selectSha(remote.sha);
              }}
              ondblclick={() => switchRemote(remote.name)}
              oncontextmenu={(e) => {
                pick('remote', remote.name);
                menu.open(e, labelMenu(repo, remoteLabel(remote.name), remote.sha));
              }}
            >
              <span class="mark">{#if repo.switching === remote.name}<span class="spinner" aria-hidden="true"></span>{/if}</span>
              <span class="name">{remote.name}</span>
            </button>
          </li>
        {:else}
          <li class="none" role="none">{refs?.remoteNames.length ? 'Aucune branche distante' : 'Aucun dépôt distant'}</li>
        {/each}
        {@render rest(remotes.length)}
      </ul>
    {/if}

    {@render heading('tags', 'tag', 'Tags', tags.length)}
    {#if isOpen('tags')}
      <ul role="group" transition:slide={{ duration: motion.med }}>
        {#each tags.slice(0, MAX_ITEMS) as tag, i (tag.name)}
          <li role="none">
            <button
              type="button"
              class="item"
              class:cursor={cursorNode?.id === rowId('tags', i)}
              id={rowId('tags', i)}
              role="treeitem"
              aria-level={2}
              aria-selected={cursorNode?.id === rowId('tags', i)}
              tabindex="-1"
              title={tag.name}
              onclick={() => {
                pick('tags', tag.name);
                repo.selectSha(tag.sha);
              }}
              oncontextmenu={(e) => {
                pick('tags', tag.name);
                menu.open(e, labelMenu(repo, tagLabel(tag.name), tag.sha));
              }}
            >
              <span class="mark"></span>
              <span class="name">{tag.name}</span>
            </button>
          </li>
        {:else}
          <li class="none" role="none">Aucun tag</li>
        {/each}
        {@render rest(tags.length)}
      </ul>
    {/if}

    {@render heading('stashes', 'stash', 'Remises', stashes.length)}
    {#if isOpen('stashes')}
      <ul role="group" transition:slide={{ duration: motion.med }}>
        {#each stashes as stash, i (stash.sha)}
          <li role="none">
            <button
              type="button"
              class="item"
              class:current={repo.openFile?.key === `stash:${stash.index}`}
              class:cursor={cursorNode?.id === rowId('stashes', i)}
              id={rowId('stashes', i)}
              role="treeitem"
              aria-level={2}
              aria-selected={cursorNode?.id === rowId('stashes', i)}
              tabindex="-1"
              title="{stash.message} — clic pour voir le contenu, clic droit pour l’appliquer"
              onclick={() => {
                pick('stashes', stash.sha);
                repo.openStashDiff(stash.index, stash.message);
              }}
              oncontextmenu={(e) => {
                pick('stashes', stash.sha);
                menu.open(e, stashMenu(repo, stash));
              }}
            >
              <span class="mark mono">{stash.index}</span>
              <span class="name">{stash.message}</span>
            </button>
          </li>
        {:else}
          <li class="none" role="none">Aucune remise</li>
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
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .search:hover:not(:focus-within) {
    border-color: color-mix(in srgb, var(--muted) 50%, var(--line));
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
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
  /* Le focus reste sur la liste : c'est la ligne courante qui le montre, d'un anneau intérieur,
     jamais un cadre autour de la liste (charte, 9.6). `.focus-ring` : le focus rendu par le script
     à la fermeture d'un menu demandé au clavier (lib/ui/focus.ts). */
  .scroll:focus-visible,
  .scroll:global(.focus-ring):focus {
    outline: none;
  }
  .scroll:focus-visible .cursor,
  .scroll:global(.focus-ring):focus .cursor {
    box-shadow: inset 0 0 0 1.5px var(--accent);
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
    border-radius: var(--radius);
    transition: background var(--t-fast) var(--ease);
  }
  .heading:hover {
    background: var(--hover);
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
    color: var(--muted);
    font-size: var(--fs-label);
  }
</style>
