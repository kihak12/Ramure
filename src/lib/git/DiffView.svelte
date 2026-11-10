<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import { fly } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import type { Diff } from './types';

  let {
    title,
    subtitle = '',
    diff,
    loading = false,
    error = '',
    selectable = null,
    busy = false,
    onapply,
    onclose,
  }: {
    title: string;
    subtitle?: string;
    diff: Diff | null;
    loading?: boolean;
    error?: string;
    /** Indexation partielle : les lignes modifiées se sélectionnent pour être indexées ou retirées. */
    selectable?: 'stage' | 'unstage' | null;
    busy?: boolean;
    /** Reçoit les indices des lignes choisies, dans `diff.text.split('\n')`. */
    onapply?: (lines: number[]) => void;
    onclose: () => void;
  } = $props();

  const MAX_LINES = 5000;

  interface Line {
    /** Position dans le texte du diff : c'est elle que le cœur Rust attend. */
    index: number;
    type: 'add' | 'del' | 'ctx' | 'hunk' | 'meta';
    old: number | null;
    new: number | null;
    text: string;
    /** Numéro du bloc auquel la ligne appartient. */
    hunk: number;
  }

  function parse(text: string): { lines: Line[]; cut: boolean; added: number; removed: number } {
    const lines: Line[] = [];
    let oldNo = 0;
    let newNo = 0;
    let inHunk = false;
    let hunk = -1;
    let added = 0;
    let removed = 0;
    const raw = text.split('\n');
    if (raw.at(-1) === '') raw.pop();
    for (const [index, line] of raw.entries()) {
      if (lines.length >= MAX_LINES) return { lines, cut: true, added, removed };
      const header = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
      if (header) {
        oldNo = Number(header[1]);
        newNo = Number(header[2]);
        inHunk = true;
        hunk++;
        lines.push({ index, type: 'hunk', old: null, new: null, text: line, hunk });
      } else if (!inHunk || line.startsWith('diff --git ')) {
        // En-tête de fichier : chemins, modes, index.
        inHunk = false;
        lines.push({ index, type: 'meta', old: null, new: null, text: line, hunk });
      } else if (line.startsWith('+')) {
        added++;
        lines.push({ index, type: 'add', old: null, new: newNo++, text: line.slice(1), hunk });
      } else if (line.startsWith('-')) {
        removed++;
        lines.push({ index, type: 'del', old: oldNo++, new: null, text: line.slice(1), hunk });
      } else if (line.startsWith('\\')) {
        // « \ No newline at end of file »
        lines.push({ index, type: 'meta', old: null, new: null, text: line, hunk });
      } else {
        lines.push({ index, type: 'ctx', old: oldNo++, new: newNo++, text: line.slice(1), hunk });
      }
    }
    return { lines, cut: false, added, removed };
  }

  const parsed = $derived(diff ? parse(diff.text) : null);
  // Un diff coupé ne permet pas de construire un patch fiable.
  const canSelect = $derived(!!selectable && !!parsed && !parsed.cut && !diff?.truncated);
  const changes = $derived(parsed?.lines.filter((l) => l.type === 'add' || l.type === 'del') ?? []);
  const verb = $derived(selectable === 'unstage' ? 'Retirer' : 'Indexer');
  const SIGN = { add: '+', del: '−', ctx: '', hunk: '', meta: '' } as const;

  const selected = new SvelteSet<number>();
  let anchor = -1;

  // Un nouveau diff remet la sélection à zéro.
  $effect(() => {
    void diff;
    selected.clear();
    anchor = -1;
  });

  function toggle(line: Line, event: MouseEvent) {
    if (!canSelect || (line.type !== 'add' && line.type !== 'del')) return;
    if (event.shiftKey && anchor >= 0) {
      // Majuscule + clic : toute la plage depuis la dernière ligne cliquée.
      const [from, to] = [Math.min(anchor, line.index), Math.max(anchor, line.index)];
      for (const change of changes) if (change.index >= from && change.index <= to) selected.add(change.index);
    } else if (selected.has(line.index)) {
      selected.delete(line.index);
    } else {
      selected.add(line.index);
    }
    anchor = line.index;
  }

  const applySelection = () => onapply?.([...selected]);
  const applyHunk = (hunk: number) => onapply?.(changes.filter((l) => l.hunk === hunk).map((l) => l.index));
</script>

<section class="diff" aria-label="Diff de {title}" in:fly={{ y: 14, duration: 200 }} out:fly={{ y: 10, duration: 120 }}>
  <header>
    <div class="names">
      <b class="mono">{title}</b>
      {#if subtitle}<small>{subtitle}</small>{/if}
    </div>
    {#if canSelect && selected.size}
      <button class="btn small" type="button" onclick={() => selected.clear()}>Désélectionner</button>
      <button class="btn primary small" type="button" disabled={busy} onclick={applySelection}>
        {verb} {selected.size} ligne{selected.size > 1 ? 's' : ''}
      </button>
    {:else if canSelect}
      <small class="hint">Clique des lignes pour n'en {selectable === 'unstage' ? 'retirer' : 'indexer'} qu'une partie</small>
    {/if}
    {#if parsed && (parsed.added || parsed.removed)}
      <span class="count mono"><span class="plus">+{parsed.added}</span> <span class="minus">−{parsed.removed}</span></span>
    {/if}
    <button class="btn" type="button" onclick={onclose} title="Fermer le diff (Échap)"><Icon name="x" size={14} /> Fermer</button>
  </header>

  {#if error}
    <p class="note err">{error}</p>
  {:else if loading && !parsed}
    <p class="note">Chargement du diff…</p>
  {:else if parsed && parsed.lines.length === 0}
    <p class="note">Aucune différence de contenu à afficher (fichier vide, ou seul le mode a changé).</p>
  {:else if parsed}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="lines mono" class:selectable={canSelect} tabindex="0">
      <table>
        <tbody>
          {#each parsed.lines as line (line.index)}
            {@const pickable = canSelect && (line.type === 'add' || line.type === 'del')}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <tr
              class="t-{line.type}"
              class:pickable
              class:picked={selected.has(line.index)}
              aria-selected={pickable ? selected.has(line.index) : undefined}
              onclick={(e) => toggle(line, e)}
            >
              <td class="no">{line.old ?? ''}</td>
              <td class="no">{line.new ?? ''}</td>
              <td class="sign">{#if selected.has(line.index)}✓{:else}{SIGN[line.type]}{/if}</td>
              <td class="code">
                {line.text}{#if canSelect && line.type === 'hunk'}
                  <button class="btn small hunk-btn" type="button" disabled={busy} onclick={() => applyHunk(line.hunk)}>{verb} ce bloc</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if parsed.cut || diff?.truncated}
        <p class="note">Diff trop long : seul le début est affiché.</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .diff {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: var(--bg);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px 6px 14px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    min-width: 0;
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
  .names b {
    font-size: 12.5px;
  }
  .names small,
  .hint {
    color: var(--muted);
    font-size: 11px;
  }
  .hint {
    white-space: nowrap;
  }
  .count {
    font-size: 12px;
    white-space: nowrap;
  }
  .plus {
    color: var(--ok);
  }
  .minus {
    color: var(--bad);
  }
  .note {
    margin: 0;
    padding: 18px;
    color: var(--muted);
  }
  .note.err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .lines {
    overflow: auto;
    font-size: 12px;
    line-height: 18px;
    min-height: 0;
  }
  .lines:focus-visible {
    outline-offset: -2px;
  }
  table {
    border-collapse: collapse;
    min-width: 100%;
  }
  td {
    padding: 0;
    white-space: pre;
    vertical-align: top;
  }
  .no {
    width: 1%;
    min-width: 44px;
    padding: 0 8px;
    text-align: right;
    color: var(--muted);
    user-select: none;
    opacity: 0.75;
  }
  .sign {
    width: 16px;
    text-align: center;
    user-select: none;
  }
  .code {
    padding-right: 16px;
  }
  .t-add {
    background: color-mix(in srgb, var(--ok) 14%, transparent);
  }
  .t-add .sign {
    color: var(--ok);
  }
  .t-del {
    background: color-mix(in srgb, var(--bad) 13%, transparent);
  }
  .t-del .sign {
    color: var(--bad);
  }
  .t-hunk {
    background: var(--sel);
    color: var(--muted);
  }
  .t-meta {
    color: var(--muted);
  }
  .t-hunk .code,
  .t-meta .code {
    padding-left: 4px;
  }
  /* Sélection de lignes */
  .pickable {
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .pickable.t-add:hover {
    background: color-mix(in srgb, var(--ok) 24%, transparent);
  }
  .pickable.t-del:hover {
    background: color-mix(in srgb, var(--bad) 22%, transparent);
  }
  .picked.t-add,
  .picked.t-add:hover {
    background: color-mix(in srgb, var(--ok) 38%, transparent);
    box-shadow: inset 3px 0 0 var(--ok);
  }
  .picked.t-del,
  .picked.t-del:hover {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
    box-shadow: inset 3px 0 0 var(--bad);
  }
  .picked .sign {
    font-weight: 700;
  }
  .hunk-btn {
    margin-left: 12px;
    padding: 0 8px;
    line-height: 16px;
    font-family: var(--f-ui);
    font-size: 11px;
    vertical-align: top;
  }
</style>
