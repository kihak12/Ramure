<script lang="ts">
  import { slide } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import { discardFiles } from './actions';
  import FileList from './FileList.svelte';
  import type { Repo } from './repo.svelte';
  import type { FileChange } from './types';
  import { motion } from '../ui/motion';

  let { repo }: { repo: Repo } = $props();

  const status = $derived(repo.status!);
  const busy = $derived(repo.locked);
  const conflicts = $derived(repo.conflicts);
  const unstaged = $derived(status.unstaged.filter((f) => f.kind !== 'conflict'));
  const message = $derived(repo.message);
  // Un amend peut ne changer que le message : rien à indexer n'est alors requis.
  const canCommit = $derived(
    (status.staged.length > 0 || repo.amend) && message.trim().length > 0 && !busy && conflicts.length === 0,
  );
  const subjectLength = $derived(message.split('\n')[0].length);
  const headPushed = $derived(repo.hasRemote && !!repo.head && !repo.head.localOnly);
  const plural = (n: number) => (n > 1 ? 's' : '');

  const active = $derived(repo.openFile?.key ?? null);
  const activeIn = (prefix: string) => (active?.startsWith(prefix) ? active.slice(2) : null);

  // Pendant un rebase, git inverse les rôles : « ours » est la base, « theirs » le commit rejoué.
  const sides = $derived(
    status.operation === 'rebase'
      ? { ours: 'Garder la version de la base', theirs: 'Garder ma modification' }
      : { ours: 'Garder ma version', theirs: 'Garder la version entrante' },
  );

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && canCommit) {
      e.preventDefault();
      void repo.commit();
    }
  }
</script>

<div class="work">
  {#if conflicts.length}
    <section class="conflicts" transition:slide={{ duration: motion.med }}>
      <header>
        <span class="label bad"><Icon name="warning" size={13} /> En conflit · {conflicts.length}</span>
      </header>
      <div class="list">
        <FileList
          files={conflicts}
          active={repo.conflictFile?.path ?? null}
          disabled={busy}
          onopen={(f) => repo.openConflict(f)}
          actions={[
            { label: sides.ours, icon: 'laptop', run: (f: FileChange) => void repo.resolve(f, 'ours') },
            { label: sides.theirs, icon: 'cloud', run: (f: FileChange) => void repo.resolve(f, 'theirs') },
            { label: 'Marquer comme résolu (après correction dans ton éditeur)', icon: 'check', run: (f: FileChange) => void repo.stage([f]) },
          ]}
        />
      </div>
      <p class="muted small">Clique un fichier pour ouvrir l'éditeur de conflits, ou garde directement un côté avec les boutons de sa ligne.</p>
    </section>
  {/if}

  <section>
    <header>
      <span class="label">Non indexé · {unstaged.length}</span>
      <button class="btn small" type="button" disabled={busy || !unstaged.length} onclick={() => void repo.stage(unstaged)}>
        Tout indexer
      </button>
    </header>
    {#if unstaged.length}
      <div class="list">
        <FileList
          files={unstaged}
          active={activeIn('u:')}
          disabled={busy}
          onopen={(f) => repo.openWorkDiff(f, false)}
          actions={[
            { label: 'Abandonner les modifications', icon: 'undo', danger: true, run: (f: FileChange) => void discardFiles(repo, [f]) },
            { label: 'Indexer', icon: 'plus', run: (f: FileChange) => void repo.stage([f]) },
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
      <button class="btn small" type="button" disabled={busy || !status.staged.length} onclick={() => void repo.unstage(status.staged)}>
        Tout retirer
      </button>
    </header>
    {#if status.staged.length}
      <div class="list">
        <FileList
          files={status.staged}
          active={activeIn('s:')}
          disabled={busy}
          onopen={(f) => repo.openWorkDiff(f, true)}
          actions={[{ label: "Retirer de l'index", icon: 'minus', run: (f: FileChange) => void repo.unstage([f]) }]}
        />
      </div>
    {:else}
      <p class="muted">Indexe les fichiers à inclure dans le commit. Ouvre un fichier modifié pour n'indexer que certaines lignes.</p>
    {/if}
  </section>

  <section class="compose">
    <label class="amend">
      <input type="checkbox" checked={repo.amend} disabled={busy || !repo.head} onchange={(e) => void repo.toggleAmend(e.currentTarget.checked)} />
      <span>Modifier le dernier commit</span>
    </label>
    {#if repo.amend && headPushed}
      <p class="warn small" transition:slide={{ duration: motion.med }}>
        Ce commit est déjà sur le dépôt distant : après modification, il faudra pousser en forçant.
      </p>
    {/if}
    <label class="label" for="commit-message">Message de commit</label>
    <textarea
      id="commit-message"
      class="field"
      rows="4"
      placeholder="Résumé sur la première ligne, détails ensuite"
      bind:value={repo.message}
      onkeydown={onKey}
      disabled={busy}
    ></textarea>
    <div class="send">
      <small class="muted" class:long={subjectLength > 72} title="Longueur de la première ligne ; au-delà de 72 caractères, elle est tronquée dans beaucoup d'outils.">
        {subjectLength} / 72
      </small>
      <button class="btn primary" type="button" disabled={!canCommit} onclick={() => void repo.commit()} title="Ctrl + Entrée">
        {#if repo.amend}
          Modifier le commit
        {:else if status.staged.length}
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
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  section {
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 56px;
  }
  section.conflicts,
  section.compose {
    flex: 0 0 auto;
  }
  .conflicts {
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--bad-soft);
    max-height: 40%;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .label.bad {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--bad);
  }
  .list {
    overflow: auto;
    min-height: 0;
    margin: 0 -6px;
  }
  .muted {
    margin: 0;
    color: var(--muted);
  }
  .small {
    margin: 0;
    font-size: var(--fs-small);
  }
  .warn {
    padding: 6px 8px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
  }
  .amend {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    color: var(--muted);
  }
  .amend input {
    margin: 0;
    accent-color: var(--accent);
  }
  textarea {
    resize: vertical;
    min-height: 64px;
    max-height: 220px;
    font-family: var(--f-mono);
    font-size: var(--fs-code);
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
