<script lang="ts">
  import type { Diff } from './types';

  let {
    title,
    subtitle = '',
    diff,
    loading = false,
    error = '',
    onclose,
  }: {
    title: string;
    subtitle?: string;
    diff: Diff | null;
    loading?: boolean;
    error?: string;
    onclose: () => void;
  } = $props();

  const MAX_LINES = 5000;

  interface Line {
    type: 'add' | 'del' | 'ctx' | 'hunk' | 'meta';
    old: number | null;
    new: number | null;
    text: string;
  }

  function parse(text: string): { lines: Line[]; cut: boolean; added: number; removed: number } {
    const lines: Line[] = [];
    let oldNo = 0;
    let newNo = 0;
    let inHunk = false;
    let added = 0;
    let removed = 0;
    const raw = text.split('\n');
    if (raw.at(-1) === '') raw.pop();
    for (const line of raw) {
      if (lines.length >= MAX_LINES) return { lines, cut: true, added, removed };
      const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
      if (hunk) {
        oldNo = Number(hunk[1]);
        newNo = Number(hunk[2]);
        inHunk = true;
        lines.push({ type: 'hunk', old: null, new: null, text: line });
      } else if (!inHunk || line.startsWith('diff --git ')) {
        // En-tête de fichier : chemins, modes, index.
        inHunk = false;
        lines.push({ type: 'meta', old: null, new: null, text: line });
      } else if (line.startsWith('+')) {
        added++;
        lines.push({ type: 'add', old: null, new: newNo++, text: line.slice(1) });
      } else if (line.startsWith('-')) {
        removed++;
        lines.push({ type: 'del', old: oldNo++, new: null, text: line.slice(1) });
      } else if (line.startsWith('\\')) {
        // « \ No newline at end of file »
        lines.push({ type: 'meta', old: null, new: null, text: line });
      } else {
        lines.push({ type: 'ctx', old: oldNo++, new: newNo++, text: line.slice(1) });
      }
    }
    return { lines, cut: false, added, removed };
  }

  const parsed = $derived(diff ? parse(diff.text) : null);
  const SIGN = { add: '+', del: '−', ctx: '', hunk: '', meta: '' } as const;
</script>

<section class="diff" aria-label="Diff de {title}">
  <header>
    <div class="names">
      <b class="mono">{title}</b>
      {#if subtitle}<small>{subtitle}</small>{/if}
    </div>
    {#if parsed && (parsed.added || parsed.removed)}
      <span class="count mono"><span class="plus">+{parsed.added}</span> <span class="minus">−{parsed.removed}</span></span>
    {/if}
    <button class="btn" type="button" onclick={onclose} title="Fermer le diff (Échap)">Fermer</button>
  </header>

  {#if error}
    <p class="note err">{error}</p>
  {:else if loading && !parsed}
    <p class="note">Chargement du diff…</p>
  {:else if parsed && parsed.lines.length === 0}
    <p class="note">Aucune différence de contenu à afficher (fichier vide, ou seul le mode a changé).</p>
  {:else if parsed}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="lines mono" tabindex="0">
      <table>
        <tbody>
          {#each parsed.lines as line, i (i)}
            <tr class="t-{line.type}">
              <td class="no">{line.old ?? ''}</td>
              <td class="no">{line.new ?? ''}</td>
              <td class="sign">{SIGN[line.type]}</td>
              <td class="code">{line.text}</td>
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
    gap: 12px;
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
  .names small {
    color: var(--muted);
    font-size: 11px;
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
</style>
