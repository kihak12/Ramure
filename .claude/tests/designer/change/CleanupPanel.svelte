<script lang="ts">
  // Panneau « Nettoyer les branches » : liste les branches locales déjà fusionnées et propose de les supprimer.
  import Icon from '../ui/Icon.svelte';
  import { confirmBody, failure, summary } from './cleanup';
  import type { Repo } from './repo.svelte';

  interface Merged {
    name: string;
    sha: string;
    merged: boolean;
    commits: number;
  }

  let { repo, branches, onclose }: { repo: Repo; branches: Merged[]; onclose: () => void } = $props();

  let filter = $state('');
  let selected = $state<string[]>([]);
  let active = $state(0);
  let loading = $state(false);
  let confirming = $state(false);
  let error = $state('');

  const shown = $derived(branches.filter((b) => b.name.includes(filter.trim())));

  function toggle(name: string) {
    selected = selected.includes(name) ? selected.filter((n) => n !== name) : [...selected, name];
  }

  // La liste garde le focus et gère elle-même les flèches et la barre d'espace.
  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = Math.min(shown.length - 1, active + 1);
    else if (e.key === 'ArrowUp') active = Math.max(0, active - 1);
    else if (e.key === ' ' && shown[active]) toggle(shown[active].name);
    else return;
    e.preventDefault();
  }

  function onEscape(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !confirming) return;
    e.preventDefault();
    confirming = false;
  }

  async function fetchAll() {
    loading = true;
    await repo.sync('fetch');
    loading = false;
  }

  async function remove() {
    confirming = false;
    error = '';
    for (const name of selected) {
      const ok = await repo.refOp({ op: 'branch-delete', name, force: false }, `Branche ${name} supprimée`);
      if (!ok) {
        error = failure();
        break;
      }
    }
    selected = [];
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="nettoyage" aria-label="Nettoyer les branches" onkeydown={onEscape}>
  <header>
    <h3>Branches fusionnées</h3>
    <span class="compte">{branches.length}</span>
    <button class="btn primary small" type="button" disabled={loading} onclick={() => void fetchAll()}>
      {#if loading}<span class="tourne"><Icon name="refresh" size={14} /></span>{/if}
      Fetch
    </button>
    <button class="btn ghost small" type="button" aria-label="Fermer le panneau" title="Fermer le panneau" onclick={onclose}>
      <Icon name="x" size={14} />
    </button>
  </header>

  <p class="aide">Cochez les branches à supprimer. Une branche fusionnée ne contient plus aucun commit à elle.</p>

  <label class="filtre">
    <Icon name="search" size={14} />
    <input type="text" placeholder="Filtrer les branches" aria-label="Filtrer les branches" bind:value={filter} />
    {#if filter}
      <button class="effacer" type="button" aria-label="Effacer le filtre" title="Effacer le filtre" onclick={() => (filter = '')}>
        <Icon name="x" size={14} />
      </button>
    {/if}
  </label>

  <div class="corps">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="liste" role="listbox" aria-label="Branches fusionnées" aria-multiselectable="true" tabindex="0" onkeydown={onKey}>
      {#each shown as branch, i (branch.name)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="ligne"
          class:sel={selected.includes(branch.name)}
          class:actif={i === active}
          role="option"
          tabindex="-1"
          aria-selected={selected.includes(branch.name)}
          onclick={() => toggle(branch.name)}
        >
          <input type="checkbox" tabindex="-1" checked={selected.includes(branch.name)} aria-hidden="true" />
          <span class="etat" class:fusionnee={branch.merged}></span>
          <span class="nom" title={branch.name}>{branch.name}</span>
          <span class="sha">{branch.sha.slice(0, 7)}</span>
          <span class="commits">{branch.commits} commit{branch.commits > 1 ? 's' : ''}</span>
        </div>
      {/each}
    </div>

    <aside class="resume">
      <span class="label">Sélection</span>
      <p>{summary(selected.length)}</p>
      <p class="note">La branche courante et HEAD ne sont jamais proposés.</p>
    </aside>
  </div>

  {#if error}<p class="erreur" role="alert">⚠️ {error}</p>{/if}

  <footer>
    <button class="btn" type="button" onclick={onclose}>Fermer</button>
    <button class="btn primary" type="button" tabindex="1" disabled={!selected.length} onclick={() => (confirming = true)}>
      <Icon name="trash" size={15} /> Supprimer la sélection
    </button>
  </footer>

  {#if confirming}
    <div class="confirmation" role="alertdialog" aria-label="Supprimer les branches">
      <h4>Supprimer les branches</h4>
      <p>{confirmBody(selected.length)}</p>
      <div class="actions">
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn danger" type="button" autofocus onclick={() => void remove()}>Supprimer</button>
        <button class="btn" type="button" onclick={() => (confirming = false)}>Annuler</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .nettoyage {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: var(--radius-card);
    background: var(--panel);
    box-shadow: var(--shadow-soft);
    animation: entre var(--t-med) var(--ease);
  }
  @keyframes entre {
    from {
      opacity: 0;
      transform: translateY(48px);
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h3 {
    flex: 1;
    margin: 0;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--accent);
  }
  .compte {
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--sel);
    font-size: var(--fs-small);
  }
  .tourne {
    display: grid;
    animation: tourne var(--t-spin) linear infinite;
  }
  @keyframes tourne {
    to {
      transform: rotate(360deg);
    }
  }
  .aide {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .filtre {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .filtre:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .filtre input {
    flex: 1;
    min-width: 0;
    padding: 6px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .filtre input::placeholder {
    color: var(--faint);
  }
  .effacer {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .effacer:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .corps {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 200px;
    gap: 12px;
    min-height: 0;
  }
  .liste {
    overflow-y: auto;
    max-height: 260px;
    padding-right: 12px;
    border-right: 1px solid var(--line-soft);
  }
  /* Le focus se lit sur la ligne active, pas par un cadre autour de la liste. */
  .liste:focus-visible {
    outline: none;
  }
  .liste:focus-visible .ligne.actif {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .ligne {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .ligne:hover {
    background: var(--sel);
  }
  .ligne.sel {
    background: var(--sel);
  }
  .ligne input {
    width: 12px;
    height: 12px;
    margin: 0;
  }
  .etat {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--warn);
  }
  .etat.fusionnee {
    background: var(--ok);
  }
  .nom {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sha {
    color: var(--muted);
    font-size: var(--fs-code);
  }
  .commits {
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
  }
  .resume {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border-radius: var(--radius-card);
    background: var(--raised);
  }
  .resume p {
    margin: 0;
  }
  .note {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .erreur {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--bad-soft);
    color: var(--bad);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 10px;
    border-top: 2px solid var(--ok);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .confirmation {
    position: absolute;
    right: 12px;
    bottom: 56px;
    z-index: 5;
    display: grid;
    gap: 10px;
    width: min(360px, calc(100% - 24px));
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    animation: pop-in var(--t-med) var(--ease);
  }
  h4 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  .confirmation p {
    margin: 0;
    color: var(--muted);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
