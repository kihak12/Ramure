<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { focusWithRing } from '../ui/focus';
  import { veil } from '../ui/veil';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh } from './ssh.svelte';

  // Écrire des hôtes du carnet dans ~/.ssh/config, après avoir montré ce qui change (charte, 9.15,
  // « Exporting to ~/.ssh/config »).

  type Status = 'added' | 'modified' | 'current' | 'blocked';
  type Row = { id: string; alias: string; status: Status; detail: string };
  type Exported = { count: number; backup: string | null; written: string };

  const NB = ' ';
  const STATE: Record<Status, string> = { added: 'Ajouté', modified: 'Modifié', current: 'À jour', blocked: 'Non exporté' };

  let element = $state<HTMLDialogElement>();
  let rows = $state<Row[]>([]);
  let checked = $state<Record<string, boolean>>({});
  let loaded = $state(false);
  let busy = $state(false);
  /** Ce qui a échoué : la lecture du fichier, ou son écriture. Dit dans la dernière ligne (9.11). */
  let failure = $state('');
  /** L'export a été tenté sans rien de coché : la raison passe en avertissement (charte, 9.1). */
  let tried = $state(false);
  /** L'export vient du clavier : le focus ira au bouton « Annuler » de la notification (9.11). */
  let byKeyboard = false;

  const single = $derived(ssh.exporting !== null && ssh.exporting !== 'all');
  const listed = $derived(rows.filter((row) => row.status !== 'current'));
  const upToDate = $derived(rows.filter((row) => row.status === 'current').length);
  const writable = $derived(rows.filter((row) => row.status === 'added' || row.status === 'modified'));
  const count = $derived(writable.filter((row) => checked[row.id]).length);
  const someModified = $derived(!single && writable.some((row) => row.status === 'modified'));
  const all = $derived(writable.length > 0 && count === writable.length);
  const plural = (n: number, word: string) => `${n}${NB}${word}${n > 1 ? 's' : ''}`;
  const WHY = 'Coche au moins un hôte.';

  $effect(() => {
    if (!element) return;
    if (ssh.exporting !== null && !element.open) {
      void load();
      element.showModal();
    } else if (ssh.exporting === null && element.open) {
      element.close();
    }
  });

  async function load() {
    rows = [];
    checked = {};
    loaded = false;
    failure = '';
    tried = false;
    const ids = ssh.exporting === 'all' ? null : [ssh.exporting];
    try {
      rows = await invoke<Row[]>('ssh_config_preview', { ids });
      // Les ajouts sont cochés ; une modification ne l'est que si on a demandé cet hôte-là.
      checked = Object.fromEntries(rows.map((row) => [row.id, row.status === 'added' || (single && row.status === 'modified')]));
    } catch (e) {
      failure = typeof e === 'string' ? e : String(e);
    }
    loaded = true;
    // Le focus va à la première case, sinon au bouton de la dernière ligne (charte, 9.10).
    await tick();
    focusWithRing(element?.querySelector<HTMLElement>('input[type="checkbox"], footer .btn:last-child'));
  }

  function checkAll(value: boolean) {
    for (const row of writable) checked[row.id] = value;
  }

  const close = () => (ssh.exporting = null);

  $effect(() => {
    if (count) tried = false;
  });

  /** Entrée sur une case lance l'export, comme sur un champ ; sans rien de coché, dit pourquoi. */
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !(e.target instanceof HTMLInputElement)) return;
    e.preventDefault();
    if (!count) {
      tried = true;
      return;
    }
    byKeyboard = true;
    (e.currentTarget as HTMLFormElement).requestSubmit();
  }

  async function exportChosen(e: SubmitEvent) {
    e.preventDefault();
    if (!count || busy) return;
    busy = true;
    failure = '';
    const ids = writable.filter((row) => checked[row.id]).map((row) => row.id);
    const alias = writable.find((row) => checked[row.id])?.alias ?? '';
    const keyboard = byKeyboard;
    byKeyboard = false;
    try {
      const done = await invoke<Exported>('ssh_config_export', { ids });
      close();
      // La boîte est fermée : la notification n'est plus sous son voile, et propose de défaire.
      toasts.ok(
        done.count === 1 ? `${alias} exporté vers ~/.ssh/config` : `${plural(done.count, 'hôte')} exportés vers ~/.ssh/config`,
        done.backup ? `Ancien fichier${NB}: ~/.ssh/config.ramure.bak` : '~/.ssh/config a été créé.',
        { label: 'Annuler', run: () => void undo(done), focus: keyboard },
      );
    } catch (err) {
      failure = typeof err === 'string' ? err : String(err);
    } finally {
      busy = false;
    }
  }

  async function undo(done: Exported) {
    try {
      await invoke('ssh_config_undo', { written: done.written, hadBackup: done.backup !== null });
      toasts.ok(done.backup ? '~/.ssh/config est revenu comme avant l’export' : '~/.ssh/config a été retiré');
    } catch (err) {
      toasts.err('L’export n’a pas pu être annulé', typeof err === 'string' ? err : String(err));
    }
  }
</script>

<dialog
  bind:this={element}
  aria-labelledby="export-title"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) close();
  }}
  use:veil={() => {
    // Une écriture ne s'arrête pas en cours de route : rien ne ferme la boîte avant qu'elle finisse.
    if (!busy) close();
  }}
>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="box" onsubmit={exportChosen} onkeydown={onKey}>
    <header>
      <h2 id="export-title">Exporter vers ~/.ssh/config</h2>
      <p>Ramure ajoute ou met à jour ces hôtes. Le reste du fichier n’est pas touché, et l’ancien fichier est copié dans <code>~/.ssh/config.ramure.bak</code>.</p>
      {#if someModified}
        <p>Une entrée modifiée garde la version du fichier tant que tu ne la coches pas.</p>
      {/if}
    </header>

    <div class="list">
      {#if loaded && listed.length}
        {#if writable.length > 1}
          <label class="all">
            <input type="checkbox" checked={all} indeterminate={count > 0 && !all} onchange={(e) => checkAll(e.currentTarget.checked)} />
            <span>Tout cocher</span>
          </label>
        {/if}
        <ul>
          {#each listed as row (row.id)}
            <li class:blocked={row.status === 'blocked'}>
              {#if row.status === 'blocked'}
                <div class="row">
                  <span class="mark"><Icon name="warning" size={14} /></span>
                  <span class="text">
                    <span class="line"><b>{row.alias}</b> <span class="state">{STATE[row.status]}</span></span>
                    <small>{row.detail}</small>
                  </span>
                </div>
              {:else}
                <label class="row">
                  <input type="checkbox" bind:checked={checked[row.id]} />
                  <span class="text">
                    <span class="line"><b>{row.alias}</b> <span class="state">{STATE[row.status]}</span></span>
                    <small class="mono">{row.detail}</small>
                  </span>
                </label>
              {/if}
            </li>
          {/each}
        </ul>
        {#if upToDate}
          <p class="count">{plural(upToDate, 'hôte')} {upToDate > 1 ? 'sont' : 'est'} déjà à jour dans le fichier.</p>
        {/if}
      {:else if loaded && !failure}
        <p class="none">
          {#if !rows.length}
            Le carnet est vide{NB}: rien à exporter.
          {:else if single}
            {rows[0].alias} est déjà à jour dans ~/.ssh/config.
          {:else}
            Tous les hôtes sont déjà à jour dans ~/.ssh/config.
          {/if}
        </p>
      {/if}
    </div>

    <!-- La sortie, puis l'export, à droite ; à gauche, pourquoi il est grisé, ou ce qui a échoué. -->
    <footer>
      <div class="result">
        {#if failure}
          <p class="bad" role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>
        {:else if writable.length && !count && tried}
          <p class="tried" role="alert"><Icon name="warning" size={14} /><span>{WHY}</span></p>
        {:else if writable.length && !count}
          <p class="why">{WHY}</p>
        {:else if loaded && listed.length && !writable.length}
          <p class="why">Aucun de ces hôtes ne peut être exporté.</p>
        {/if}
      </div>
      <!-- La dernière ligne paraît une fois le fichier lu : ses boutons ne changent pas sous le pointeur. -->
      {#if loaded && writable.length}
        <button class="btn" type="button" disabled={busy} onclick={close}>Annuler</button>
        <button
          class="btn primary"
          type="submit"
          disabled={!count || busy}
          title={count ? undefined : WHY}
          onclick={(e) => (byKeyboard = e.detail === 0)}
        >
          {busy ? 'Export en cours…' : count ? `Exporter ${plural(count, 'hôte')}` : 'Exporter'}
        </button>
      {:else if loaded}
        <button class="btn" type="button" onclick={close}>Fermer</button>
      {/if}
    </footer>
  </form>
</dialog>

<style>
  dialog {
    width: min(640px, calc(100vw - 48px));
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--ink);
    box-shadow: var(--shadow-pop);
  }
  dialog[open] {
    display: flex;
    animation: pop-in var(--t-med) var(--ease);
  }
  dialog::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(2px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .box {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 0;
    margin: 0;
  }
  header {
    padding: 16px 20px 8px;
    display: grid;
    gap: 6px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  header p {
    margin: 0;
    color: var(--muted);
  }
  code {
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  /* Des lignes simples, séparées par un filet --line (charte, 9.6, liste dans une boîte). */
  .list {
    flex: 1 1 auto;
    overflow-y: auto;
    min-height: 0;
    padding: 4px 20px 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li + li {
    border-top: 1px solid var(--line);
  }
  .all {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 0;
  }
  /* La case et l'icône d'un hôte non exporté occupent la même colonne, alignées sur le nom. */
  .row input,
  .mark {
    margin-top: 2px;
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 15px;
    color: var(--warn);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .state {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  li.blocked .state {
    color: var(--warn);
  }
  small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .count,
  .none {
    margin: 0;
    padding: 8px 0;
    color: var(--muted);
  }
  .none {
    padding: 12px 0 4px;
  }
  footer {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 16px 20px;
  }
  footer .btn {
    flex: none;
  }
  .result {
    flex: 1;
    min-width: 0;
    padding-top: 8px;
    font-size: var(--fs-small);
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .result p {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
  }
  .result .why {
    color: var(--muted);
  }
  .result .tried {
    color: var(--warn);
  }
  .result .bad {
    color: var(--bad);
  }
  .result :global(svg) {
    flex: none;
  }
</style>
