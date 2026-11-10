<script lang="ts">
  import Icon from './Icon.svelte';
  import { comboOf, display, keys, refusal, SHORTCUTS } from './keys.svelte';

  let element = $state<HTMLDialogElement>();
  // Action dont on attend la nouvelle combinaison.
  let recording = $state<string | null>(null);
  let message = $state('');

  const custom = $derived(Object.keys(keys.overrides).length > 0);

  $effect(() => {
    if (!element) return;
    if (keys.editing && !element.open) {
      recording = null;
      message = '';
      element.showModal();
    } else if (!keys.editing && element.open) {
      element.close();
    }
  });

  function onKey(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      recording = null;
      message = '';
      return;
    }
    if (event.key === 'Backspace' || event.key === 'Delete') {
      keys.set(recording, '');
      recording = null;
      message = '';
      return;
    }
    const combo = comboOf(event);
    if (!combo) return; // touche de modification seule : on attend la suite
    const problem = refusal(combo);
    if (problem) {
      message = `${display(combo)} : ${problem}`;
      return;
    }
    const previous = keys.find(combo);
    const taken = previous && previous !== recording ? SHORTCUTS.find((s) => s.id === previous) : null;
    keys.set(recording, combo);
    message = taken ? `${display(combo)} était attribué à « ${taken.title} » : cette action n'a plus de raccourci.` : '';
    recording = null;
  }

  function close() {
    recording = null;
    keys.editing = false;
  }
</script>

<!-- En capture : la combinaison enregistrée ne doit déclencher aucune autre action. -->
<dialog
  bind:this={element}
  onkeydowncapture={onKey}
  oncancel={(e) => {
    e.preventDefault();
    if (!recording) close();
  }}
  onclick={(e) => {
    if (e.target === element && !recording) close();
  }}
>
  <div class="box">
    <header>
      <h2>Raccourcis clavier</h2>
      <p>
        Clique un raccourci, puis appuie sur la nouvelle combinaison. Retour arrière le retire, Échap annule.
        Dans un terminal, seuls les raccourcis avec Maj, Alt ou une touche de fonction sont actifs : les autres restent au shell.
      </p>
    </header>

    <div class="list">
      {#each SHORTCUTS as shortcut, i (shortcut.id)}
        {#if i === 0 || SHORTCUTS[i - 1].group !== shortcut.group}
          <div class="group">{shortcut.group}</div>
        {/if}
        {@const combo = keys.binding(shortcut.id)}
        <div class="row" class:recording={recording === shortcut.id}>
          <span class="title">{shortcut.title}</span>
          <button
            class="combo"
            type="button"
            aria-label="Raccourci de « {shortcut.title} » : {combo ? display(combo) : 'aucun'}. Cliquer pour le changer."
            onclick={() => {
              recording = shortcut.id;
              message = '';
            }}
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
            onclick={() => keys.reset(shortcut.id)}
          >
            <Icon name="undo" size={14} />
          </button>
        </div>
      {/each}
    </div>

    <footer>
      <small class:warn={!!message}>{message || 'Les raccourcis sont enregistrés au fur et à mesure.'}</small>
      <button class="btn" type="button" disabled={!custom} onclick={() => keys.resetAll()}>Tout réinitialiser</button>
      <button class="btn primary" type="button" onclick={close}>Fermer</button>
    </footer>
  </div>
</dialog>

<style>
  dialog {
    width: min(600px, calc(100vw - 48px));
    max-height: calc(100vh - 80px);
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
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: 100%;
    min-height: 0;
  }
  header {
    padding: 18px 20px 8px;
    display: grid;
    gap: 6px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-label);
  }
  .list {
    overflow-y: auto;
    padding: 0 14px 6px;
  }
  .group {
    padding: 12px 6px 4px;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 6px;
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
    color: var(--faint);
    font-size: var(--fs-label);
  }
  .wait {
    color: var(--accent);
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    animation: blink var(--t-wait) var(--ease) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.45;
    }
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
    opacity: 0.25;
    cursor: default;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 20px 16px;
    border-top: 1px solid var(--line-soft);
  }
  footer small {
    flex: 1;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  footer small.warn {
    color: var(--warn);
  }
</style>
