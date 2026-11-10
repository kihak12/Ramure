<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { comboOf, display, keys, refusal, SHORTCUTS } from '../ui/keys.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import { settingsPage } from './settings.svelte';

  // Action dont on attend la nouvelle combinaison.
  let recording = $state<string | null>(null);
  /** Ce qu'il faut dire sous une ligne : une combinaison refusée, ou reprise à une autre action. */
  let notice = $state<{ id: string; text: string } | null>(null);
  let filter = $state('');

  const custom = $derived(Object.keys(keys.overrides).length > 0);
  const fold = (text: string) => text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
  const needle = $derived(fold(filter.trim()));
  // Le filtre cherche dans le nom, le groupe et la combinaison telle qu'elle est affichée.
  const shown = $derived(
    SHORTCUTS.filter((s) => !needle || fold(`${s.title} ${s.group} ${display(keys.binding(s.id))}`).includes(needle)),
  );

  // Les raccourcis de l'application se taisent pendant qu'on en tape un nouveau.
  $effect(() => {
    keys.recording = recording !== null;
    return () => (keys.recording = false);
  });
  // Quitter la page abandonne l'enregistrement en cours.
  $effect(() => {
    if (!settingsPage.open) recording = null;
  });

  function start(id: string) {
    recording = id;
    notice = null;
  }

  function onKey(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    const id = recording;
    if (event.key === 'Escape') {
      recording = null;
      notice = null;
      return;
    }
    if (event.key === 'Backspace' || event.key === 'Delete') {
      keys.set(id, '');
      recording = null;
      notice = null;
      return;
    }
    const combo = comboOf(event);
    if (!combo) return; // touche de modification seule : on attend la suite
    const problem = refusal(combo);
    if (problem) {
      notice = { id, text: `${display(combo)} : ${problem}` };
      return;
    }
    const previous = keys.find(combo);
    const taken = previous && previous !== id ? SHORTCUTS.find((s) => s.id === previous) : null;
    keys.set(id, combo);
    notice = taken ? { id, text: `${display(combo)} était le raccourci de « ${taken.title} », qui n’en a plus.` } : null;
    recording = null;
  }

  function onFilterKey(event: KeyboardEvent) {
    // Échap vide d'abord le filtre ; la page ne se ferme qu'à l'appui suivant.
    if (event.key === 'Escape' && filter) {
      event.stopPropagation();
      filter = '';
    }
  }
</script>

<!-- En capture : la combinaison enregistrée ne doit déclencher aucune autre action. -->
<svelte:window onkeydowncapture={onKey} />

<SectionHeader
  title="Raccourcis clavier"
  note="Clique une combinaison, puis tape la nouvelle. Retour arrière la retire, Échap annule."
  changed={custom}
  onreset={() => {
    keys.resetAll();
    notice = null;
  }}
/>

<label class="search">
  <Icon name="search" size={14} />
  <input type="search" placeholder="Filtrer les raccourcis" aria-label="Filtrer les raccourcis" bind:value={filter} onkeydown={onFilterKey} />
</label>

<div class="list">
  {#each shown as shortcut, i (shortcut.id)}
    {#if i === 0 || shown[i - 1].group !== shortcut.group}
      <h3>{shortcut.group}</h3>
    {/if}
    {@const combo = keys.binding(shortcut.id)}
    <div class="row" class:recording={recording === shortcut.id}>
      <span class="title">{shortcut.title}</span>
      <button
        class="combo"
        type="button"
        aria-label={recording === shortcut.id
          ? `Raccourci de « ${shortcut.title} » : en attente de la nouvelle combinaison.`
          : `Raccourci de « ${shortcut.title} » : ${combo ? display(combo) : 'aucun'}. Cliquer pour le changer.`}
        onclick={() => start(shortcut.id)}
      >
        {#if recording === shortcut.id}
          <span class="wait">Appuie sur les touches…</span>
        {:else if combo}
          {#each combo.split('+') as part, n (n)}<kbd>{display(part)}</kbd>{/each}
        {:else}
          <span class="none">aucun</span>
        {/if}
      </button>
      <button
        class="reset"
        type="button"
        title="Revenir au raccourci par défaut"
        aria-label="Revenir au raccourci par défaut de « {shortcut.title} »"
        disabled={!keys.isCustom(shortcut.id)}
        onclick={() => {
          keys.reset(shortcut.id);
          notice = null;
        }}
      >
        <Icon name="undo" size={14} />
      </button>
    </div>
    {#if notice?.id === shortcut.id}
      <p class="notice" role="status"><Icon name="warning" /> <span>{notice.text}</span></p>
    {/if}
  {:else}
    <p class="empty">Aucun raccourci ne correspond à « {filter.trim()} ».</p>
  {/each}
</div>

<p class="foot">Dans un terminal, seuls les raccourcis avec Maj, Alt ou une touche de fonction sont actifs : les autres restent au shell.</p>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
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
    padding: 6px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .list {
    display: grid;
    gap: 2px;
  }
  h3 {
    margin: 0;
    padding: 12px 6px 4px;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  h3:first-child {
    padding-top: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 6px;
    border-radius: var(--radius);
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.recording {
    background: var(--accent-soft);
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .combo {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    min-width: 150px;
    min-height: 26px;
    padding: 2px 6px;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: none;
    cursor: pointer;
    transition: border-color var(--t-fast) var(--ease);
  }
  .combo:hover {
    border-color: var(--line);
    background: var(--raised);
  }
  kbd {
    padding: 0 6px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: var(--radius-xs);
    background: var(--bg);
    font: var(--fs-micro) / 18px var(--f-ui);
  }
  .none {
    color: var(--muted);
    font-size: var(--fs-label);
  }
  /* En --ink : sur --accent-soft, l'accent n'a pas le contraste d'un texte courant en thème clair. */
  .wait {
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
  }
  .reset {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .reset:hover:not(:disabled) {
    background: var(--line);
    color: var(--ink);
  }
  .reset:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .notice {
    display: flex;
    gap: 8px;
    margin: 2px 0 6px;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .notice span {
    color: var(--ink);
  }
  .empty,
  .foot {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .empty {
    padding: 12px 6px;
    font-size: var(--fs-body);
  }
</style>
