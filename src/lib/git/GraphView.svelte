<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount, tick } from 'svelte';
  import type { LogResult } from './types';

  const ROW = 28; // hauteur d'une ligne
  const LANE = 16; // largeur d'une lane
  const PAD = 14; // marge gauche du graphe
  const BUFFER = 24; // lignes rendues au-dessus et au-dessous de la zone visible
  const STEP = 12; // la fenêtre de rendu ne se déplace que par paliers de 12 lignes
  const MAX_GRAPH = 340; // largeur maximale de la colonne du graphe
  const STORAGE_KEY = 'ramure.repo';

  // `$state.raw` : 50 000 lignes ne doivent pas être enveloppées dans des proxys réactifs.
  let result = $state.raw<LogResult | null>(null);
  let limit = $state(10000);
  let loading = $state(false);
  let error = $state('');
  let scrollTop = $state(0);
  let viewH = $state(600);
  let selected = $state(-1);
  let showJournal = $state(false);
  let perf = $state({ ipc: 0, paint: 0 });
  let frames = $state<{ avg: number; worst: number } | null>(null);
  let dark = $state(false);
  let copied = $state(false);

  let scroller = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();

  const commits = $derived(result?.commits ?? []);
  const graphW = $derived(
    Math.min(MAX_GRAPH, PAD * 2 + Math.max(0, (result?.maxLanes ?? 1) - 1) * LANE + 6),
  );
  // Fenêtre de rendu : seules ces lignes existent dans le DOM et sur le canvas.
  const winStart = $derived(Math.max(0, Math.floor((scrollTop / ROW - BUFFER) / STEP) * STEP));
  const winEnd = $derived(
    Math.min(commits.length, winStart + Math.ceil(viewH / ROW) + 2 * BUFFER + STEP),
  );
  const rows = $derived(commits.slice(winStart, winEnd));
  const current = $derived(selected >= 0 ? (commits[selected] ?? null) : null);
  const lastCommand = $derived(result?.commands.at(-1) ?? null);

  const dateFmt = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' });
  const fullDateFmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' });
  const numFmt = new Intl.NumberFormat('fr-FR');
  const ms = (n: number) => `${numFmt.format(Math.round(n))} ms`;

  async function load(path: string) {
    if (!path || loading) return;
    loading = true;
    error = '';
    const t0 = performance.now();
    try {
      const data = await invoke<LogResult>('git_log', { repo: path, limit: limit > 0 ? limit : null });
      const t1 = performance.now();
      result = data;
      selected = -1;
      frames = null;
      scrollTop = 0;
      if (scroller) scroller.scrollTop = 0;
      try {
        localStorage.setItem(STORAGE_KEY, data.repo);
      } catch {
        /* stockage indisponible : on se passe de la mémorisation */
      }
      await tick();
      // Deux images plus tard, le graphe est réellement à l'écran.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => (perf = { ipc: t1 - t0, paint: performance.now() - t0 })),
      );
    } catch (e) {
      error = typeof e === 'string' ? e : String(e);
      result = null;
    } finally {
      loading = false;
    }
  }

  async function pick() {
    const dir = await open({ directory: true, multiple: false, title: 'Ouvrir un dépôt Git' });
    if (typeof dir === 'string') await load(dir);
  }

  function reload() {
    if (result) void load(result.repo);
  }

  // --- Dessin du graphe -------------------------------------------------------------------

  function draw() {
    if (!canvas) return;
    const css = getComputedStyle(document.documentElement);
    const palette = Array.from({ length: 8 }, (_, i) => css.getPropertyValue(`--l${i}`).trim());
    const background = css.getPropertyValue('--bg').trim();
    const color = (lane: number) => palette[lane % palette.length];

    const dpr = window.devicePixelRatio || 1;
    const width = graphW;
    const height = Math.max(1, (winEnd - winStart) * ROW);
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.lineWidth = 2;
    ctx.lineCap = 'round';

    const x = (lane: number) => PAD + lane * LANE;
    const y = (index: number) => (index - winStart) * ROW + ROW / 2;

    // Segments : de la ligne i vers la ligne i + 1. On part d'une ligne plus haut pour
    // tracer aussi ce qui entre dans la fenêtre par le dessus.
    for (let i = Math.max(0, winStart - 1); i < winEnd; i++) {
      const commit = commits[i];
      const y1 = y(i);
      const y2 = y1 + ROW;
      for (const [from, to, lane] of commit.edges) {
        ctx.strokeStyle = color(lane);
        ctx.setLineDash(commit.wip && lane === commit.lane ? [3, 3] : []);
        ctx.beginPath();
        ctx.moveTo(x(from), y1);
        if (from === to) ctx.lineTo(x(to), y2);
        else ctx.bezierCurveTo(x(from), y1 + ROW * 0.6, x(to), y2 - ROW * 0.6, x(to), y2);
        ctx.stroke();
      }
    }
    ctx.setLineDash([]);

    // Nœuds, par-dessus les segments.
    for (let i = winStart; i < winEnd; i++) {
      const commit = commits[i];
      const cx = x(commit.lane);
      const cy = y(i);
      const isHead = commit.refs.some((r) => r.kind === 'head' || r.kind === 'detached');
      if (isHead) {
        ctx.beginPath();
        ctx.arc(cx, cy, 8, 0, Math.PI * 2);
        ctx.strokeStyle = color(commit.lane);
        ctx.globalAlpha = 0.5;
        ctx.lineWidth = 1.5;
        ctx.stroke();
        ctx.globalAlpha = 1;
      }
      ctx.beginPath();
      ctx.arc(cx, cy, commit.parents.length > 1 ? 4 : 5, 0, Math.PI * 2);
      ctx.lineWidth = 2;
      ctx.strokeStyle = commit.wip ? color(commit.lane) : background;
      ctx.fillStyle = commit.wip ? background : color(commit.lane);
      if (commit.wip) ctx.setLineDash([2.5, 2]);
      ctx.fill();
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }

  $effect(() => {
    // Redessine quand la fenêtre, les données, la largeur ou le thème changent.
    void dark;
    draw();
  });

  // --- Défilement et mesure de fluidité ---------------------------------------------------

  let raf = 0;
  let lastFrame = 0;
  let lastScroll = 0;
  let samples: number[] = [];

  function measure(now: number) {
    samples.push(now - lastFrame);
    lastFrame = now;
    if (now - lastScroll < 250) {
      raf = requestAnimationFrame(measure);
      return;
    }
    raf = 0;
    // La dernière mesure couvre la pause de fin de défilement : on l'écarte.
    samples.pop();
    if (samples.length >= 10) {
      const sorted = [...samples].sort((a, b) => a - b);
      frames = {
        avg: samples.reduce((a, b) => a + b, 0) / samples.length,
        worst: sorted[Math.floor(sorted.length * 0.99)],
      };
    }
    samples = [];
  }

  function onScroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    lastScroll = performance.now();
    if (!raf) {
      lastFrame = lastScroll;
      raf = requestAnimationFrame(measure);
    }
  }

  function reveal(index: number) {
    if (!scroller) return;
    const top = index * ROW;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW > scroller.scrollTop + viewH) scroller.scrollTop = top + ROW - viewH;
  }

  function onKey(e: KeyboardEvent) {
    if (!commits.length) return;
    const page = Math.max(1, Math.floor(viewH / ROW) - 1);
    const moves: Record<string, number> = {
      ArrowDown: selected + 1,
      ArrowUp: selected - 1,
      PageDown: selected + page,
      PageUp: selected - page,
      Home: 0,
      End: commits.length - 1,
    };
    if (!(e.key in moves)) return;
    e.preventDefault();
    selected = Math.max(0, Math.min(commits.length - 1, moves[e.key]));
    reveal(selected);
  }

  async function copySha(sha: string) {
    try {
      await navigator.clipboard.writeText(sha);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* presse-papiers refusé : le SHA reste sélectionnable dans le panneau */
    }
  }

  onMount(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    dark = media.matches;
    const onTheme = () => (dark = media.matches);
    media.addEventListener('change', onTheme);

    const observer = new ResizeObserver(() => {
      if (scroller && scroller.clientHeight > 0) viewH = scroller.clientHeight;
    });
    if (scroller) observer.observe(scroller);

    let saved = '';
    try {
      saved = localStorage.getItem(STORAGE_KEY) ?? '';
    } catch {
      /* stockage indisponible */
    }
    if (saved) void load(saved);

    return () => {
      media.removeEventListener('change', onTheme);
      observer.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  });
</script>

<div class="view">
  <header class="toolbar">
    <div class="repo">
      <b>{result?.name ?? 'Aucun dépôt'}</b>
      <small class="mono">{result ? (result.branch ?? 'HEAD détaché') : 'ouvre un dossier pour commencer'}</small>
    </div>
    <button class="btn primary" type="button" onclick={pick} disabled={loading}>Ouvrir un dépôt…</button>
    <button class="btn" type="button" onclick={reload} disabled={!result || loading}>Recharger</button>
    <label class="limit">
      <span class="label">Commits chargés</span>
      <select class="field" bind:value={limit} onchange={reload} disabled={loading}>
        <option value={1000}>1 000</option>
        <option value={10000}>10 000</option>
        <option value={50000}>50 000</option>
        <option value={0}>Tous</option>
      </select>
    </label>
    <div class="stats" aria-live="polite">
      {#if loading}
        <span>Chargement…</span>
      {:else if result}
        <span><b>{numFmt.format(commits.length - (result.wipFiles > 0 ? 1 : 0))}</b> commits{result.truncated ? ' (historique tronqué)' : ''}</span>
        <span>git {ms(result.timings.gitMs)}</span>
        <span>lanes {ms(result.timings.layoutMs)}</span>
        <span>affiché en <b>{ms(perf.paint)}</b></span>
        {#if frames}
          <span title="Temps moyen entre deux images pendant le dernier défilement, et pire image (99e centile). 16,7 ms = 60 images par seconde.">
            défilement <b>{frames.avg.toFixed(1)} ms</b>/image, pire {frames.worst.toFixed(0)} ms
          </span>
        {/if}
      {/if}
    </div>
  </header>

  <div class="body">
    <div class="graph">
      <div class="head" style:padding-left="{graphW}px">
        <span>Message</span><span>Auteur</span><span>Date</span><span>SHA</span>
      </div>

      {#if error}
        <div class="empty">
          <p class="err">{error}</p>
          <button class="btn primary" type="button" onclick={pick}>Choisir un autre dossier…</button>
        </div>
      {:else if !result && !loading}
        <div class="empty">
          <p>Ouvre un dépôt Git pour afficher son graphe.</p>
          <button class="btn primary" type="button" onclick={pick}>Ouvrir un dépôt…</button>
        </div>
      {:else if result && commits.length === 0}
        <div class="empty"><p>Ce dépôt ne contient encore aucun commit.</p></div>
      {/if}

      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="scroller"
        bind:this={scroller}
        onscroll={onScroll}
        onkeydown={onKey}
        tabindex="0"
        role="listbox"
        aria-label="Historique des commits"
        aria-activedescendant={current ? `commit-${selected}` : undefined}
      >
        <div class="spacer" style:height="{commits.length * ROW}px">
          <canvas
            bind:this={canvas}
            class="lanes"
            style:top="{winStart * ROW}px"
            style:width="{graphW}px"
            style:height="{(winEnd - winStart) * ROW}px"
            aria-hidden="true"
          ></canvas>
          {#each rows as commit, i (commit.sha)}
            {@const index = winStart + i}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="row"
              class:sel={index === selected}
              id="commit-{index}"
              role="option"
              tabindex="-1"
              aria-selected={index === selected}
              style:top="{index * ROW}px"
              style:padding-left="{graphW}px"
              onclick={() => (selected = index)}
            >
              <span class="msg">
                {#if commit.wip}
                  <span class="ref k-wip">// WIP</span>
                  <span class="subject muted">{result?.wipFiles} fichier{(result?.wipFiles ?? 0) > 1 ? 's' : ''} modifié{(result?.wipFiles ?? 0) > 1 ? 's' : ''}</span>
                {:else}
                  {#each commit.refs as ref (ref.kind + ref.name)}
                    <span class="ref k-{ref.kind}" style:--c="var(--l{commit.lane % 8})">{ref.kind === 'head' ? '✓ ' : ''}{ref.name}</span>
                  {/each}
                  <span class="subject">{commit.subject}</span>
                {/if}
              </span>
              <span class="author">{commit.author}</span>
              <span class="date">{commit.wip ? '' : dateFmt.format(commit.time * 1000)}</span>
              <span class="sha mono">{commit.wip ? '' : commit.sha.slice(0, 7)}</span>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <aside class="detail">
      {#if current?.wip}
        <span class="label">Modifications en cours</span>
        <p class="title">{result?.wipFiles} fichier{(result?.wipFiles ?? 0) > 1 ? 's' : ''} modifié{(result?.wipFiles ?? 0) > 1 ? 's' : ''} sur {result?.branch ?? 'HEAD'}</p>
        <p class="muted">L'index, le diff et le commit arrivent au jalon J1.</p>
      {:else if current}
        <span class="label">Commit</span>
        <p class="title">{current.subject}</p>
        <dl>
          <dt>SHA</dt>
          <dd class="mono sha-full">{current.sha}</dd>
          <dt>Auteur</dt>
          <dd>{current.author}</dd>
          <dt>Date</dt>
          <dd>{fullDateFmt.format(current.time * 1000)}</dd>
          <dt>Parent{current.parents.length > 1 ? 's' : ''}</dt>
          <dd class="mono">{current.parents.length ? current.parents.map((p) => p.slice(0, 7)).join(', ') : 'aucun (premier commit)'}</dd>
          {#if current.refs.length}
            <dt>Références</dt>
            <dd>{current.refs.map((r) => r.name).join(', ')}</dd>
          {/if}
        </dl>
        <button class="btn" type="button" onclick={() => copySha(current.sha)}>{copied ? 'SHA copié' : 'Copier le SHA'}</button>
        <p class="muted">Fichiers modifiés et diff : jalon J1.</p>
      {:else}
        <span class="label">Détail</span>
        <p class="muted">Clique un commit, ou utilise les flèches du clavier dans la liste.</p>
      {/if}
    </aside>
  </div>

  <footer class="status">
    <span class="mono branch">{result?.branch ? `⑂ ${result.branch}` : ''}</span>
    <button class="journal-btn mono" type="button" aria-expanded={showJournal} onclick={() => (showJournal = !showJournal)} disabled={!result}>
      {lastCommand ? `$ ${lastCommand.command}` : 'Journal des commandes'}
    </button>
    {#if showJournal && result}
      <div class="journal">
        <span class="label">Journal des commandes</span>
        {#each result.commands as entry, i (i)}
          <div class="entry mono" class:failed={!entry.ok}>
            <span>{ms(entry.ms)}</span><span>$ {entry.command}</span>
          </div>
        {/each}
      </div>
    {/if}
  </footer>
</div>

<style>
  .view {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) 26px;
    height: 100%;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    min-width: 0;
  }
  .repo {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
    padding-right: 12px;
    border-right: 1px solid var(--line);
    min-width: 120px;
    max-width: 240px;
  }
  .repo b,
  .repo small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .repo small {
    color: var(--muted);
    font-size: 11px;
  }
  .limit {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .limit select {
    width: auto;
  }
  .stats {
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 2px 14px;
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    min-width: 0;
  }
  .stats b {
    color: var(--ink);
    font-weight: 600;
  }

  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    min-height: 0;
  }
  .graph {
    display: grid;
    grid-template-rows: 26px minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    position: relative;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 150px 96px 72px;
    align-items: center;
    column-gap: 10px;
  }
  .head {
    border-bottom: 1px solid var(--line);
    font-size: 10.5px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--muted);
    overflow: hidden;
    scrollbar-gutter: stable;
    padding-right: 10px;
  }
  .scroller {
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    min-height: 0;
  }
  .scroller:focus-visible {
    outline-offset: -2px;
  }
  .spacer {
    position: relative;
  }
  .lanes {
    position: absolute;
    left: 0;
    pointer-events: none;
    z-index: 1;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 28px;
    padding-right: 10px;
    cursor: pointer;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--sel);
  }
  .msg {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    overflow: hidden;
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .author,
  .date {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sha {
    color: var(--muted);
    font-size: 11.5px;
  }
  .muted {
    color: var(--muted);
  }
  .ref {
    flex: none;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 11px;
    line-height: 16px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--c, var(--line));
    background: color-mix(in srgb, var(--c, var(--line)) 15%, transparent);
    font-weight: 500;
  }
  /* Préfixe `k-` : la branche courante a le type `head`, qui ne doit pas hériter de l'en-tête `.head`. */
  .ref.k-head,
  .ref.k-detached {
    font-weight: 700;
  }
  .ref.k-remote {
    border-style: dashed;
    font-weight: 400;
  }
  .ref.k-tag {
    --c: var(--muted);
    border-radius: 9px;
    font-weight: 400;
  }
  .ref.k-wip {
    --c: var(--muted);
    border-style: dashed;
    font-family: var(--f-mono);
    font-weight: 400;
  }

  .empty {
    position: absolute;
    inset: 26px 0 0 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    text-align: center;
    padding: 24px;
    z-index: 2;
    background: var(--bg);
  }
  .empty p {
    margin: 0;
    max-width: 52ch;
    color: var(--muted);
    font-size: 14px;
  }
  .empty .err {
    color: var(--bad);
  }

  .detail {
    border-left: 1px solid var(--line);
    background: var(--panel);
    padding: 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    min-height: 0;
  }
  .detail p {
    margin: 0;
  }
  .title {
    font-size: 14px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0;
    width: 100%;
  }
  dt {
    color: var(--muted);
    font-size: 12px;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .sha-full {
    font-size: 11.5px;
    user-select: all;
  }

  .status {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 10px;
    border-top: 1px solid var(--line);
    background: var(--side);
    color: var(--muted);
    font-size: 11.5px;
    min-width: 0;
  }
  .branch {
    color: var(--ink);
    white-space: nowrap;
  }
  .journal-btn {
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .journal-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
  }
  .journal {
    position: absolute;
    left: 10px;
    bottom: 30px;
    width: min(760px, calc(100vw - 80px));
    max-height: 260px;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 9px;
    padding: 10px 12px;
    display: grid;
    gap: 4px;
    z-index: 5;
    box-shadow: 0 14px 30px -12px rgb(0 0 0 / 0.35);
  }
  .entry {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    gap: 10px;
    font-size: 11.5px;
    color: var(--ink);
  }
  .entry span:first-child {
    color: var(--muted);
    text-align: right;
  }
  .entry span:last-child {
    overflow-wrap: anywhere;
  }
  .entry.failed span:last-child {
    color: var(--muted);
  }
</style>
