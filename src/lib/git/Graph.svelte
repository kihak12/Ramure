<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu } from '../ui/menu.svelte';
  import { commitMenu, dropMenu, labelMenu } from './actions';
  import { accepts, drag, isBranch } from './drag.svelte';
  import { bare, labelsOf, type Label } from './labels';
  import { prefs } from './prefs.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo }: { repo: Repo } = $props();

  // Deux rendus du graphe, au choix dans les réglages.
  // « Ruban » : pastilles d'auteur sur le graphe, tracés arrondis, bande de couleur qui relie
  // chaque commit à son message. « Classique » : points et courbes, plus compact.
  const STYLES = {
    ribbon: { row: 32, lane: 22, pad: 18, node: 9.5, merge: 4.5, line: 2.5, max: 400 },
    classic: { row: 28, lane: 16, pad: 14, node: 5, merge: 4, line: 2, max: 340 },
  } as const;

  const LABELS = 164; // largeur de la colonne des branches et des tags
  const BUFFER = 24; // lignes rendues au-dessus et au-dessous de la zone visible
  const STEP = 12; // la fenêtre de rendu ne se déplace que par paliers de 12 lignes
  const BAND = 22; // hauteur du ruban qui relie un commit à son message
  const BAND_ALPHA = 0.13; // même opacité que le dégradé CSS qui le prolonge dans la colonne du message

  let scrollTop = $state(0);
  let viewH = $state(600);
  let dark = $state(false);
  // Vrai juste après une ouverture : les lignes entrent en cascade.
  let fresh = $state(false);

  let scroller = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();

  const ribbon = $derived(prefs.graphStyle !== 'classic');
  const metrics = $derived(STYLES[ribbon ? 'ribbon' : 'classic']);
  const ROW = $derived(metrics.row);

  const commits = $derived(repo.commits);
  // Jamais plus étroit que son en-tête de colonne.
  const graphW = $derived(
    Math.max(72, Math.min(metrics.max, metrics.pad * 2 + Math.max(0, (repo.result?.maxLanes ?? 1) - 1) * metrics.lane + 6)),
  );
  // Fenêtre de rendu : seules ces lignes existent dans le DOM et sur le canvas.
  const winStart = $derived(Math.max(0, Math.floor((scrollTop / ROW - BUFFER) / STEP) * STEP));
  const winEnd = $derived(Math.min(commits.length, winStart + Math.ceil(viewH / ROW) + 2 * BUFFER + STEP));
  const rows = $derived(commits.slice(winStart, winEnd));
  // Ligne du commit courant, pour l'halo animé posé sur son nœud.
  const headIndex = $derived(repo.head ? commits.indexOf(repo.head) : -1);

  // Répartition des modifications en cours, pour la ligne WIP.
  const wip = $derived.by(() => {
    const status = repo.status;
    if (!status) return null;
    const kinds = new Map<string, string>();
    for (const file of [...status.unstaged, ...status.staged]) kinds.set(file.path, file.kind);
    const count = (...wanted: string[]) => [...kinds.values()].filter((k) => wanted.includes(k)).length;
    return {
      modified: count('modified', 'renamed', 'copied', 'typechange'),
      added: count('added', 'untracked'),
      deleted: count('deleted'),
      conflicts: count('conflict'),
    };
  });

  const dateFmt = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' });

  const initials = (author: string) =>
    author
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]!.toUpperCase())
      .join('');
  /** Couleur d'un auteur dans le rendu classique : toujours la même, prise dans la palette des lanes. */
  const hue = (author: string) => [...author].reduce((sum, ch) => sum + ch.charCodeAt(0), 0) % 8;

  // --- Dessin du graphe -------------------------------------------------------------------

  function draw() {
    if (!canvas) return;
    const css = getComputedStyle(document.documentElement);
    const palette = Array.from({ length: 8 }, (_, i) => css.getPropertyValue(`--l${i}`).trim());
    const background = css.getPropertyValue('--bg').trim();
    const onColor = css.getPropertyValue('--on-accent').trim();
    const family = getComputedStyle(document.body).fontFamily;
    const color = (lane: number) => palette[lane % palette.length];
    const { row, lane: laneW, pad, node, merge, line } = metrics;

    const dpr = window.devicePixelRatio || 1;
    const width = graphW;
    const height = Math.max(1, (winEnd - winStart) * row);
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';

    const x = (lane: number) => pad + lane * laneW;
    const y = (index: number) => (index - winStart) * row + row / 2;

    // Ruban : une bande de la couleur de la branche part du commit et file vers son message.
    if (ribbon) {
      ctx.globalAlpha = BAND_ALPHA;
      for (let i = winStart; i < winEnd; i++) {
        const commit = commits[i];
        const cx = x(commit.lane);
        if (cx >= width) continue;
        ctx.fillStyle = color(commit.lane);
        ctx.fillRect(cx, y(i) - BAND / 2, width - cx, BAND);
      }
      ctx.globalAlpha = 1;
    }

    // Trait qui relie une étiquette, dans la colonne de gauche, au commit qu'elle désigne.
    ctx.lineWidth = ribbon ? 2 : 1.5;
    ctx.globalAlpha = ribbon ? 0.6 : 0.45;
    for (let i = winStart; i < winEnd; i++) {
      const commit = commits[i];
      if (!commit.refs.length) continue;
      ctx.strokeStyle = color(commit.lane);
      ctx.beginPath();
      ctx.moveTo(0, y(i));
      ctx.lineTo(x(commit.lane) - node - 3, y(i));
      ctx.stroke();
    }
    ctx.globalAlpha = 1;

    // Segments : de la ligne i vers la ligne i + 1. On part d'une ligne plus haut pour
    // tracer aussi ce qui entre dans la fenêtre par le dessus.
    ctx.lineWidth = line;
    for (let i = Math.max(0, winStart - 1); i < winEnd; i++) {
      const commit = commits[i];
      const next = commits[i + 1];
      const y1 = y(i);
      const y2 = y1 + row;
      for (const [from, to, lane] of commit.edges) {
        const [x1, x2] = [x(from), x(to)];
        ctx.strokeStyle = color(lane);
        ctx.setLineDash(commit.wip && lane === commit.lane ? [3, 3] : []);
        ctx.beginPath();
        ctx.moveTo(x1, y1);
        if (from === to) {
          ctx.lineTo(x2, y2);
        } else if (!ribbon) {
          ctx.bezierCurveTo(x1, y1 + row * 0.6, x2, y2 - row * 0.6, x2, y2);
        } else {
          // Tracé « plan de métro » : un angle arrondi plutôt qu'une courbe en S.
          const radius = Math.min(10, Math.abs(x2 - x1), row / 2);
          if (from === commit.lane && lane !== commit.lane) {
            // Second parent d'une fusion : la ligne sort du nœud par le côté, puis descend dans
            // sa propre colonne. Descendre d'abord recouvrirait la ligne du premier parent.
            ctx.arcTo(x2, y1, x2, y2, radius);
          } else if (next && to === next.lane) {
            // La ligne rejoint le commit du dessous : elle descend dans sa colonne, puis entre
            // dans le nœud par le côté.
            ctx.arcTo(x1, y2, x2, y2, radius);
          } else {
            // Simple changement de colonne d'une ligne qui passe.
            ctx.bezierCurveTo(x1, y1 + row * 0.55, x2, y2 - row * 0.55, x2, y2);
            ctx.stroke();
            continue;
          }
          ctx.lineTo(x2, y2);
        }
        ctx.stroke();
      }
    }
    ctx.setLineDash([]);

    // Nœuds, par-dessus les segments.
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (let i = winStart; i < winEnd; i++) {
      const commit = commits[i];
      const cx = x(commit.lane);
      const cy = y(i);
      const tint = color(commit.lane);
      const isHead = commit.refs.some((r) => r.kind === 'head' || r.kind === 'detached');
      const isMerge = commit.parents.length > 1;
      const radius = isMerge ? merge : node;

      if (isHead) {
        ctx.beginPath();
        ctx.arc(cx, cy, radius + 3.5, 0, Math.PI * 2);
        ctx.strokeStyle = tint;
        ctx.globalAlpha = 0.55;
        ctx.lineWidth = 1.5;
        ctx.stroke();
        ctx.globalAlpha = 1;
      }

      ctx.beginPath();
      ctx.arc(cx, cy, commit.wip ? node - 1.5 : radius, 0, Math.PI * 2);
      ctx.lineWidth = 2;
      if (commit.wip) {
        ctx.fillStyle = background;
        ctx.strokeStyle = tint;
        ctx.setLineDash([2.5, 2]);
      } else {
        ctx.fillStyle = tint;
        ctx.strokeStyle = background;
      }
      ctx.fill();
      ctx.stroke();
      ctx.setLineDash([]);

      // Initiales de l'auteur dans la pastille.
      if (ribbon && !commit.wip && !isMerge) {
        const letters = initials(commit.author);
        ctx.fillStyle = onColor;
        ctx.font = `700 ${letters.length > 1 ? 8 : 10}px ${family}`;
        ctx.fillText(letters, cx, cy + 0.5);
      }
    }
  }

  $effect(() => {
    // Redessine quand la fenêtre, les données, la largeur, le style ou le thème changent.
    void dark;
    draw();
  });

  // Apparition en cascade après une ouverture ou un rechargement complet.
  $effect(() => {
    if (repo.loadCount === 0) return;
    fresh = true;
    const timer = window.setTimeout(() => (fresh = false), 700);
    return () => window.clearTimeout(timer);
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
      repo.frames = {
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
      ArrowDown: repo.selected + 1,
      ArrowUp: repo.selected - 1,
      PageDown: repo.selected + page,
      PageUp: repo.selected - page,
      Home: 0,
      End: commits.length - 1,
    };
    if (!(e.key in moves)) return;
    e.preventDefault();
    repo.selected = Math.max(0, Math.min(commits.length - 1, moves[e.key]));
    reveal(repo.selected);
  }

  // --- Étiquettes ---------------------------------------------------------------------------

  /** Double-clic sur une étiquette : on bascule sur la branche. */
  function switchTo(label: Label) {
    if (repo.locked) return;
    if (label.local) {
      if (label.local !== repo.branch) void repo.refOp({ op: 'switch', name: label.local }, `Basculé sur ${label.local}`, 'Impossible de basculer');
    } else if (label.remotes[0]) {
      const name = label.remotes[0];
      void repo.refOp({ op: 'switch-remote', name }, `Basculé sur ${bare(name)}`, 'Impossible de basculer');
    }
  }

  // Étiquette survolée pendant un glissement, pour la mettre en évidence.
  let dropOn = $state('');
  const labelKey = (label: Label, commit: CommitRow) => `${commit.sha}:${label.kind}:${label.name}`;

  function onDrop(event: DragEvent, target: Label) {
    const source = drag.label;
    dropOn = '';
    drag.label = null;
    if (!source || !isBranch(target)) return;
    event.preventDefault();
    menu.open(event, dropMenu(repo, source, target));
  }

  function openCommitMenu(event: MouseEvent, commit: CommitRow, index: number) {
    repo.selected = index;
    menu.open(event, commitMenu(repo, commit));
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

    repo.reveal = reveal;
    repo.resetScroll = () => {
      scrollTop = 0;
      if (scroller) scroller.scrollTop = 0;
    };

    return () => {
      media.removeEventListener('change', onTheme);
      observer.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  });
</script>

{#snippet chip(label: Label, commit: CommitRow)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="ref k-{label.kind}"
    class:local={label.local !== null}
    class:drop={dropOn === labelKey(label, commit)}
    style:--c="var(--l{commit.lane % 8})"
    title="{label.title} — double-clic pour basculer, clic droit pour les actions, glisser sur une autre branche pour fusionner ou rebaser"
    draggable={isBranch(label) ? 'true' : undefined}
    ondragstart={(e) => {
      drag.label = label;
      e.dataTransfer?.setData('text/plain', label.name);
      if (e.dataTransfer) e.dataTransfer.effectAllowed = 'link';
    }}
    ondragend={() => {
      drag.label = null;
      dropOn = '';
    }}
    ondragover={(e) => {
      if (!accepts(label)) return;
      e.preventDefault();
      dropOn = labelKey(label, commit);
    }}
    ondragleave={() => (dropOn = '')}
    ondrop={(e) => onDrop(e, label)}
    ondblclick={(e) => {
      e.stopPropagation();
      switchTo(label);
    }}
    oncontextmenu={(e) => menu.open(e, labelMenu(repo, label, commit.sha))}
  >
    {#if label.kind === 'head'}<Icon name="check" size={12} />{/if}
    <span class="ref-name">{label.name}</span>
    {#if label.local}<Icon name="laptop" size={12} />{/if}
    {#if label.remotes.length}<Icon name="cloud" size={12} />{/if}
    {#if label.tag}<Icon name="tag" size={12} />{/if}
  </span>
{/snippet}

<div class="graph" class:ribbon style:--labels="{LABELS}px" style:--graph="{graphW}px" style:--row="{ROW}px" style:--band="{BAND}px">
  <div class="head">
    <span>Branche / Tag</span><span>Graphe</span><span>Message</span><span>Auteur</span><span>Date</span><span>SHA</span>
  </div>

  {#if repo.result && commits.length === 0}
    <div class="empty">
      <span class="art"><Icon name="commit" size={28} /></span>
      <p>Ce dépôt ne contient encore aucun commit.</p>
    </div>
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
    aria-activedescendant={repo.current ? `commit-${repo.selected}` : undefined}
  >
    <div class="spacer" class:fresh style:height="{commits.length * ROW}px">
      <canvas
        bind:this={canvas}
        class="lanes"
        style:top="{winStart * ROW}px"
        style:width="{graphW}px"
        style:height="{(winEnd - winStart) * ROW}px"
        aria-hidden="true"
      ></canvas>
      {#if ribbon && headIndex >= winStart && headIndex < winEnd && repo.head}
        <!-- Halo animé sur le commit courant : un élément CSS, pour ne pas redessiner le canvas à chaque image. -->
        <span
          class="pulse"
          aria-hidden="true"
          style:top="{headIndex * ROW + ROW / 2}px"
          style:left="{LABELS + metrics.pad + repo.head.lane * metrics.lane}px"
          style:--c="var(--l{repo.head.lane % 8})"
        ></span>
      {/if}
      {#each rows as commit, i (commit.sha)}
        {@const index = winStart + i}
        {@const labels = commit.wip ? [] : labelsOf(commit, repo.shortRemotes)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          class:sel={index === repo.selected}
          id="commit-{index}"
          role="option"
          tabindex="-1"
          aria-selected={index === repo.selected}
          style:top="{index * ROW}px"
          style:--i={Math.min(i, 30)}
          style:--c="var(--l{commit.lane % 8})"
          onclick={() => (repo.selected = index)}
          oncontextmenu={(e) => openCommitMenu(e, commit, index)}
        >
          <span class="labels">
            {#if labels.length}
              {@render chip(labels[0], commit)}
              {#if labels.length > 1}
                <span class="more">+{labels.length - 1}</span>
                <span class="all">
                  {#each labels as label (label.kind + label.name)}{@render chip(label, commit)}{/each}
                </span>
              {/if}
              <span class="wire"></span>
            {/if}
          </span>
          <span></span>
          <span class="msg">
            {#if commit.wip}
              <span class="wip-tag mono">// WIP</span>
              {#if wip}
                {#if wip.conflicts}<span class="stat bad" title="Fichiers en conflit"><Icon name="warning" size={12} />{wip.conflicts}</span>{/if}
                {#if wip.modified}<span class="stat warn" title="Fichiers modifiés"><Icon name="pencil" size={12} />{wip.modified}</span>{/if}
                {#if wip.added}<span class="stat ok" title="Nouveaux fichiers"><Icon name="plus" size={12} />{wip.added}</span>{/if}
                {#if wip.deleted}<span class="stat bad" title="Fichiers supprimés"><Icon name="minus" size={12} />{wip.deleted}</span>{/if}
              {:else}
                <span class="subject muted">{repo.wipCount} fichier{repo.wipCount > 1 ? 's' : ''} modifié{repo.wipCount > 1 ? 's' : ''}</span>
              {/if}
            {:else}
              <span class="subject">{commit.subject}</span>
            {/if}
          </span>
          <span class="author">
            {#if !commit.wip}
              {#if !ribbon}
                <span class="avatar" style:--c="var(--l{hue(commit.author)})" aria-hidden="true">{initials(commit.author)}</span>
              {/if}
              <span class="who">{commit.author}</span>
            {/if}
          </span>
          <span class="date">{commit.wip ? '' : dateFmt.format(commit.time * 1000)}</span>
          <span class="sha mono">{commit.wip ? '' : commit.sha.slice(0, 7)}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .graph {
    display: grid;
    grid-template-rows: 28px minmax(0, 1fr);
    height: 100%;
    min-width: 0;
    min-height: 0;
    position: relative;
    container-type: inline-size;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) 150px 92px 66px;
    align-items: center;
  }
  /* Quand la place manque, le message passe avant le reste : le SHA disparaît d'abord
     (il reste dans le panneau de détail), puis le nom de l'auteur, puis la date. */
  @container (max-width: 860px) {
    .head,
    .row {
      grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) 150px 92px;
    }
    .head span:nth-child(6),
    .sha {
      display: none;
    }
  }
  @container (max-width: 740px) {
    .head,
    .row {
      grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) 34px 92px;
    }
    .head span:nth-child(4) {
      visibility: hidden;
    }
    .who {
      display: none;
    }
    /* Le rendu « Ruban » montre déjà l'auteur sur le graphe : sa colonne disparaît entièrement. */
    .ribbon .head,
    .ribbon .row {
      grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) 0 92px;
    }
  }
  @container (max-width: 600px) {
    .head,
    .row,
    .ribbon .head,
    .ribbon .row {
      grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) 34px;
    }
    .ribbon .head,
    .ribbon .row {
      grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) 0;
    }
    .head span:nth-child(5),
    .date {
      display: none;
    }
  }
  .head {
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    font-size: 10.5px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--muted);
    font-weight: 600;
    overflow: hidden;
    scrollbar-gutter: stable;
    padding-right: 10px;
    white-space: nowrap;
  }
  .head span {
    padding-left: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .head span:nth-child(2) {
    padding-left: 8px;
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
    left: var(--labels);
    pointer-events: none;
    z-index: 1;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--row);
    padding-right: 10px;
    cursor: pointer;
    white-space: nowrap;
    /* Un double-clic bascule de branche : il ne doit pas sélectionner du texte au passage. */
    user-select: none;
    transition: background var(--t-fast) var(--ease);
  }
  .row::before {
    content: '';
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
    transform: scaleY(0);
    transition: transform var(--t-med) var(--ease);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--sel);
  }
  .row.sel::before {
    transform: scaleY(1);
  }
  .fresh .row {
    animation: row-in var(--t-slow) var(--ease) both;
    animation-delay: calc(var(--i) * 14ms);
  }
  .fresh .lanes {
    animation: fade-in 500ms var(--ease) both;
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateX(-10px);
    }
  }

  /* Halo du commit courant */
  .pulse {
    position: absolute;
    z-index: 2;
    width: 20px;
    height: 20px;
    margin: -10px 0 0 -10px;
    border-radius: 50%;
    border: 2px solid var(--c);
    pointer-events: none;
    animation: pulse 2.4s var(--ease) infinite;
  }
  @keyframes pulse {
    from {
      transform: scale(1);
      opacity: 0.7;
    }
    70%,
    to {
      transform: scale(2.3);
      opacity: 0;
    }
  }

  /* Colonne des branches et des tags */
  .labels {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    height: 100%;
    padding-left: 10px;
  }
  .wire {
    flex: 1;
    min-width: 6px;
    height: 1.5px;
    background: var(--c);
    opacity: 0.45;
  }
  .ribbon .wire {
    height: 2px;
    opacity: 0.6;
  }
  .ref {
    flex: 0 1 auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    font-size: 11px;
    line-height: 18px;
    padding: 0 7px;
    border-radius: 5px;
    border: 1px solid var(--c, var(--line));
    background: color-mix(in srgb, var(--c, var(--line)) 16%, var(--bg));
    font-weight: 500;
    transition:
      background var(--t-fast) var(--ease),
      filter var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .ref:hover {
    background: color-mix(in srgb, var(--c, var(--line)) 30%, var(--bg));
  }
  .ref:active {
    transform: scale(0.97);
  }
  .ref[draggable='true'] {
    cursor: grab;
  }
  /* Branche sur laquelle on s'apprête à déposer celle qu'on glisse. */
  .ref.drop,
  .ribbon .ref.drop {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    transform: scale(1.06);
  }
  .ref-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Préfixe `k-` : la branche courante a le type `head`, qui ne doit pas hériter de l'en-tête `.head`. */
  .ref.k-head,
  .ref.k-detached {
    font-weight: 700;
    background: color-mix(in srgb, var(--c) 30%, var(--bg));
  }
  .ref.k-remote {
    border-style: dashed;
    font-weight: 400;
  }
  .ref.k-tag {
    --c: var(--muted);
    border-radius: 10px;
    font-weight: 400;
  }
  /* Rendu « Ruban » : une branche locale est une pastille pleine, de la couleur de sa ligne. */
  .ribbon .ref {
    line-height: 20px;
    border-radius: 6px;
  }
  .ribbon .ref.local {
    background: var(--c);
    color: var(--on-accent);
    font-weight: 600;
  }
  .ribbon .ref.local:hover {
    background: var(--c);
    filter: brightness(1.12);
  }
  .ribbon .ref.k-head {
    font-weight: 700;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 28%, transparent);
  }
  .more {
    flex: none;
    font-size: 10.5px;
    padding: 0 5px;
    line-height: 16px;
    border-radius: 8px;
    background: var(--side);
    border: 1px solid var(--line);
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  /* Toutes les étiquettes d'un commit, dépliées au survol de la colonne. */
  .all {
    position: absolute;
    left: 6px;
    top: 2px;
    z-index: 4;
    display: none;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 3px 8px 6px 3px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    max-width: 320px;
  }
  .labels:hover .all {
    display: flex;
    animation: pop-in var(--t-fast) var(--ease);
  }

  .msg {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    padding-left: 10px;
  }
  /* Le ruban dessiné sur le canvas se prolonge ici, puis s'estompe sous le texte. */
  .ribbon .msg {
    height: var(--band);
    background: linear-gradient(90deg, color-mix(in srgb, var(--c) 13%, transparent), transparent min(420px, 85%));
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .wip-tag {
    font-size: 11px;
    line-height: 18px;
    padding: 0 7px;
    border-radius: 5px;
    border: 1px dashed var(--muted);
    color: var(--muted);
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--c);
  }
  .stat.ok {
    --c: var(--ok);
  }
  .stat.warn {
    --c: var(--warn);
  }
  .stat.bad {
    --c: var(--bad);
  }
  .author {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding-left: 10px;
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
  }
  .who {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 20%, var(--bg));
    border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
  }
  .date {
    padding-left: 10px;
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sha {
    padding-left: 10px;
    color: var(--muted);
    font-size: 11.5px;
  }
  .muted {
    color: var(--muted);
  }

  .empty {
    position: absolute;
    inset: 28px 0 0 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 14px;
    text-align: center;
    padding: 24px;
    z-index: 2;
    background: var(--bg);
    animation: fade-in var(--t-slow) var(--ease);
  }
  .empty p {
    margin: 0;
    max-width: 52ch;
    color: var(--muted);
    font-size: 14px;
  }
  .art {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 20px;
    color: var(--accent);
    background: var(--accent-soft);
    animation: float 4s ease-in-out infinite;
  }
  @keyframes float {
    50% {
      transform: translateY(-5px);
    }
  }
</style>
