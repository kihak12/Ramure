<script lang="ts">
  import { veil } from './veil';
  import Icon from './Icon.svelte';
  import { fold, palette, score, type Command } from './palette.svelte';

  const MAX_RESULTS = 80;

  let element = $state<HTMLDialogElement>();
  let list = $state<HTMLDivElement>();
  let query = $state('');
  let active = $state(0);

  const commands = $derived(palette.commands);

  // Résultats dans l'ordre d'affichage : par rubrique, puis du plus pertinent au moins pertinent.
  const results = $derived.by(() => {
    if (!commands) return [];
    const words = fold(query).split(/\s+/).filter(Boolean);
    const order = new Map<string, number>();
    for (const command of commands) if (!order.has(command.group)) order.set(command.group, order.size);
    return commands
      .map((command, index) => ({ command, index, points: words.length ? score(command, words) : 1 }))
      .filter((entry) => entry.points > 0)
      .sort((a, b) => order.get(a.command.group)! - order.get(b.command.group)! || b.points - a.points || a.index - b.index)
      .slice(0, MAX_RESULTS)
      .map((entry) => entry.command);
  });

  $effect(() => {
    if (!element) return;
    if (commands && !element.open) {
      query = '';
      active = 0;
      element.showModal();
    } else if (!commands && element.open) {
      element.close();
    }
  });

  // Une nouvelle recherche repart du premier résultat utilisable.
  $effect(() => {
    void query;
    active = Math.max(0, results.findIndex((c) => !c.disabled));
  });

  function move(step: number) {
    if (!results.length) return;
    let next = active;
    for (let tries = 0; tries < results.length; tries++) {
      next = (next + step + results.length) % results.length;
      if (!results[next].disabled) break;
    }
    active = next;
    queueMicrotask(() => list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' }));
  }

  function run(command: Command | undefined) {
    if (!command || command.disabled) return;
    palette.hide();
    command.run();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      move(1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      move(-1);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      run(results[active]);
    }
  }
</script>

<dialog
  bind:this={element}
  aria-label="Palette de commandes"
  oncancel={(e) => {
    e.preventDefault();
    palette.hide();
  }}
  use:veil={() => palette.hide()}
>
  {#if commands}
    <div class="box">
      <label class="search">
        <Icon name="search" size={16} />
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="text"
          placeholder="Chercher une action, une branche, un dépôt…"
          spellcheck="false"
          autocomplete="off"
          autofocus
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-results"
          aria-activedescendant={results[active] ? `palette-${active}` : undefined}
          bind:value={query}
          onkeydown={onKey}
        />
        <kbd>Échap</kbd>
      </label>

      <div class="results" id="palette-results" role="listbox" bind:this={list}>
        {#each results as command, i (command.id)}
          {#if i === 0 || results[i - 1].group !== command.group}
            <div class="group">{command.group}</div>
          {/if}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="item"
            class:disabled={command.disabled}
            id="palette-{i}"
            role="option"
            tabindex="-1"
            aria-selected={i === active}
            aria-disabled={command.disabled}
            onmousemove={() => {
              if (!command.disabled) active = i;
            }}
            onclick={() => run(command)}
          >
            <span class="glyph">{#if command.icon}<Icon name={command.icon} size={16} />{/if}</span>
            <span class="title">{command.title}</span>
            {#if command.hint}<span class="hint">{command.hint}</span>{/if}
            {#if command.shortcut}<span class="keys">{#each command.shortcut.split(' + ') as part, n (n)}<kbd>{part}</kbd>{/each}</span>{/if}
          </div>
        {:else}
          <p class="none">Rien ne correspond à « {query} ».</p>
        {/each}
      </div>

      <footer>
        <span><kbd>↑</kbd><kbd>↓</kbd> choisir</span>
        <span><kbd>Entrée</kbd> lancer</span>
        <span class="count">{results.length} résultat{results.length > 1 ? 's' : ''}</span>
      </footer>
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(640px, calc(100vw - 48px));
    margin-top: 11vh;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--ink);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
  }
  dialog[open] {
    animation: drop var(--t-med) var(--ease);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-10px) scale(0.98);
    }
  }
  dialog::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(2px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .box {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    max-height: min(520px, 70vh);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    outline: none;
    font-size: var(--fs-title);
    color: var(--ink);
  }
  .results {
    overflow-y: auto;
    padding: 6px;
    min-height: 0;
  }
  .group {
    padding: 8px 8px 4px;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: var(--radius);
    cursor: pointer;
  }
  /* Les résultats sont des lignes (section 9.6) : la ligne active est sélectionnée, en --sel. */
  .item[aria-selected='true'] {
    background: var(--sel);
  }
  .item.disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .glyph {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--muted);
  }
  .title {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .keys {
    flex: none;
    display: inline-flex;
    gap: 2px;
    margin-left: auto;
  }
  .hint + .keys {
    margin-left: 8px;
  }
  .none {
    margin: 0;
    padding: 24px 12px;
    color: var(--muted);
    text-align: center;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 6px 16px;
    border-top: 1px solid var(--line);
    background: var(--panel);
    color: var(--muted);
    font-size: var(--fs-small);
  }
  footer span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .count {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
</style>
