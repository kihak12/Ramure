<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu } from '../ui/menu.svelte';
  import { commitMenu, dropMenu, labelMenu, switchBranch, switchToRemote } from './actions';
  import { accepts, drag, isBranch } from './drag.svelte';
  import { bare, labelsOf, type Label } from './labels';
  import { revealDetail } from './layout.svelte';
  import { theme } from '../settings/theme.svelte';
  import { prefs } from './prefs.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo }: { repo: Repo } = $props();

  // Deux rendus du graphe, au choix dans les réglages (charte graphique, section 10).
  // « Ruban » : pastilles d'auteur sur le graphe, courbes souples, message dans une capsule de
  // la couleur de la branche. « Classique » : points et courbes, plus compact.
  const STYLES = {
    ribbon: { row: 34, lane: 24, pad: 20, node: 10, merge: 5, line: 3, max: 420 },
    classic: { row: 28, lane: 16, pad: 14, node: 5, merge: 4, line: 2, max: 340 },
  } as const;

  const LABELS = { max: 220, min: 164 }; // colonne « Branche / Tag »
  const MESSAGE = { wanted: 200, min: 120 }; // colonne du message
  const GRAPH_MIN = 72; // le graphe n'est jamais plus étroit que son en-tête
  const ROW_END = 10; // marge droite d'une ligne
  const BUFFER = 24; // lignes rendues au-dessus et au-dessous de la zone visible
  const STEP = 12; // la fenêtre de rendu ne se déplace que par paliers de 12 lignes
  const BAND = 24; // hauteur de la bande qui relie un commit à la capsule de son message
  const BAND_ALPHA = 0.15; // même opacité que la capsule CSS qui la prolonge dans la colonne du message

  let scrollTop = $state(0);
  let viewH = $state(600);
  // Le canvas lit ses couleurs dans les jetons : il se redessine quand le thème change.
  const dark = $derived(theme.key);
  let fontsReady = $state(false); // le canevas doit se redessiner quand la police embarquée est chargée
  // Vrai juste après une ouverture : les lignes entrent en cascade.
  let fresh = $state(false);

  let scroller = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();

  const ribbon = $derived(prefs.graphStyle !== 'classic');
  const metrics = $derived(STYLES[ribbon ? 'ribbon' : 'classic']);
  const ROW = $derived(metrics.row);

  const commits = $derived(repo.commits);
  // Largeur que demanderaient toutes les voies, jamais moins que l'en-tête de colonne.
  const lanesW = $derived(
    Math.max(GRAPH_MIN, Math.min(metrics.max, metrics.pad * 2 + Math.max(0, (repo.result?.maxLanes ?? 1) - 1) * metrics.lane + 6)),
  );

  // --- Colonnes (charte, section 10.1) ------------------------------------------------------
  // Le message passe d'abord. Quand il aurait moins de 200 px, les autres colonnes cèdent, dans
  // l'ordre : le SHA, le nom de l'auteur, la date, puis « Branche / Tag » jusqu'à 164 px, puis le
  // graphe, dont les voies de droite sont coupées, jusqu'à ce que le message ait 120 px. Le calcul
  // part de la place qui reste après le graphe : un dépôt à nombreuses voies montre moins de colonnes.
  let cardW = $state(0);
  const columns = $derived.by(() => {
    // Colonnes à droite du message, de la plus complète à la plus réduite. `who` : le nom de
    // l'auteur est écrit ; le rendu « Classique » garde la place de ses initiales.
    const initials = ribbon ? 0 : 34;
    const levels = [
      { author: 150, who: true, date: 92, sha: 66 },
      { author: 150, who: true, date: 92, sha: 0 },
      { author: initials, who: false, date: 92, sha: 0 },
      { author: initials, who: false, date: 0, sha: 0 },
    ];
    const rest = (level: (typeof levels)[number]) => level.author + level.date + level.sha;
    if (cardW <= 0) return { ...levels[0], labels: LABELS.max, graph: lanesW };
    const room = cardW - ROW_END;
    const level = levels.find((l) => room - rest(l) - LABELS.max - lanesW >= MESSAGE.wanted) ?? levels[levels.length - 1];
    const left = room - rest(level);
    const labels = Math.max(LABELS.min, Math.min(LABELS.max, left - lanesW - MESSAGE.wanted));
    let graph = Math.max(GRAPH_MIN, Math.min(lanesW, left - labels - MESSAGE.min));
    // Graphe coupé : il s'arrête juste après la dernière voie entière, pour ne pas montrer une voie
    // sans ses points. La place gagnée revient au message.
    if (graph < lanesW) {
      const { pad, node, lane } = metrics;
      const whole = Math.max(1, Math.floor((graph - pad - node - 4) / lane) + 1);
      graph = Math.max(GRAPH_MIN, pad + (whole - 1) * lane + node + 4);
    }
    return { ...level, labels, graph };
  });
  const graphW = $derived(columns.graph);
  // Voies qui tiennent dans la colonne, point et anneau du commit courant compris. Au-delà, elles
  // sont coupées : leurs commits n'ont pas de point, et leur bande part du bord de la colonne.
  const shownLanes = $derived(Math.max(1, Math.floor((graphW - metrics.pad - metrics.node - 4) / metrics.lane) + 1));
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
    const background = css.getPropertyValue('--panel').trim(); // le graphe est posé sur une carte
    const onColor = css.getPropertyValue('--on-accent').trim();
    const family = getComputedStyle(document.body).fontFamily;
    const initialSize = parseFloat(css.getPropertyValue('--fs-micro'));
    const initialWeight = css.getPropertyValue('--fw-strong').trim();
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
        const cx = commit.lane < shownLanes ? x(commit.lane) : width - pad;
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
        } else {
          // Changement de colonne : une courbe souple, sans angle.
          ctx.bezierCurveTo(x1, y1 + row * 0.6, x2, y2 - row * 0.6, x2, y2);
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
      if (commit.lane >= shownLanes) continue;
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
        ctx.font = `${initialWeight} ${initialSize}px ${family}`;
        // Deux initiales trop larges pour la pastille : on ne garde que la première, sans réduire le texte.
        const room = 2 * Math.sqrt(Math.max(0, node * node - (initialSize * 0.36) ** 2)) - 3;
        ctx.fillText(ctx.measureText(letters).width > room ? letters.slice(0, 1) : letters, cx, cy + 0.5);
      }
    }
  }

  $effect(() => {
    // Redessine quand la fenêtre, les données, la largeur, le style ou le thème changent.
    void dark;
    void fontsReady;
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

  /** Clic sur une ligne : elle est sélectionnée, et le focus reste sur la liste, qui gère le clavier.
   *  Une ligne qui garderait le focus le perdrait en sortant de la fenêtre de rendu. */
  function select(index: number) {
    repo.selected = index;
    scroller?.focus({ preventScroll: true });
    // La ligne des modifications en cours : on y vient pour indexer ou commiter, le détail revient.
    if (commits[index]?.wip) revealDetail();
  }

  /** Focus au clavier : la liste a toujours un commit sélectionné et visible, c'est lui qui porte le repère. */
  function onFocus() {
    if (!commits.length || !scroller?.matches(':focus-visible')) return;
    if (repo.selected < 0) repo.selected = 0;
    reveal(repo.selected);
  }

  function onKey(e: KeyboardEvent) {
    if (!commits.length) return;
    // Entrée demande le détail du commit sélectionné : il revient s'il était masqué. Les flèches,
    // elles, ne font que déplacer la sélection (charte, section 9.17).
    if (e.key === 'Enter' && repo.selected >= 0) {
      e.preventDefault();
      revealDetail();
      return;
    }
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
      void switchBranch(repo, label.local);
    } else if (label.remotes[0]) {
      // Branche distante seule sur son commit : on bascule sur la branche locale et on la met à jour.
      void switchToRemote(repo, label.remotes[0]);
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
    void document.fonts.ready.then(() => (fontsReady = true));

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
      observer.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  });
</script>

{#snippet chip(label: Label, commit: CommitRow)}
  {@const waiting = repo.switching !== null && (label.local === repo.switching || label.remotes.includes(repo.switching))}
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
    {#if label.kind === 'head'}<Icon name="check" size={14} />{/if}
    <span class="ref-name">{label.name}</span>
    <!-- Pendant le basculement, l'anneau prend la place d'une icône : l'étiquette garde sa taille. -->
    {#if label.local}{#if waiting}<span class="wait" aria-hidden="true"><span class="spinner"></span></span>{:else}<Icon name="laptop" size={14} />{/if}{/if}
    {#if label.remotes.length}{#if waiting && !label.local}<span class="wait" aria-hidden="true"><span class="spinner"></span></span>{:else}<Icon name="cloud" size={14} />{/if}{/if}
    {#if label.tag}<Icon name="tag" size={14} />{/if}
  </span>
{/snippet}

<div
  class="graph"
  class:ribbon
  class:no-who={!columns.who}
  class:no-date={!columns.date}
  class:no-sha={!columns.sha}
  style:--labels="{columns.labels}px"
  style:--graph="{graphW}px"
  style:--author="{columns.author}px"
  style:--date="{columns.date}px"
  style:--sha="{columns.sha}px"
  style:--row="{ROW}px"
  style:--band="{BAND}px"
>
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
    bind:clientWidth={cardW}
    onscroll={onScroll}
    onkeydown={onKey}
    onfocus={onFocus}
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
      {#if ribbon && headIndex >= winStart && headIndex < winEnd && repo.head && repo.head.lane < shownLanes - 1}
        <!-- Halo animé sur le commit courant : un élément CSS, pour ne pas redessiner le canvas à chaque image. -->
        <span
          class="pulse"
          aria-hidden="true"
          style:top="{headIndex * ROW + ROW / 2}px"
          style:--x="{metrics.pad + repo.head.lane * metrics.lane}px"
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
          onclick={() => select(index)}
          ondblclick={revealDetail}
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
          <span>
            {#if ribbon && !commit.wip && commit.lane < shownLanes}
              <!-- Le point est dessiné sur le canevas, qui ne reçoit pas le pointeur : cette pastille
                   transparente, posée dessus, porte le nom de l'auteur, celui d'une fusion compris
                   (charte, 10.2). -->
              <span
                class="dot"
                style:--x="{metrics.pad + commit.lane * metrics.lane}px"
                title={commit.author}
                aria-hidden="true"
              ></span>
            {/if}
          </span>
          <span class="msg">
            {#if commit.wip}
              <span class="wip-tag mono">// WIP</span>
              {#if wip}
                {#if wip.conflicts}<span class="stat bad" title="Fichiers en conflit"><Icon name="warning" size={14} />{wip.conflicts}</span>{/if}
                {#if wip.modified}<span class="stat warn" title="Fichiers modifiés"><Icon name="pencil" size={14} />{wip.modified}</span>{/if}
                {#if wip.added}<span class="stat ok" title="Nouveaux fichiers"><Icon name="plus" size={14} />{wip.added}</span>{/if}
                {#if wip.deleted}<span class="stat bad" title="Fichiers supprimés"><Icon name="minus" size={14} />{wip.deleted}</span>{/if}
              {:else}
                <span class="subject muted">{repo.wipCount} fichier{repo.wipCount > 1 ? 's' : ''} modifié{repo.wipCount > 1 ? 's' : ''}</span>
              {/if}
            {:else}
              <!-- Voie coupée : le point et les initiales manquent, l'auteur est dans l'infobulle. -->
              <span class="subject" title={commit.lane < shownLanes ? commit.subject : `${commit.subject} — ${commit.author}`}>{commit.subject}</span>
            {/if}
          </span>
          <span class="author">
            {#if !commit.wip}
              {#if !ribbon}
                <span class="avatar" style:--c="var(--l{hue(commit.author)})" title={commit.author} aria-hidden="true">{initials(commit.author)}</span>
              {/if}
              <span class="who" title={commit.author}>{commit.author}</span>
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
  }
  /* Largeurs calculées dans le script (charte, section 10.1). Une colonne qui cède garde sa
     place dans la grille, à 0 px, et son contenu est caché. */
  .head,
  .row {
    display: grid;
    grid-template-columns: var(--labels) var(--graph) minmax(0, 1fr) var(--author) var(--date) var(--sha);
    align-items: center;
  }
  .head span:nth-child(n + 4) {
    overflow: hidden;
    white-space: nowrap;
  }
  /* `visibility`, et non `display` : une cellule retirée décalerait les suivantes d'une colonne. */
  .no-who .head span:nth-child(4),
  .no-who .who,
  .no-date .head span:nth-child(5),
  .no-date .date,
  .no-sha .head span:nth-child(6),
  .no-sha .sha {
    visibility: hidden;
  }
  .head {
    border-bottom: 1px solid var(--line-soft);
    font-size: var(--fs-label);
    color: var(--muted);
    font-weight: var(--fw-strong);
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
  /* Au clavier, le focus se lit sur le commit sélectionné, pas par un cadre autour de la liste. */
  .scroller:focus-visible,
  .scroller:global(.focus-ring):focus,
  .row:focus-visible {
    outline: none;
  }
  /* `.focus-ring` : le focus rendu à la liste par le script, à la fermeture d'un menu demandé au
     clavier (lib/ui/focus.ts). Il se montre comme celui de Tab, sur la ligne sélectionnée. */
  .scroller:focus-visible .row.sel::before,
  .scroller:global(.focus-ring):focus .row.sel::before,
  .scroller:has(.row:focus-visible) .row.sel::before {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  /* Les lignes, le canevas et le halo s'empilent entre eux, à part du reste de la carte. */
  .spacer {
    position: relative;
    isolation: isolate;
  }
  .lanes {
    position: absolute;
    left: var(--labels);
    pointer-events: none;
    z-index: 2;
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
    /* Le fond de survol et de sélection passe sous le contenu de la ligne. */
    isolation: isolate;
  }
  /* Survol et sélection : un fond arrondi, en retrait des bords de la carte. Pas de trait sur le bord. */
  .row::before {
    content: '';
    position: absolute;
    inset: 1px 6px;
    z-index: -1;
    border-radius: var(--radius);
    background: var(--hover);
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  .row:hover::before {
    opacity: 1;
  }
  .row.sel::before {
    opacity: 1;
    background: var(--sel);
  }
  .fresh .row {
    animation: row-in var(--t-slow) var(--ease) both;
    animation-delay: calc(var(--i) * var(--t-step));
  }
  .fresh .lanes {
    animation: fade-in var(--t-slow) var(--ease) both;
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateX(-10px);
    }
  }

  /* Posée sur le point d'un commit, en « Ruban » : elle ne montre rien, elle porte l'infobulle. */
  .dot {
    position: absolute;
    top: 50%;
    left: calc(var(--labels) + var(--x));
    width: 20px;
    height: 20px;
    margin: -10px 0 0 -10px;
    border-radius: 50%;
  }

  /* Halo du commit courant */
  .pulse {
    position: absolute;
    left: calc(var(--labels) + var(--x));
    z-index: 2;
    width: 20px;
    height: 20px;
    margin: -10px 0 0 -10px;
    border-radius: 50%;
    border: 2px solid var(--c);
    pointer-events: none;
    animation: pulse var(--t-halo) var(--ease) infinite;
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
  /* L'anneau de 12 px est centré dans la boîte de 14 px de l'icône qu'il remplace. */
  .ref .wait {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
  }
  .ref .spinner {
    --spinner: currentColor;
  }
  .ref {
    flex: 0 1 auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-small);
    line-height: 20px;
    padding: 0 8px;
    border-radius: var(--radius-s);
    border: 1px solid var(--c, var(--line));
    background: color-mix(in srgb, var(--c, var(--line)) 16%, var(--panel));
    font-weight: var(--fw-regular);
    /* Une étiquette ne change jamais de taille : ni au clic, ni au survol, ni au dépôt. */
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  .ref:hover {
    background: color-mix(in srgb, var(--c, var(--line)) 28%, var(--panel));
  }
  .ref[draggable='true'] {
    cursor: grab;
  }
  /* Branche sur laquelle on s'apprête à déposer celle qu'on glisse. */
  .ref.drop,
  .ribbon .ref.drop {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .ref-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Préfixe `k-` : la branche courante a le type `head`, qui ne doit pas hériter de l'en-tête `.head`. */
  .ref.k-head,
  .ref.k-detached {
    font-weight: var(--fw-strong);
    background: color-mix(in srgb, var(--c) 28%, var(--panel));
  }
  .ref.k-remote {
    border-style: dashed;
    font-weight: var(--fw-regular);
  }
  .ref.k-tag {
    font-weight: var(--fw-regular);
  }
  .ref.local {
    font-weight: var(--fw-strong);
  }
  /* Rendu « Ruban » : toutes les étiquettes ont le même fond, teinté de la couleur de leur ligne.
     L'icône, la bordure et la graisse disent si c'est une branche locale, distante ou un tag.
     La branche courante est pleine. */
  .ribbon .ref:not(.k-detached) {
    background: color-mix(in srgb, var(--c, var(--line)) 24%, var(--panel));
  }
  /* Pas plus de 28 % : au-delà, le texte passe sous 7:1 dans certains thèmes sombres (charte, section 2.4). */
  .ribbon .ref:not(.k-detached):hover {
    background: color-mix(in srgb, var(--c, var(--line)) 28%, var(--panel));
  }
  .ribbon .ref.local {
    border-color: color-mix(in srgb, var(--c) 55%, transparent);
    font-weight: var(--fw-strong);
  }
  .ribbon .ref.k-head {
    background: var(--c);
    border-color: var(--c);
    color: var(--on-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 28%, transparent);
  }
  /* Au survol, le fond va vers la couleur du texte courant : plus sombre en clair, plus clair en
     sombre. Le texte de la branche courante y gagne du contraste dans les deux thèmes. */
  .ribbon .ref.k-head:hover {
    background: color-mix(in srgb, var(--c) 90%, var(--ink));
    border-color: color-mix(in srgb, var(--c) 90%, var(--ink));
  }
  .more {
    flex: none;
    font-size: var(--fs-micro);
    padding: 0 5px;
    line-height: 16px;
    border-radius: var(--radius-pill);
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
  /* La ligne dont on survole les étiquettes passe devant les suivantes : sans cela, la liste
     dépliée resterait sous elles, visible mais hors d'atteinte du pointeur, et se fermerait dès
     qu'il descend dessus. Elle reste sous le canevas, qui garde ses traits par-dessus son fond. */
  .row:has(.labels:hover) {
    z-index: 1;
  }

  .msg {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    padding-left: 10px;
  }
  /* La bande dessinée sur le canvas se prolonge ici en capsule : large comme le texte, arrondie à droite. */
  .ribbon .msg {
    width: fit-content;
    max-width: 100%;
    height: var(--band);
    padding-right: 14px;
    border-radius: 0 var(--radius-pill) var(--radius-pill) 0;
    background: color-mix(in srgb, var(--c) 15%, transparent);
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .wip-tag {
    font-size: var(--fs-small);
    line-height: 18px;
    padding: 0 7px;
    border-radius: var(--radius-s);
    border: 1px dashed var(--muted);
    color: var(--muted);
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    color: var(--c);
  }
  /* Deux compteurs sont plus écartés que l'icône ne l'est de son chiffre. */
  .stat + .stat {
    margin-left: 4px;
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
    font-size: var(--fs-label);
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
    font-size: var(--fs-micro);
    font-weight: var(--fw-strong);
    letter-spacing: -0.02em;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 20%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
  }
  .date {
    padding-left: 10px;
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sha {
    padding-left: 10px;
    overflow: hidden;
    color: var(--muted);
    font-size: var(--fs-code);
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
    background: var(--panel);
    animation: fade-in var(--t-slow) var(--ease);
  }
  .empty p {
    margin: 0;
    max-width: 52ch;
    color: var(--muted);
    font-size: var(--fs-body);
  }
  .art {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: var(--radius-card);
    color: var(--accent);
    background: var(--accent-soft);
  }
</style>
