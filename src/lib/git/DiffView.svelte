<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import { fly } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import { prefs, savePrefs, type DiffMode } from './prefs.svelte';
  import type { Diff } from './types';
  import { motion } from '../ui/motion';

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

  // --- Affichage côte à côte ----------------------------------------------------------------

  /** Une ligne du tableau à deux colonnes : ancien à gauche, nouveau à droite. */
  interface Pair {
    key: number;
    /** En-tête de fichier ou de bloc, sur toute la largeur. */
    full: Line | null;
    left: Line | null;
    right: Line | null;
    /** Partie réellement modifiée de chaque côté, quand les deux lignes se ressemblent. */
    inline: { pre: string; old: string; new: string; post: string } | null;
  }

  /** Ce qui change entre deux lignes : leur début et leur fin communs encadrent la différence. */
  function inlineChange(before: string, after: string): Pair['inline'] {
    let pre = 0;
    const max = Math.min(before.length, after.length);
    while (pre < max && before[pre] === after[pre]) pre++;
    let post = 0;
    while (post < max - pre && before[before.length - 1 - post] === after[after.length - 1 - post]) post++;
    // Trop peu en commun : surligner presque toute la ligne n'apprendrait rien.
    if (pre + post < Math.min(before.length, after.length) * 0.3) return null;
    return {
      pre: before.slice(0, pre),
      old: before.slice(pre, before.length - post),
      new: after.slice(pre, after.length - post),
      post: before.slice(before.length - post),
    };
  }

  /** Aligne suppressions et ajouts d'un même changement sur les mêmes lignes du tableau. */
  function pairUp(lines: Line[]): Pair[] {
    const out: Pair[] = [];
    let removed: Line[] = [];
    let added: Line[] = [];
    const flush = () => {
      const count = Math.max(removed.length, added.length);
      for (let k = 0; k < count; k++) {
        const left = removed[k] ?? null;
        const right = added[k] ?? null;
        out.push({
          key: (left ?? right)!.index,
          full: null,
          left,
          right,
          inline: left && right ? inlineChange(left.text, right.text) : null,
        });
      }
      removed = [];
      added = [];
    };
    for (const line of lines) {
      if (line.type === 'del') {
        // Une suppression après des ajouts ouvre un nouveau changement.
        if (added.length) flush();
        removed.push(line);
      } else if (line.type === 'add') {
        added.push(line);
      } else {
        flush();
        if (line.type === 'ctx') out.push({ key: line.index, full: null, left: line, right: line, inline: null });
        else out.push({ key: line.index, full: line, left: null, right: null, inline: null });
      }
    }
    flush();
    return out;
  }

  const split = $derived(prefs.diffMode === 'split');
  const allPairs = $derived(parsed ? pairUp(parsed.lines) : []);
  const pairs = $derived(split ? allPairs : []);

  // Une ligne modifiée, c'est une suppression et un ajout qui vont ensemble. N'en choisir qu'une
  // moitié laisse l'ancienne version à côté de la nouvelle, ou retire la ligne sans la remplacer.
  // Ne comptent que les paires dont les deux lignes se ressemblent : les autres sont deux
  // changements distincts que le hasard a placés côte à côte.
  const halves = $derived.by(() => {
    const missing: number[] = [];
    if (!canSelect || !selected.size) return missing;
    for (const pair of allPairs) {
      if (!pair.inline || !pair.left || !pair.right) continue;
      const old = selected.has(pair.left.index);
      if (old !== selected.has(pair.right.index)) missing.push(old ? pair.right.index : pair.left.index);
    }
    return missing;
  });
  const pickHalves = () => {
    for (const index of halves) selected.add(index);
  };
  const setMode = (mode: DiffMode) => savePrefs({ ...prefs, diffMode: mode });

  const applySelection = () => onapply?.([...selected]);
  const applyHunk = (hunk: number) => onapply?.(changes.filter((l) => l.hunk === hunk).map((l) => l.index));
</script>

<section class="diff" aria-label="Diff de {title}" in:fly={{ y: 14, duration: motion.med }} out:fly={{ y: 10, duration: motion.fast }}>
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
      <small class="hint">Clique des lignes pour n’en {selectable === 'unstage' ? 'retirer' : 'indexer'} qu’une partie</small>
    {/if}
    {#if parsed && (parsed.added || parsed.removed)}
      <span class="count mono"><span class="plus">+{parsed.added}</span> <span class="minus">−{parsed.removed}</span></span>
    {/if}
    <div class="modes" role="group" aria-label="Disposition du diff">
      <button type="button" aria-pressed={!split} onclick={() => setMode('unified')} title="Une seule colonne">Unifié</button>
      <button type="button" aria-pressed={split} onclick={() => setMode('split')} title="Ancien à gauche, nouveau à droite">Côte à côte</button>
    </div>
    <button class="btn" type="button" onclick={onclose} title="Fermer le diff (Échap)"><Icon name="x" size={14} /> Fermer</button>
  </header>

  {#if halves.length}
    <p class="lonely" role="status">
      <Icon name="warning" size={14} />
      <span>
        {halves.length > 1 ? `${halves.length} lignes modifiées ne sont choisies` : 'Une ligne modifiée n’est choisie'} qu’à moitié : l’ancienne
        version et la nouvelle ne seront pas {selectable === 'unstage' ? 'retirées' : 'indexées'} ensemble.
      </span>
      <button class="btn small" type="button" onclick={pickHalves}>Choisir l’autre moitié</button>
    </p>
  {/if}

  {#if error}
    <p class="note err">{error}</p>
  {:else if loading && !parsed}
    <p class="note">Chargement du diff…</p>
  {:else if parsed && parsed.lines.length === 0}
    <p class="note">Aucune différence de contenu à afficher (fichier vide, ou seul le mode a changé).</p>
  {:else if parsed && split}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="lines mono" tabindex="0">
      <table class="split">
        <colgroup><col class="c-no" /><col class="c-sign" /><col /><col class="c-no" /><col class="c-sign" /><col /></colgroup>
        <tbody>
          {#each pairs as pair (pair.key)}
            {#if pair.full}
              <tr class="t-{pair.full.type}">
                <td class="code" colspan="6">
                  {pair.full.text}{#if canSelect && pair.full.type === 'hunk'}
                    <button class="btn small hunk-btn" type="button" disabled={busy} onclick={() => applyHunk(pair.full!.hunk)}>{verb} ce bloc</button>
                  {/if}
                </td>
              </tr>
            {:else}
              {@const left = pair.left}
              {@const right = pair.right}
              {@const leftPick = canSelect && left?.type === 'del'}
              {@const rightPick = canSelect && right?.type === 'add'}
              <tr>
                <td class="no">{left?.old ?? ''}</td>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <td
                  class="half"
                  colspan="2"
                  class:c-del={left?.type === 'del'}
                  class:c-void={!left}
                  class:pickable={leftPick}
                  class:picked={left ? selected.has(left.index) : false}
                  onclick={(e) => left && toggle(left, e)}
                >
                  <span class="sign">{#if left?.type === 'del'}−{/if}</span>{#if left}{#if pair.inline && left.type === 'del'}{pair.inline.pre}<mark>{pair.inline.old}</mark>{pair.inline.post}{:else}{left.text}{/if}{/if}
                </td>
                <td class="no">{right?.new ?? ''}</td>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <td
                  class="half"
                  colspan="2"
                  class:c-add={right?.type === 'add'}
                  class:c-void={!right}
                  class:pickable={rightPick}
                  class:picked={right ? selected.has(right.index) : false}
                  onclick={(e) => right && toggle(right, e)}
                >
                  <span class="sign">{#if right?.type === 'add'}+{/if}</span>{#if right}{#if pair.inline && right.type === 'add'}{pair.inline.pre}<mark>{pair.inline.new}</mark>{pair.inline.post}{:else}{right.text}{/if}{/if}
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
      {#if parsed.cut || diff?.truncated}
        <p class="note">Diff trop long : seul le début est affiché.</p>
      {/if}
    </div>
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
              <td class="sign">{SIGN[line.type]}</td>
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
  .lonely {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 12px 0;
    padding: 6px 8px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    font-size: var(--fs-small);
  }
  .lonely :global(svg) {
    flex: none;
    color: var(--warn);
  }
  .lonely span {
    flex: 1;
    min-width: 0;
  }
  .diff {
    position: absolute;
    inset: 0;
    z-index: 3;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: var(--panel);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px 6px 14px;
    border-bottom: 1px solid var(--line-soft);
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
    font-size: var(--fs-body);
  }
  .names small,
  .hint {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .hint {
    white-space: nowrap;
  }
  .count {
    font-size: var(--fs-label);
    white-space: nowrap;
  }
  /* Bascule entre les deux dispositions */
  .modes {
    display: flex;
    padding: 2px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
  }
  .modes button {
    padding: 2px 9px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    font-size: var(--fs-label);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .modes button:hover {
    color: var(--ink);
  }
  .modes button[aria-pressed='true'] {
    background: var(--raised);
    color: var(--ink);
    font-weight: var(--fw-strong);
    box-shadow: var(--shadow-soft);
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
    font-size: var(--fs-code);
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
  /* Côte à côte : deux moitiés de largeur égale, les longues lignes passent à la ligne. */
  .split {
    width: 100%;
    table-layout: fixed;
  }
  .split .c-no {
    width: 46px;
  }
  .split .c-sign {
    width: 18px;
  }
  .split .half {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    padding-right: 10px;
    border-right: 1px solid var(--line-soft);
  }
  .split .half .sign {
    display: inline-block;
    width: 18px;
    text-align: center;
  }
  .split .no {
    width: auto;
    min-width: 0;
    padding: 0 6px;
  }
  .c-del {
    background: color-mix(in srgb, var(--bad) 13%, transparent);
  }
  .c-del .sign {
    color: var(--bad);
  }
  .c-add {
    background: color-mix(in srgb, var(--ok) 14%, transparent);
  }
  .c-add .sign {
    color: var(--ok);
  }
  /* Côté sans ligne correspondante. */
  .c-void {
    background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--muted) 9%, transparent) 6px 12px);
  }
  mark {
    border-radius: var(--radius-xs);
    color: inherit;
  }
  .c-del mark {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
  }
  .c-add mark {
    background: color-mix(in srgb, var(--ok) 36%, transparent);
  }
  .c-del.pickable:hover {
    background: color-mix(in srgb, var(--bad) 22%, transparent);
  }
  .c-add.pickable:hover {
    background: color-mix(in srgb, var(--ok) 24%, transparent);
  }
  .c-del.picked,
  .c-del.picked:hover {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
  }
  .c-add.picked,
  .c-add.picked:hover {
    background: color-mix(in srgb, var(--ok) 38%, transparent);
  }

  /* Sélection de lignes : une ligne choisie se lit à sa teinte plus forte et à son signe en gras,
     sans trait de couleur sur le bord. */
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
  }
  .picked.t-del,
  .picked.t-del:hover {
    background: color-mix(in srgb, var(--bad) 34%, transparent);
  }
  .picked .sign {
    font-weight: var(--fw-strong);
  }
  .hunk-btn {
    margin-left: 12px;
    padding: 0 8px;
    line-height: 16px;
    font-family: var(--f-ui);
    font-size: var(--fs-small);
    vertical-align: top;
  }
</style>
