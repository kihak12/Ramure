<script lang="ts">
  import FileList from './FileList.svelte';
  import type { FileChange, Status } from './types';

  let {
    status,
    busy,
    active,
    message = $bindable(''),
    onopen,
    onstage,
    onunstage,
    ondiscard,
    oncommit,
  }: {
    status: Status;
    busy: boolean;
    /** Fichier dont le diff est ouvert : `u:chemin` (non indexé) ou `s:chemin` (indexé). */
    active: string | null;
    message?: string;
    onopen: (file: FileChange, staged: boolean) => void;
    onstage: (files: FileChange[]) => void;
    onunstage: (files: FileChange[]) => void;
    ondiscard: (files: FileChange[]) => void;
    oncommit: () => void;
  } = $props();

  const canCommit = $derived(status.staged.length > 0 && message.trim().length > 0 && !busy);
  const subjectLength = $derived(message.split('\n')[0].length);
  const activeIn = (prefix: string) => (active?.startsWith(prefix) ? active.slice(2) : null);
  const plural = (n: number) => (n > 1 ? 's' : '');

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && canCommit) {
      e.preventDefault();
      oncommit();
    }
  }
</script>

<div class="work">
  <section>
    <header>
      <span class="label">Non indexé · {status.unstaged.length}</span>
      <button class="btn small" type="button" disabled={busy || !status.unstaged.length} onclick={() => onstage(status.unstaged)}>
        Tout indexer
      </button>
    </header>
    {#if status.unstaged.length}
      <div class="list">
        <FileList
          files={status.unstaged}
          active={activeIn('u:')}
          disabled={busy}
          onopen={(f) => onopen(f, false)}
          actions={[
            { label: 'Abandonner les modifications', glyph: '↺', danger: true, run: (f) => ondiscard([f]) },
            { label: 'Indexer', glyph: '+', run: (f) => onstage([f]) },
          ]}
        />
      </div>
    {:else}
      <p class="muted">Rien à indexer.</p>
    {/if}
  </section>

  <section>
    <header>
      <span class="label">Indexé · {status.staged.length}</span>
      <button class="btn small" type="button" disabled={busy || !status.staged.length} onclick={() => onunstage(status.staged)}>
        Tout retirer
      </button>
    </header>
    {#if status.staged.length}
      <div class="list">
        <FileList
          files={status.staged}
          active={activeIn('s:')}
          disabled={busy}
          onopen={(f) => onopen(f, true)}
          actions={[{ label: "Retirer de l'index", glyph: '−', run: (f) => onunstage([f]) }]}
        />
      </div>
    {:else}
      <p class="muted">Indexe les fichiers à inclure dans le commit.</p>
    {/if}
  </section>

  <section class="compose">
    <label class="label" for="commit-message">Message de commit</label>
    <textarea
      id="commit-message"
      class="field"
      rows="4"
      placeholder="Résumé sur la première ligne, détails ensuite"
      bind:value={message}
      onkeydown={onKey}
      disabled={busy}
    ></textarea>
    <div class="send">
      <small class="muted" class:long={subjectLength > 72} title="Longueur de la première ligne ; au-delà de 72 caractères, elle est tronquée dans beaucoup d'outils.">
        {subjectLength} / 72
      </small>
      <button class="btn primary" type="button" disabled={!canCommit} onclick={oncommit} title="Ctrl + Entrée">
        {#if status.staged.length}
          Commiter {status.staged.length} fichier{plural(status.staged.length)}
        {:else}
          Commiter
        {/if}
      </button>
    </div>
  </section>
</div>

<style>
  .work {
    display: grid;
    grid-template-rows: minmax(60px, 1fr) minmax(60px, 1fr) auto;
    gap: 12px;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .list {
    overflow: auto;
    min-height: 0;
    margin: 0 -6px;
  }
  .small {
    padding: 2px 8px;
    font-size: 12px;
  }
  .muted {
    margin: 0;
    color: var(--muted);
  }
  textarea {
    resize: vertical;
    min-height: 64px;
    max-height: 220px;
    font-family: var(--f-mono);
    font-size: 12px;
  }
  .send {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .send small {
    font-variant-numeric: tabular-nums;
  }
  .send small.long {
    color: var(--warn);
  }
</style>
