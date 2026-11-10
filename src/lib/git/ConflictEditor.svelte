<script lang="ts">
  import { untrack } from 'svelte';
  import { fly } from 'svelte/transition';
  import CodeEditor, { type Mark } from '../ui/CodeEditor.svelte';
  import { dialogs } from '../ui/dialogs.svelte';
  import Icon from '../ui/Icon.svelte';
  import { locate, parseConflicts, splitLines, type ConflictBlock } from './conflicts';
  import type { Repo } from './repo.svelte';
  import type { ConflictFile, FileChange } from './types';

  let { repo, file }: { repo: Repo; file: FileChange } = $props();

  let data = $state.raw<ConflictFile | null>(null);
  let error = $state('');
  // Texte du résultat, tenu à jour par l'éditeur du bas.
  let text = $state('');
  let index = $state(0);
  let showBase = $state(false);

  let result = $state<ReturnType<typeof CodeEditor>>();
  let oursView = $state<ReturnType<typeof CodeEditor>>();
  let theirsView = $state<ReturnType<typeof CodeEditor>>();

  // Pendant un rebase, git inverse les rôles : « ours » est la base, « theirs » le commit rejoué.
  const rebase = $derived(repo.status?.operation === 'rebase');
  const names = $derived(
    rebase ? { ours: 'La base', theirs: 'Mon commit' } : { ours: 'Ma version', theirs: 'La version entrante' },
  );

  const separator = $derived(data?.merged?.includes('\r\n') ? '\r\n' : '\n');
  const lines = $derived(splitLines(text, separator));
  const blocks = $derived(parseConflicts(lines));
  const current = $derived<ConflictBlock | null>(blocks[Math.min(index, blocks.length - 1)] ?? null);
  const editable = $derived(!!data && !data.binary && data.merged !== null && data.ours !== null && data.theirs !== null);
  const modified = $derived(!!data && text !== (data.merged ?? ''));

  const oursLines = $derived(splitLines(data?.ours ?? '', separator));
  const theirsLines = $derived(splitLines(data?.theirs ?? '', separator));
  // Où se trouve, dans chaque version, le passage du conflit courant.
  const oursAt = $derived(current ? locate(oursLines, current.ours, current.start) : -1);
  const theirsAt = $derived(current ? locate(theirsLines, current.theirs, current.start) : -1);

  const resultMarks = $derived.by(() => {
    const marks: Mark[] = [];
    for (const block of blocks) {
      const middle = block.theirsAt - 1;
      const oursEnd = block.baseAt >= 0 ? block.baseAt : middle;
      marks.push({ from: block.start + 1, to: block.start + 1, kind: 'marker' });
      if (block.ours.length) marks.push({ from: block.oursAt + 1, to: oursEnd, kind: 'ours' });
      if (block.baseAt >= 0) {
        marks.push({ from: block.baseAt + 1, to: block.baseAt + 1, kind: 'marker' });
        if (block.base?.length) marks.push({ from: block.baseAt + 2, to: middle, kind: 'base' });
      }
      marks.push({ from: middle + 1, to: middle + 1, kind: 'marker' });
      if (block.theirs.length) marks.push({ from: block.theirsAt + 1, to: block.end, kind: 'theirs' });
      marks.push({ from: block.end + 1, to: block.end + 1, kind: 'marker' });
    }
    return marks;
  });
  const sideMarks = (at: number, length: number): Mark[] => (at >= 0 ? [{ from: at + 1, to: at + length, kind: 'focus' }] : []);
  const oursMarks = $derived(current ? sideMarks(oursAt, current.ours.length) : []);
  const theirsMarks = $derived(current ? sideMarks(theirsAt, current.theirs.length) : []);

  // Chargement des versions du fichier.
  $effect(() => {
    const target = file;
    data = null;
    error = '';
    repo
      .readConflict(target)
      .then((loaded) => {
        if (target !== file) return;
        data = loaded;
        text = loaded.merged ?? '';
        index = 0;
      })
      .catch((e: unknown) => {
        if (target === file) error = e instanceof Error ? e.message : String(e);
      });
  });

  // Le conflit courant est amené à l'écran dans les trois volets : quand on en change, ou quand
  // il en reste un de moins. Jamais pendant une simple saisie dans le résultat.
  $effect(() => {
    void index;
    void blocks.length;
    void data;
    untrack(() => {
      const block = current;
      if (!block) return;
      const [mine, theirs] = [oursAt, theirsAt];
      // Après la mise à jour des documents, pas pendant.
      queueMicrotask(() => {
        result?.scrollToLine(block.start + 1 + Math.floor((block.end - block.start) / 2));
        if (mine >= 0) oursView?.scrollToLine(mine + 1);
        if (theirs >= 0) theirsView?.scrollToLine(theirs + 1);
      });
    });
  });

  type Choice = 'ours' | 'theirs' | 'both' | 'both-reversed';

  const chosen = (block: ConflictBlock, choice: Choice) =>
    choice === 'ours'
      ? block.ours
      : choice === 'theirs'
        ? block.theirs
        : choice === 'both'
          ? [...block.ours, ...block.theirs]
          : [...block.theirs, ...block.ours];

  function take(choice: Choice) {
    if (current) result?.replaceLines(current.start + 1, current.end + 1, chosen(current, choice));
  }

  /** Applique le même choix à tous les conflits restants, du dernier au premier : les positions restent justes. */
  function takeAll(choice: 'ours' | 'theirs') {
    for (const block of [...blocks].reverse()) result?.replaceLines(block.start + 1, block.end + 1, chosen(block, choice));
  }

  const step = (by: number) => (index = (Math.min(index, blocks.length - 1) + by + blocks.length) % blocks.length);

  async function close() {
    if (modified) {
      const sure = await dialogs.confirm({
        title: 'Fermer sans enregistrer ?',
        body: 'Les choix et les corrections faits dans cet éditeur seront perdus. Le fichier garde ses marqueurs de conflit.',
        confirmLabel: 'Fermer sans enregistrer',
        danger: true,
      });
      if (!sure) return;
    }
    repo.conflictFile = null;
  }

  const save = () => void repo.saveConflict(file, result?.getText() ?? text);
</script>

<section class="conflict" aria-label="Éditeur de conflits : {file.path}" in:fly={{ y: 16, duration: 220 }} out:fly={{ y: 10, duration: 120 }}>
  <header>
    <span class="glyph"><Icon name="merge" size={18} /></span>
    <div class="names">
      <b class="mono">{file.path}</b>
      <small>
        {#if !data && !error}
          Lecture des versions…
        {:else if blocks.length}
          {blocks.length} conflit{blocks.length > 1 ? 's' : ''} à résoudre
        {:else if editable}
          Plus aucun marqueur de conflit : le fichier peut être enregistré
        {/if}
      </small>
    </div>
    <button class="btn" type="button" onclick={close}><Icon name="x" size={14} /> Fermer</button>
    <button class="btn primary" type="button" disabled={!editable || blocks.length > 0 || repo.locked} onclick={save} title="Écrit le fichier et le marque comme résolu (git add)">
      <Icon name="check" size={14} /> Enregistrer et marquer résolu
    </button>
  </header>

  {#if error}
    <p class="note err">{error}</p>
  {:else if !data}
    <p class="note">Lecture des versions du fichier…</p>
  {:else if !editable}
    <div class="note">
      <p>
        {#if data.binary}
          Ce fichier n'est pas du texte, ou il est trop volumineux pour l'éditeur.
        {:else}
          L'un des deux côtés a supprimé ce fichier : il n'y a pas deux textes à réunir.
        {/if}
        Résous-le en gardant un côté.
      </p>
      <div class="pick">
        <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.resolve(file, 'ours')}>Garder : {names.ours.toLowerCase()}</button>
        <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.resolve(file, 'theirs')}>Garder : {names.theirs.toLowerCase()}</button>
      </div>
    </div>
  {:else}
    <div class="sides">
      <div class="side ours">
        <div class="bar">
          <span class="dot"></span>
          <b>{names.ours}</b>
          {#if current?.oursLabel}<small class="mono">{current.oursLabel}</small>{/if}
          <button class="btn small" type="button" disabled={!blocks.length} onclick={() => takeAll('ours')} title="Prend ce côté pour tous les conflits restants">Tout prendre</button>
        </div>
        <CodeEditor bind:this={oursView} value={data.ours ?? ''} readonly marks={oursMarks} lineSeparator={separator} label="{names.ours}, en lecture seule" />
      </div>
      <div class="side theirs">
        <div class="bar">
          <span class="dot"></span>
          <b>{names.theirs}</b>
          {#if current?.theirsLabel}<small class="mono">{current.theirsLabel}</small>{/if}
          <button class="btn small" type="button" disabled={!blocks.length} onclick={() => takeAll('theirs')} title="Prend ce côté pour tous les conflits restants">Tout prendre</button>
        </div>
        <CodeEditor bind:this={theirsView} value={data.theirs ?? ''} readonly marks={theirsMarks} lineSeparator={separator} label="{names.theirs}, en lecture seule" />
      </div>
    </div>

    <div class="choose">
      {#if current}
        <div class="nav">
          <button class="btn small" type="button" aria-label="Conflit précédent" disabled={blocks.length < 2} onclick={() => step(-1)}><Icon name="up" size={13} /></button>
          <span class="count">Conflit <b>{Math.min(index, blocks.length - 1) + 1}</b> sur {blocks.length}</span>
          <button class="btn small" type="button" aria-label="Conflit suivant" disabled={blocks.length < 2} onclick={() => step(1)}><Icon name="down" size={13} /></button>
        </div>
        <button class="btn small take ours" type="button" onclick={() => take('ours')}>Prendre : {names.ours.toLowerCase()}</button>
        <button class="btn small take theirs" type="button" onclick={() => take('theirs')}>Prendre : {names.theirs.toLowerCase()}</button>
        <button class="btn small" type="button" onclick={() => take('both')} title="{names.ours}, puis {names.theirs.toLowerCase()}">Les deux</button>
        <button class="btn small" type="button" onclick={() => take('both-reversed')} title="{names.theirs}, puis {names.ours.toLowerCase()}">Les deux, inversés</button>
        {#if data.base !== null}
          <label class="base-toggle"><input type="checkbox" bind:checked={showBase} /> Voir l'ancêtre commun</label>
        {/if}
      {:else}
        <span class="done"><Icon name="check" size={15} /> Tous les conflits sont résolus. Relis le résultat, corrige-le au besoin, puis enregistre.</span>
        {#if data.base !== null}
          <label class="base-toggle"><input type="checkbox" bind:checked={showBase} /> Voir l'ancêtre commun</label>
        {/if}
      {/if}
    </div>

    <div class="bottom" class:split={showBase && data.base !== null}>
      <div class="side result">
        <div class="bar">
          <span class="dot"></span>
          <b>Résultat</b>
          <small>modifiable : les choix ci-dessus s'y appliquent, et tu peux le corriger à la main</small>
        </div>
        <CodeEditor bind:this={result} value={data.merged ?? ''} marks={resultMarks} lineSeparator={separator} label="Résultat de la résolution" onchange={(next) => (text = next)} />
      </div>
      {#if showBase && data.base !== null}
        <div class="side base">
          <div class="bar"><span class="dot"></span><b>Ancêtre commun</b><small>avant les deux modifications</small></div>
          <CodeEditor value={data.base} readonly lineSeparator={separator} label="Ancêtre commun, en lecture seule" />
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .conflict {
    position: absolute;
    inset: 0;
    z-index: 6;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    min-height: 0;
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    min-width: 0;
  }
  .glyph {
    display: grid;
    color: var(--warn);
  }
  .names {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.3;
  }
  .names b,
  .names small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .names small {
    color: var(--muted);
    font-size: 11.5px;
  }
  .note {
    margin: 0;
    padding: 20px;
    color: var(--muted);
  }
  .note p {
    margin: 0 0 12px;
    max-width: 70ch;
  }
  .note.err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .pick {
    display: flex;
    gap: 8px;
  }

  .sides {
    flex: 1 1 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    min-height: 0;
  }
  .bottom {
    flex: 1.15 1 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
  }
  .bottom.split {
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
  }
  .side {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    --c: var(--muted);
  }
  .side + .side {
    border-left: 1px solid var(--line);
  }
  .ours {
    --c: var(--l0);
  }
  .theirs {
    --c: var(--l1);
  }
  .result {
    --c: var(--ok);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--line-soft);
    background: color-mix(in srgb, var(--c) 9%, var(--panel));
    min-width: 0;
  }
  .bar small {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar b {
    white-space: nowrap;
  }
  .bar b:only-of-type:last-child {
    flex: 1;
  }
  .dot {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
  }

  .choose {
    flex: none;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 12px;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-right: 6px;
  }
  .count {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .take {
    border-color: color-mix(in srgb, var(--c) 55%, var(--line));
    background: color-mix(in srgb, var(--c) 12%, var(--raised));
  }
  .take:hover:not(:disabled) {
    background: color-mix(in srgb, var(--c) 24%, var(--raised));
    border-color: var(--c);
  }
  .take.ours {
    --c: var(--l0);
  }
  .take.theirs {
    --c: var(--l1);
  }
  .done {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--ok);
  }
  .base-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    color: var(--muted);
    cursor: pointer;
  }
  .base-toggle input {
    margin: 0;
    accent-color: var(--accent);
  }
</style>
