<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { ask, open } from '@tauri-apps/plugin-dialog';
  import { onMount, tick, untrack } from 'svelte';
  import DiffView from './DiffView.svelte';
  import FileList from './FileList.svelte';
  import WorkPanel from './WorkPanel.svelte';
  import type { CommandLog, CommitDetail, CommitRow, Diff, Done, FileChange, LogResult, RefKind, Status } from './types';

  const ROW = 28; // hauteur d'une ligne
  const LANE = 16; // largeur d'une lane
  const PAD = 14; // marge gauche du graphe
  const BUFFER = 24; // lignes rendues au-dessus et au-dessous de la zone visible
  const STEP = 12; // la fenêtre de rendu ne se déplace que par paliers de 12 lignes
  const MAX_GRAPH = 340; // largeur maximale de la colonne du graphe
  const STORAGE_KEY = 'ramure.repo';
  const JOURNAL_MAX = 300; // commandes conservées dans le journal

  type Busy = '' | 'fetch' | 'pull' | 'push' | 'commit' | 'index';
  interface OpenFile {
    /** `u:chemin`, `s:chemin` (répertoire de travail) ou `c:chemin` (commit). */
    key: string;
    title: string;
    subtitle: string;
  }

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

  let journal = $state.raw<CommandLog[]>([]);
  let status = $state.raw<Status | null>(null);
  let busy = $state<Busy>('');
  let notice = $state<{ ok: boolean; title: string; body: string } | null>(null);
  let message = $state('');
  let detail = $state.raw<CommitDetail | null>(null);
  let detailError = $state('');
  let openFile = $state<OpenFile | null>(null);
  let diff = $state.raw<Diff | null>(null);
  let diffLoading = $state(false);
  let diffError = $state('');

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
  const lastCommand = $derived(journal.at(-1) ?? null);
  // Sans branche distante connue, distinguer « local » de « distant » n'a pas de sens.
  const hasRemote = $derived(commits.some((c) => c.refs.some((r) => r.kind === 'remote')));
  // Clé du commit sélectionné : ne change pas quand l'historique est simplement rechargé.
  const currentKey = $derived(current ? `${current.sha}|${current.parents[0] ?? ''}` : '');
  const wipCount = $derived(
    status ? new Set([...status.staged, ...status.unstaged].map((f) => f.path)).size : (result?.wipFiles ?? 0),
  );
  const locked = $derived(loading || busy !== '');

  const dateFmt = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' });
  const fullDateFmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' });
  const numFmt = new Intl.NumberFormat('fr-FR');
  const ms = (n: number) => `${numFmt.format(Math.round(n))} ms`;

  function record(commands: CommandLog[]) {
    if (commands.length) journal = [...journal, ...commands].slice(-JOURNAL_MAX);
  }

  /** Appelle une commande git du cœur Rust et verse ses commandes au journal, réussite ou non. */
  async function git<T>(command: string, args: Record<string, unknown>): Promise<T> {
    try {
      const reply = await invoke<Done<T>>(command, args);
      record(reply.commands);
      return reply.data;
    } catch (e) {
      if (e && typeof e === 'object' && 'message' in e) {
        const failed = e as { message: string; commands?: CommandLog[] };
        record(failed.commands ?? []);
        throw new Error(failed.message);
      }
      throw new Error(typeof e === 'string' ? e : String(e));
    }
  }

  const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

  /** `keep` : conserve la sélection et la position, pour un rechargement après une opération. */
  async function load(path: string, keep = false) {
    if (!path || loading) return;
    loading = true;
    error = '';
    const previous = keep ? current : null;
    const t0 = performance.now();
    try {
      const data = await invoke<LogResult>('git_log', { repo: path, limit: limit > 0 ? limit : null });
      const t1 = performance.now();
      if (data.repo !== result?.repo) {
        status = null;
        message = '';
        notice = null;
      }
      result = data;
      record(data.commands);
      if (previous?.wip) selected = data.commits[0]?.wip ? 0 : -1;
      else if (previous) selected = data.commits.findIndex((c) => c.sha === previous.sha);
      else selected = -1;
      frames = null;
      if (!keep) {
        scrollTop = 0;
        if (scroller) scroller.scrollTop = 0;
      }
      logHead = undefined;
      void refreshStatus(true);
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

  // --- Étiquettes de références -------------------------------------------------------------

  interface Label {
    name: string;
    kind: RefKind;
    /** La référence existe sur cette machine, sur un dépôt distant, ou les deux. */
    local: boolean;
    remote: boolean;
    title: string;
  }

  /** Nom d'une branche distante sans son dépôt : `origin/feat/x` → `feat/x`. */
  const bare = (remote: string) => remote.slice(remote.indexOf('/') + 1);

  /**
   * Étiquettes d'un commit. Une branche locale et ses homologues distantes posées sur le même
   * commit sont réunies en une seule étiquette, qui porte les deux icônes.
   */
  function labels(commit: CommitRow): Label[] {
    const locals = new Set(commit.refs.filter((r) => r.kind === 'head' || r.kind === 'local').map((r) => r.name));
    const out: Label[] = [];
    for (const ref of commit.refs) {
      if (ref.kind === 'head' || ref.kind === 'local') {
        const twins = commit.refs.filter((r) => r.kind === 'remote' && bare(r.name) === ref.name).map((r) => r.name);
        const current = ref.kind === 'head' ? 'Branche courante' : 'Branche locale';
        out.push({
          name: ref.name,
          kind: ref.kind,
          local: true,
          remote: twins.length > 0,
          title: twins.length ? `${current} ${ref.name}, au même commit que ${twins.join(', ')}` : `${current} ${ref.name}`,
        });
      } else if (ref.kind === 'remote') {
        if (!locals.has(bare(ref.name))) {
          out.push({ name: ref.name, kind: ref.kind, local: false, remote: true, title: `Branche distante ${ref.name}` });
        }
      } else {
        const title = ref.kind === 'tag' ? `Tag ${ref.name}` : 'HEAD détaché';
        out.push({ name: ref.name, kind: ref.kind, local: false, remote: false, title });
      }
    }
    return out;
  }

  async function pick() {
    const dir = await open({ directory: true, multiple: false, title: 'Ouvrir un dépôt Git' });
    if (typeof dir === 'string') await load(dir);
  }

  function reload() {
    if (result) void load(result.repo);
  }

  // --- Répertoire de travail, index et commit ---------------------------------------------

  // Commit courant au moment du dernier chargement de l'historique ; `undefined` tant qu'il est inconnu.
  let logHead: string | null | undefined;

  /** Enregistre un nouvel état ; recharge l'historique s'il ne lui correspond plus. */
  function applyStatus(next: Status, fromLoad = false) {
    status = next;
    if (fromLoad || logHead === undefined) {
      logHead = next.head;
      return;
    }
    const dirty = next.staged.length + next.unstaged.length > 0;
    if (result && (next.head !== logHead || dirty !== !!commits[0]?.wip)) void load(result.repo, true);
  }

  async function refreshStatus(fromLoad = false) {
    const repo = result?.repo;
    if (!repo) return;
    try {
      const next = await git<Status>('git_status', { repo });
      if (repo === result?.repo) applyStatus(next, fromLoad);
    } catch (e) {
      if (!fromLoad) notify(false, "Impossible de lire l'état du dépôt", errorText(e));
    }
  }

  let noticeTimer = 0;
  function notify(ok: boolean, title: string, body = '') {
    clearTimeout(noticeTimer);
    // Les sorties bavardes (push, fetch) sont réduites à leurs dernières lignes.
    notice = { ok, title, body: ok ? body.split('\n').slice(-6).join('\n') : body };
    if (ok) noticeTimer = window.setTimeout(() => (notice = null), 5000);
  }

  /** Lance une commande qui modifie l'index et renvoie le nouvel état. */
  async function mutate(command: string, args: Record<string, unknown>) {
    if (!result || busy) return;
    busy = 'index';
    try {
      applyStatus(await git<Status>(command, { repo: result.repo, ...args }));
      syncOpenDiff();
    } catch (e) {
      notify(false, "L'opération a échoué", errorText(e));
      void refreshStatus();
    } finally {
      busy = '';
    }
  }

  const stage = (files: FileChange[]) => mutate('git_stage', { paths: files.map((f) => f.path) });
  // Retirer un renommage de l'index concerne ses deux chemins.
  const unstage = (files: FileChange[]) =>
    mutate('git_unstage', { paths: files.flatMap((f) => (f.oldPath ? [f.oldPath, f.path] : [f.path])) });

  async function discard(files: FileChange[]) {
    if (!files.length) return;
    const what = files.length === 1 ? `« ${files[0].path} »` : `${files.length} fichiers`;
    const fresh = files.filter((f) => f.kind === 'untracked');
    const question =
      fresh.length === files.length
        ? `Supprimer ${what} ? ${files.length === 1 ? 'Ce fichier est nouveau' : 'Ces fichiers sont nouveaux'} : git ne pourra pas le${files.length === 1 ? '' : 's'} restaurer.`
        : `Abandonner les modifications non indexées de ${what} ? Elles seront perdues définitivement.`;
    const confirmed = await ask(question, { title: 'Abandonner les modifications', kind: 'warning', okLabel: 'Abandonner', cancelLabel: 'Annuler' });
    if (!confirmed) return;
    await mutate('git_discard', {
      tracked: files.filter((f) => f.kind !== 'untracked').map((f) => f.path),
      untracked: fresh.map((f) => f.path),
    });
  }

  async function commit() {
    if (!result || busy) return;
    busy = 'commit';
    try {
      await git<null>('git_commit', { repo: result.repo, message });
      message = '';
      closeDiff();
      notify(true, 'Commit créé');
    } catch (e) {
      notify(false, 'Le commit a échoué', errorText(e));
    } finally {
      busy = '';
    }
    await load(result.repo, true);
    // La ligne WIP a disparu : on montre le commit qui vient d'être créé.
    if (selected < 0 && commits.length) selected = 0;
  }

  const SYNC = {
    fetch: { done: 'Récupération terminée', failed: 'La récupération a échoué' },
    pull: { done: 'Branche mise à jour', failed: 'Le tirage a échoué' },
    push: { done: 'Branche poussée', failed: 'Le push a échoué' },
  } as const;

  async function sync(action: keyof typeof SYNC) {
    if (!result || busy) return;
    busy = action;
    try {
      const output = await git<string>('git_sync', { repo: result.repo, action });
      notify(true, SYNC[action].done, output);
    } catch (e) {
      notify(false, SYNC[action].failed, errorText(e));
    } finally {
      busy = '';
    }
    await load(result.repo, true);
  }

  // --- Diff -------------------------------------------------------------------------------

  let diffToken = 0;

  async function showDiff(file: OpenFile, fetch: () => Promise<Diff>) {
    const token = ++diffToken;
    openFile = file;
    diffLoading = true;
    diffError = '';
    try {
      const data = await fetch();
      if (token === diffToken) diff = data;
    } catch (e) {
      if (token === diffToken) {
        diff = null;
        diffError = errorText(e);
      }
    } finally {
      if (token === diffToken) diffLoading = false;
    }
  }

  function closeDiff() {
    diffToken++;
    openFile = null;
    diff = null;
    diffError = '';
    diffLoading = false;
  }

  function openWorkDiff(file: FileChange, staged: boolean) {
    const repo = result?.repo;
    if (!repo) return;
    diff = null;
    void showDiff(
      {
        key: `${staged ? 's' : 'u'}:${file.path}`,
        title: file.path,
        subtitle: `${staged ? 'Indexé' : 'Non indexé'}${file.oldPath ? ` · renommé depuis ${file.oldPath}` : ''}`,
      },
      () =>
        git<Diff>('git_diff_file', {
          repo,
          path: file.path,
          oldPath: file.oldPath,
          staged,
          untracked: file.kind === 'untracked',
        }),
    );
  }

  function openCommitDiff(file: FileChange) {
    const repo = result?.repo;
    const commit = current;
    if (!repo || !commit || commit.wip) return;
    diff = null;
    void showDiff(
      {
        key: `c:${file.path}`,
        title: file.path,
        subtitle: `${commit.sha.slice(0, 7)} · ${commit.subject}${file.oldPath ? ` · renommé depuis ${file.oldPath}` : ''}`,
      },
      () =>
        git<Diff>('git_diff_commit', {
          repo,
          sha: commit.sha,
          parent: commit.parents[0] ?? null,
          path: file.path,
          oldPath: file.oldPath,
        }),
    );
  }

  /** Après une modification de l'index : le diff ouvert suit son fichier, ou se ferme s'il a quitté sa liste. */
  function syncOpenDiff() {
    if (!openFile || !status || openFile.key.startsWith('c:')) return;
    const staged = openFile.key.startsWith('s:');
    const path = openFile.key.slice(2);
    const file = (staged ? status.staged : status.unstaged).find((f) => f.path === path);
    if (file) openWorkDiff(file, staged);
    else closeDiff();
  }

  // Détail du commit sélectionné, chargé après une courte pause pour ne pas lancer git
  // à chaque ligne survolée au clavier.
  let detailTimer = 0;
  $effect(() => {
    void currentKey;
    untrack(() => {
      clearTimeout(detailTimer);
      closeDiff();
      detail = null;
      detailError = '';
      const commit = current;
      const repo = result?.repo;
      if (!commit || commit.wip || !repo) return;
      const key = currentKey;
      detailTimer = window.setTimeout(async () => {
        try {
          const data = await git<CommitDetail>('git_commit_detail', {
            repo,
            sha: commit.sha,
            parent: commit.parents[0] ?? null,
          });
          if (key === currentKey) detail = data;
        } catch (e) {
          if (key === currentKey) detailError = errorText(e);
        }
      }, 120);
    });
  });

  function onViewKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && openFile) {
      e.preventDefault();
      closeDiff();
      scroller?.focus();
    }
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

    // Retour dans la fenêtre : les fichiers ont pu changer dans l'éditeur ou le terminal.
    const onFocus = () => {
      if (result && !loading && !busy) void refreshStatus();
    };
    window.addEventListener('focus', onFocus);

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
      window.removeEventListener('focus', onFocus);
      clearTimeout(noticeTimer);
      clearTimeout(detailTimer);
      if (raf) cancelAnimationFrame(raf);
    };
  });
</script>

{#snippet icon(name: 'local' | 'remote' | 'tag')}
  <svg class="ico" viewBox="0 0 16 16" aria-hidden="true">
    {#if name === 'local'}
      <rect x="3" y="3.5" width="10" height="7" rx="1" /><path d="M1.5 12.5h13" />
    {:else if name === 'remote'}
      <path d="M4.6 12.5a3 3 0 0 1-.5-5.96 4 4 0 0 1 7.72.9A2.55 2.55 0 0 1 11.4 12.5z" />
    {:else}
      <path d="M2.5 2.5h5.2l5.8 5.8-5.2 5.2-5.8-5.8z" /><circle cx="5.6" cy="5.6" r="0.6" />
    {/if}
  </svg>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="view" onkeydown={onViewKey}>
  <header class="toolbar">
    <div class="repo">
      <b>{result?.name ?? 'Aucun dépôt'}</b>
      <small class="mono">{result ? (result.branch ?? 'HEAD détaché') : 'ouvre un dossier pour commencer'}</small>
    </div>
    <button class="btn" type="button" onclick={pick} disabled={locked}>Ouvrir…</button>
    <button class="btn" type="button" onclick={reload} disabled={!result || locked}>Recharger</button>
    <div class="sync" role="group" aria-label="Synchronisation">
      <button class="btn" type="button" onclick={() => sync('fetch')} disabled={!result || locked} title="git fetch --all">
        {busy === 'fetch' ? 'Récupération…' : 'Récupérer'}
      </button>
      <button
        class="btn"
        type="button"
        onclick={() => sync('pull')}
        disabled={!status?.upstream || locked}
        title={status?.upstream ? `git pull depuis ${status.upstream}` : "Cette branche n'a pas de branche distante associée"}
      >
        {busy === 'pull' ? 'Tirage…' : 'Tirer'}{#if status?.behind}<span class="pill">↓ {status.behind}</span>{/if}
      </button>
      <button
        class="btn"
        type="button"
        onclick={() => sync('push')}
        disabled={!status?.branch || locked}
        title={status?.upstream ? `git push vers ${status.upstream}` : 'Publie la branche et la relie à son homologue distante'}
      >
        {busy === 'push' ? 'Envoi…' : status && status.branch && !status.upstream ? 'Publier' : 'Pousser'}{#if status?.ahead}<span class="pill">↑ {status.ahead}</span>{/if}
      </button>
    </div>
    <label class="limit">
      <span class="label">Commits</span>
      <select class="field" bind:value={limit} onchange={reload} disabled={locked}>
        <option value={1000}>1 000</option>
        <option value={10000}>10 000</option>
        <option value={50000}>50 000</option>
        <option value={0}>Tous</option>
      </select>
    </label>
  </header>

  {#if notice}
    <div class="notice" class:bad={!notice.ok} role={notice.ok ? 'status' : 'alert'}>
      <div>
        <b>{notice.title}</b>
        {#if notice.body}<pre class="mono">{notice.body}</pre>{/if}
      </div>
      <button class="btn" type="button" onclick={() => (notice = null)}>Fermer</button>
    </div>
  {/if}

  <div class="body">
    <div class="graph">
      {#if openFile}
        <DiffView title={openFile.title} subtitle={openFile.subtitle} {diff} loading={diffLoading} error={diffError} onclose={closeDiff} />
      {/if}
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
                  <span class="subject muted">{wipCount} fichier{wipCount > 1 ? 's' : ''} modifié{wipCount > 1 ? 's' : ''}</span>
                {:else}
                  {#each labels(commit) as label (label.kind + label.name)}
                    <span class="ref k-{label.kind}" style:--c="var(--l{commit.lane % 8})" title={label.title}>
                      <span class="ref-name">{label.kind === 'head' ? '✓ ' : ''}{label.name}</span>
                      {#if label.local}{@render icon('local')}{/if}
                      {#if label.remote}{@render icon('remote')}{/if}
                      {#if label.kind === 'tag'}{@render icon('tag')}{/if}
                    </span>
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

    <aside class="detail" class:working={current?.wip}>
      {#if current?.wip}
        {#if status}
          <WorkPanel
            {status}
            busy={locked}
            active={openFile && !openFile.key.startsWith('c:') ? openFile.key : null}
            bind:message
            onopen={openWorkDiff}
            onstage={stage}
            onunstage={unstage}
            ondiscard={discard}
            oncommit={commit}
          />
        {:else}
          <p class="muted">Lecture de l'état du dépôt…</p>
        {/if}
      {:else if current}
        {@const body = detail ? detail.message.split('\n').slice(1).join('\n').trim() : ''}
        <span class="label">Commit</span>
        <p class="title">{current.subject}</p>
        {#if body}<p class="message">{body}</p>{/if}
        <dl>
          <dt>SHA</dt>
          <dd class="mono sha-full">{current.sha}</dd>
          <dt>Auteur</dt>
          <dd>{current.author}</dd>
          <dt>Date</dt>
          <dd>{fullDateFmt.format(current.time * 1000)}</dd>
          <dt>Parent{current.parents.length > 1 ? 's' : ''}</dt>
          <dd class="mono">{current.parents.length ? current.parents.map((p) => p.slice(0, 7)).join(', ') : 'aucun (premier commit)'}</dd>
          {#if hasRemote}
            <dt>Emplacement</dt>
            <dd>{current.localOnly ? 'Local uniquement, pas encore poussé' : 'Présent sur le dépôt distant'}</dd>
          {/if}
          {#if current.refs.length}
            <dt>Références</dt>
            <dd>{current.refs.map((r) => r.name).join(', ')}</dd>
          {/if}
        </dl>
        <button class="btn" type="button" onclick={() => copySha(current.sha)}>{copied ? 'SHA copié' : 'Copier le SHA'}</button>
        {#if detailError}
          <p class="err">{detailError}</p>
        {:else if detail}
          <span class="label">
            Fichiers modifiés · {detail.files.length}{current.parents.length > 1 ? ' (par rapport au premier parent)' : ''}
          </span>
          {#if detail.files.length}
            <div class="files">
              <FileList files={detail.files} active={openFile?.key.startsWith('c:') ? openFile.key.slice(2) : null} onopen={openCommitDiff} />
            </div>
          {:else}
            <p class="muted">Ce commit ne modifie aucun fichier.</p>
          {/if}
        {:else}
          <p class="muted">Chargement des fichiers…</p>
        {/if}
      {:else}
        <span class="label">Détail</span>
        <p class="muted">Clique un commit, ou utilise les flèches du clavier dans la liste.</p>
        {#if wipCount > 0}
          <button class="btn" type="button" onclick={() => { selected = 0; reveal(0); }}>
            Voir les {wipCount} fichier{wipCount > 1 ? 's' : ''} modifié{wipCount > 1 ? 's' : ''}
          </button>
        {/if}
      {/if}
    </aside>
  </div>

  <footer class="status">
    <span class="mono branch">{result?.branch ? `⑂ ${result.branch}` : ''}</span>
    <button class="journal-btn mono" type="button" aria-expanded={showJournal} onclick={() => (showJournal = !showJournal)} disabled={!journal.length}>
      {lastCommand ? `$ ${lastCommand.command}` : 'Journal des commandes'}
    </button>
    <div class="stats" aria-live="polite">
      {#if loading}
        <span>Chargement…</span>
      {:else if result}
        {@const count = commits.length - (commits[0]?.wip ? 1 : 0)}
        <span><b>{numFmt.format(count)}</b> commit{count > 1 ? 's' : ''}{result.truncated ? ' (tronqué)' : ''}</span>
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
    {#if showJournal && result}
      <div class="journal">
        <span class="label">Journal des commandes</span>
        {#each [...journal].reverse() as entry, i (i)}
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
    grid-template-rows: auto auto minmax(0, 1fr) 26px;
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
  .sync {
    display: flex;
    gap: 6px;
    padding-left: 10px;
    border-left: 1px solid var(--line);
  }
  .pill {
    margin-left: 6px;
    padding: 0 5px;
    border-radius: 8px;
    background: var(--sel);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .notice {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--sel);
    min-width: 0;
  }
  .notice.bad {
    background: var(--bad-soft);
  }
  .notice div {
    min-width: 0;
  }
  .notice pre {
    margin: 4px 0 0;
    font-size: 11.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 140px;
    overflow: auto;
  }
  .stats {
    margin-left: auto;
    display: flex;
    gap: 14px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    min-width: 0;
  }
  .stats b {
    color: var(--ink);
    font-weight: 600;
  }

  .limit {
    margin-left: auto;
  }
  .body {
    grid-row: 3;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 330px;
    min-height: 0;
  }
  .status {
    grid-row: 4;
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
  .ico {
    flex: none;
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ref-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ref {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 220px;
    overflow: hidden;
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
  .detail.working {
    align-items: stretch;
    overflow: hidden;
  }
  .message {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--muted);
  }
  .files {
    width: calc(100% + 12px);
    margin: -4px -6px 0;
  }
  .err {
    color: var(--bad);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
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
