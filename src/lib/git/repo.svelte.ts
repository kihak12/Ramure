// État d'un dépôt ouvert et toutes les opérations qu'on peut lui appliquer.
// Les composants lisent cet état et appellent ces méthodes ; aucun n'invoque le cœur Rust lui-même.

import { invoke } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import { toasts } from '../ui/toasts.svelte';
import { revealDetail } from './layout.svelte';
import { prefs, type Settings } from './prefs.svelte';
import { RepoTerminals } from './terminals.svelte';
import type {
  CommandLog,
  CommitDetail,
  ConflictFile,
  Diff,
  Done,
  FileChange,
  HistoryOp,
  LogResult,
  Operation,
  Outcome,
  RebasePlan,
  RefOp,
  Refs,
  Status,
  TodoItem,
} from './types';

const JOURNAL_MAX = 300; // commandes conservées dans le journal

export interface OpenFile {
  /** `u:chemin`, `s:chemin` (répertoire de travail), `c:chemin` (commit) ou `stash:n`. */
  key: string;
  title: string;
  subtitle: string;
  /** Lignes sélectionnables, pour indexer ou retirer une partie du fichier. */
  selectable: 'stage' | 'unstage' | null;
}

export type SyncAction = 'fetch' | 'pull' | 'push' | 'push-force';

const SYNC: Record<SyncAction, { done: string; failed: string }> = {
  fetch: { done: 'Récupération terminée', failed: 'La récupération a échoué' },
  pull: { done: 'Branche mise à jour', failed: 'Le tirage a échoué' },
  push: { done: 'Branche poussée', failed: 'Le push a échoué' },
  'push-force': { done: 'Branche poussée (forcé)', failed: 'Le push forcé a échoué' },
};

export const OPERATION: Record<Operation, string> = {
  merge: 'Fusion',
  rebase: 'Rebase',
  'cherry-pick': 'Cherry-pick',
  revert: 'Revert',
};

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class Repo {
  // `$state.raw` : 50 000 lignes ne doivent pas être enveloppées dans des proxys réactifs.
  result = $state.raw<LogResult | null>(null);
  status = $state.raw<Status | null>(null);
  refs = $state.raw<Refs | null>(null);
  journal = $state.raw<CommandLog[]>([]);
  loading = $state(false);
  /** Opération en cours (`fetch`, `commit`, `index`…) ; vide quand le dépôt est libre. */
  busy = $state('');
  /** Branche visée par le basculement en cours, pour montrer l'attente là où il a été demandé. */
  switching = $state<string | null>(null);
  error = $state('');
  selected = $state(-1);
  perf = $state({ ipc: 0, paint: 0 });
  /** Dossier demandé, puis racine du dépôt une fois celui-ci ouvert ; vide pour un onglet neuf. */
  path = $state('');
  /** Augmente à chaque ouverture ou rechargement complet : le graphe rejoue son apparition. */
  loadCount = $state(0);
  lastFetch = $state<{ at: number; ok: boolean } | null>(null);
  /** Fluidité du dernier défilement du graphe, mesurée par celui-ci. */
  frames = $state<{ avg: number; worst: number } | null>(null);

  message = $state('');
  amend = $state(false);
  detail = $state.raw<CommitDetail | null>(null);
  detailError = $state('');
  openFile = $state<OpenFile | null>(null);
  diff = $state.raw<Diff | null>(null);
  diffLoading = $state(false);
  diffError = $state('');
  /** Fichier ouvert dans l'éditeur de conflits. */
  conflictFile = $state.raw<FileChange | null>(null);
  /** Plan d'un rebase interactif en préparation ; la boîte de dialogue s'ouvre quand il existe. */
  rebasePlan = $state.raw<RebasePlan | null>(null);
  /** Terminaux locaux ouverts dans le dossier du dépôt, sous le graphe (charte, section 9.20). */
  readonly terminals = new RepoTerminals();

  /** Posé par l'espace de travail : prévenu chaque fois qu'un dépôt vient d'être ouvert. */
  onopened: (repo: Repo) => void = () => {};
  /** Posés par le graphe : amener une ligne à l'écran, revenir en haut. */
  reveal: (index: number) => void = () => {};
  resetScroll: () => void = () => {};

  /** Réglages, communs à tous les dépôts ouverts. */
  get settings(): Settings {
    return prefs;
  }

  commits = $derived(this.result?.commits ?? []);
  current = $derived(this.selected >= 0 ? (this.commits[this.selected] ?? null) : null);
  /** Clé du commit sélectionné : ne change pas quand l'historique est simplement rechargé. */
  currentKey = $derived(this.current ? `${this.current.sha}|${this.current.parents[0] ?? ''}` : '');
  locked = $derived(this.loading || this.busy !== '');
  /** Sans branche distante connue, distinguer « local » de « distant » n'a pas de sens. */
  hasRemote = $derived(this.commits.some((c) => c.refs.some((r) => r.kind === 'remote')));
  shortRemotes = $derived((this.refs?.remoteNames.length ?? 1) <= 1);
  wipCount = $derived(
    this.status
      ? new Set([...this.status.staged, ...this.status.unstaged].map((f) => f.path)).size
      : (this.result?.wipFiles ?? 0),
  );
  /** Libellé du bouton qui mène aux modifications en cours. */
  wipText = $derived(this.wipCount > 1 ? `Voir les ${this.wipCount} fichiers modifiés` : 'Voir le fichier modifié');
  conflicts = $derived(this.status?.unstaged.filter((f) => f.kind === 'conflict') ?? []);
  /** Commit sur lequel se trouve HEAD. */
  head = $derived(
    this.commits.find((c) => !c.wip && c.refs.some((r) => r.kind === 'head' || r.kind === 'detached')) ?? null,
  );
  branch = $derived(this.status?.branch ?? this.result?.branch ?? null);

  // Commit courant au moment du dernier chargement de l'historique ; `undefined` tant qu'il est inconnu.
  #logHead: string | null | undefined;
  #opened = Date.now();
  #diffToken = 0;
  #detailTimer = 0;

  // --- Appels au cœur Rust ----------------------------------------------------------------

  record(commands: CommandLog[]) {
    if (commands.length) this.journal = [...this.journal, ...commands].slice(-JOURNAL_MAX);
  }

  /** Appelle une commande git du cœur Rust et verse ses commandes au journal, réussite ou non. */
  async git<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    try {
      const reply = await invoke<Done<T>>(command, { repo: this.result?.repo, ...args });
      this.record(reply.commands);
      return reply.data;
    } catch (e) {
      if (e && typeof e === 'object' && 'message' in e) {
        const failed = e as { message: string; commands?: CommandLog[] };
        this.record(failed.commands ?? []);
        throw new Error(failed.message);
      }
      throw new Error(typeof e === 'string' ? e : String(e));
    }
  }

  // --- Chargement -------------------------------------------------------------------------

  open(path: string) {
    return this.load(path, false);
  }

  /** Recharge l'historique en conservant la sélection et la position. */
  async reload() {
    if (this.result) await this.load(this.result.repo, true);
  }

  async load(path: string, keep: boolean) {
    if (!path || this.loading) return;
    this.loading = true;
    this.error = '';
    if (!keep) this.path = path;
    const previous = keep ? this.current : null;
    const limit = this.settings.limit > 0 ? this.settings.limit : null;
    const t0 = performance.now();
    try {
      const data = await invoke<LogResult>('git_log', { repo: path, limit });
      const t1 = performance.now();
      if (data.repo !== this.result?.repo) {
        this.status = null;
        this.refs = null;
        this.message = '';
        this.amend = false;
        this.lastFetch = null;
        this.#opened = Date.now();
        this.closeDiff();
        this.conflictFile = null;
      }
      this.result = data;
      this.record(data.commands);
      if (previous?.wip) this.selected = data.commits[0]?.wip ? 0 : -1;
      else if (previous) this.selected = data.commits.findIndex((c) => c.sha === previous.sha);
      else this.selected = -1;
      if (!keep) {
        this.resetScroll();
        this.loadCount++;
        this.path = data.repo;
        this.onopened(this);
      }
      this.#logHead = undefined;
      void this.refreshStatus(true);
      void this.refreshRefs();
      await tick();
      // Deux images plus tard, le graphe est réellement à l'écran.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => (this.perf = { ipc: t1 - t0, paint: performance.now() - t0 })),
      );
    } catch (e) {
      const text = typeof e === 'string' ? e : String(e);
      if (keep && this.result) {
        toasts.err("Impossible de relire l'historique", text);
      } else {
        this.error = text;
        this.result = null;
        this.path = '';
      }
    } finally {
      this.loading = false;
    }
  }

  /** Enregistre un nouvel état ; recharge l'historique s'il ne lui correspond plus. */
  applyStatus(next: Status, fromLoad = false) {
    this.status = next;
    // Le fichier ouvert dans l'éditeur de conflits a pu être résolu ou abandonné entre-temps.
    const editing = this.conflictFile?.path;
    if (editing && !next.unstaged.some((f) => f.kind === 'conflict' && f.path === editing)) this.conflictFile = null;
    if (fromLoad || this.#logHead === undefined) {
      this.#logHead = next.head;
      return;
    }
    const dirty = next.staged.length + next.unstaged.length > 0;
    if (next.head !== this.#logHead || dirty !== !!this.commits[0]?.wip) void this.reload();
  }

  async refreshStatus(fromLoad = false) {
    const repo = this.result?.repo;
    if (!repo) return;
    try {
      const next = await this.git<Status>('git_status');
      if (repo === this.result?.repo) this.applyStatus(next, fromLoad);
    } catch (e) {
      if (!fromLoad) toasts.err("Impossible de lire l'état du dépôt", errorText(e));
    }
  }

  async refreshRefs() {
    const repo = this.result?.repo;
    if (!repo) return;
    try {
      const next = await this.git<Refs>('git_refs');
      if (repo === this.result?.repo) this.refs = next;
    } catch {
      /* la barre latérale garde sa dernière liste ; l'erreur est dans le journal */
    }
  }

  /** Sélectionne le commit `sha` et l'amène à l'écran, s'il est chargé. */
  selectSha(sha: string) {
    const index = this.commits.findIndex((c) => c.sha === sha);
    if (index < 0) {
      toasts.warn('Commit hors de la partie chargée', "Augmente le nombre de commits chargés dans les réglages pour l'atteindre.");
      return;
    }
    this.selected = index;
    this.reveal(index);
  }

  /**
   * Sélectionne les modifications en cours. C'est une demande de les voir : le panneau de détail
   * revient s'il était masqué. `quiet` : l'appel ne vient pas de l'utilisateur, le panneau reste
   * comme il l'a laissé (charte, section 9.17).
   */
  showWip(quiet = false) {
    if (this.commits[0]?.wip) {
      this.selected = 0;
      this.reveal(0);
      if (!quiet) revealDetail();
    }
  }

  // --- Index et commit --------------------------------------------------------------------

  /** Lance une commande qui modifie l'index et renvoie le nouvel état. */
  async #mutate(command: string, args: Record<string, unknown>) {
    if (!this.result || this.busy) return;
    this.busy = 'index';
    try {
      this.applyStatus(await this.git<Status>(command, args));
      this.#syncOpenDiff();
    } catch (e) {
      toasts.err("L'opération a échoué", errorText(e));
      void this.refreshStatus();
    } finally {
      this.busy = '';
    }
  }

  stage(files: FileChange[]) {
    return this.#mutate('git_stage', { paths: files.map((f) => f.path) });
  }

  /** Retirer un renommage de l'index concerne ses deux chemins. */
  unstage(files: FileChange[]) {
    return this.#mutate('git_unstage', { paths: files.flatMap((f) => (f.oldPath ? [f.oldPath, f.path] : [f.path])) });
  }

  discard(files: FileChange[]) {
    return this.#mutate('git_discard', {
      tracked: files.filter((f) => f.kind !== 'untracked').map((f) => f.path),
      untracked: files.filter((f) => f.kind === 'untracked').map((f) => f.path),
    });
  }

  /** Indexe ou retire de l'index les lignes choisies dans le diff ouvert. */
  applyLines(lines: number[]) {
    if (!this.diff || !this.openFile?.selectable) return Promise.resolve();
    return this.#mutate('git_apply_lines', {
      diff: this.diff.text,
      lines,
      staged: this.openFile.selectable === 'unstage',
    });
  }

  /** Résout un conflit en gardant un seul côté. */
  resolve(file: FileChange, side: 'ours' | 'theirs') {
    return this.#mutate('git_resolve', { path: file.path, side });
  }

  /** Ouvre un fichier en conflit dans l'éditeur à trois voies. */
  openConflict(file: FileChange) {
    this.closeDiff();
    this.conflictFile = file;
  }

  /** Versions d'un fichier en conflit : ancêtre, les deux côtés, et le fichier avec ses marqueurs. */
  readConflict(file: FileChange) {
    return this.git<ConflictFile>('git_conflict_read', { path: file.path });
  }

  /** Écrit le résultat de la résolution et marque le fichier comme résolu. */
  async saveConflict(file: FileChange, content: string) {
    if (!this.result || this.busy) return false;
    this.busy = 'index';
    try {
      this.applyStatus(await this.git<Status>('git_conflict_write', { path: file.path, content }));
      this.conflictFile = null;
      toasts.ok('Conflit résolu', file.path);
      return true;
    } catch (e) {
      toasts.err("Le fichier n'a pas pu être enregistré", errorText(e));
      return false;
    } finally {
      this.busy = '';
    }
  }

  async toggleAmend(on: boolean) {
    this.amend = on;
    if (!on || this.message.trim() || !this.result) return;
    try {
      this.message = await this.git<string>('git_head_message');
    } catch {
      /* le champ reste vide : l'utilisateur saisit son message */
    }
  }

  async commit() {
    if (!this.result || this.busy) return;
    this.busy = 'commit';
    try {
      await this.git<null>('git_commit', { message: this.message, amend: this.amend });
      toasts.ok(this.amend ? 'Dernier commit modifié' : 'Commit créé');
      this.message = '';
      this.amend = false;
      this.closeDiff();
    } catch (e) {
      toasts.err('Le commit a échoué', errorText(e));
    } finally {
      this.busy = '';
    }
    await this.reload();
    // La ligne WIP a disparu : on montre le commit qui vient d'être créé.
    if (this.selected < 0 && this.commits.length) this.selected = 0;
  }

  // --- Synchronisation, références, historique --------------------------------------------

  #settle(outcome: Outcome, done: string) {
    if (outcome.stopped) {
      toasts.warn(`${OPERATION[outcome.stopped]} en suspens : des conflits sont à résoudre`, outcome.output);
    } else {
      toasts.ok(done, outcome.output);
    }
  }

  /** Exécute une opération longue, affiche son résultat, puis recharge l'historique. */
  async #task(label: string, failed: string, action: () => Promise<Outcome | string>, done: string) {
    if (!this.result || this.busy) return false;
    this.busy = label;
    let ok = false;
    let stopped = false;
    try {
      const value = await action();
      if (typeof value === 'string') {
        toasts.ok(done, value);
      } else {
        this.#settle(value, done);
        stopped = !!value.stopped;
      }
      ok = true;
    } catch (e) {
      toasts.err(failed, errorText(e));
    } finally {
      this.busy = '';
    }
    await this.reload();
    if (stopped) this.showWip(true);
    return ok;
  }

  /** `quiet` : fetch automatique, sans notification ; l'échec se lit dans la barre d'état. */
  async sync(action: SyncAction, quiet = false) {
    if (!this.result || this.busy) return;
    if (!quiet) {
      await this.#task(action, SYNC[action].failed, async () => {
        try {
          const outcome = await this.git<Outcome>('git_sync', { action, pullMode: this.settings.pullMode });
          if (action === 'fetch') this.lastFetch = { at: Date.now(), ok: true };
          return outcome;
        } catch (e) {
          if (action === 'fetch') this.lastFetch = { at: Date.now(), ok: false };
          throw e;
        }
      }, SYNC[action].done);
      return;
    }
    this.busy = action;
    let changed = false;
    try {
      const outcome = await this.git<Outcome>('git_sync', { action });
      this.lastFetch = { at: Date.now(), ok: true };
      changed = outcome.output !== '';
    } catch {
      this.lastFetch = { at: Date.now(), ok: false };
    } finally {
      this.busy = '';
    }
    // Rien de neuf : inutile de redessiner le graphe.
    if (changed) await this.reload();
  }

  async refOp(op: RefOp, done: string, failed = "L'opération a échoué") {
    if (this.busy) return false;
    if (op.op === 'switch' || op.op === 'switch-remote') this.switching = op.name;
    try {
      return await this.#task(op.op, failed, () => this.git<string>('git_ref_op', { op }), done);
    } finally {
      this.switching = null;
    }
  }

  history(op: HistoryOp, done: string, failed = "L'opération a échoué") {
    return this.#task(op.op, failed, () => this.git<Outcome>('git_history', { op }), done);
  }

  /** Continue, passe ou abandonne l'opération en suspens. */
  operate(action: 'continue' | 'skip' | 'abort') {
    const name = this.status?.operation ? OPERATION[this.status.operation] : 'Opération';
    const done = action === 'abort' ? `${name} abandonné(e)` : `${name} terminé(e)`;
    return this.#task(action, "L'opération n'a pas pu avancer", () => this.git<Outcome>('git_operate', { action }), done);
  }

  /** Prépare un rebase interactif de `sha` jusqu'à HEAD ; la boîte de dialogue prend le relais. */
  async planRebase(sha: string) {
    if (!this.result || this.busy) return;
    try {
      this.rebasePlan = await this.git<RebasePlan>('git_rebase_plan', { sha });
    } catch (e) {
      toasts.err('Rebase interactif impossible', errorText(e));
    }
  }

  rebaseInteractive(base: string | null, todo: TodoItem[]) {
    return this.#task(
      'rebase',
      'Le rebase interactif a échoué',
      () => this.git<Outcome>('git_rebase_interactive', { base, todo }),
      'Historique réécrit',
    );
  }

  /** Démarre le fetch automatique ; renvoie la fonction qui l'arrête. */
  startAutoFetch() {
    const timer = window.setInterval(() => {
      const every = this.settings.autoFetch;
      if (!every || !this.result || this.locked || document.hidden) return;
      if (!this.refs?.remoteNames.length) return;
      const last = this.lastFetch?.at ?? this.#opened;
      if (Date.now() - last >= every * 60_000) void this.sync('fetch', true);
    }, 20_000);
    return () => window.clearInterval(timer);
  }

  // --- Diff -------------------------------------------------------------------------------

  async #showDiff(file: OpenFile, fetch: () => Promise<Diff>) {
    const token = ++this.#diffToken;
    if (this.openFile?.key !== file.key) this.diff = null;
    this.openFile = file;
    this.diffLoading = true;
    this.diffError = '';
    try {
      const data = await fetch();
      if (token === this.#diffToken) this.diff = data;
    } catch (e) {
      if (token === this.#diffToken) {
        this.diff = null;
        this.diffError = errorText(e);
      }
    } finally {
      if (token === this.#diffToken) this.diffLoading = false;
    }
  }

  closeDiff() {
    this.#diffToken++;
    this.openFile = null;
    this.diff = null;
    this.diffError = '';
    this.diffLoading = false;
  }

  openWorkDiff(file: FileChange, staged: boolean) {
    // L'indexation partielle ne vaut que pour un fichier texte modifié.
    const partial = file.kind === 'modified';
    void this.#showDiff(
      {
        key: `${staged ? 's' : 'u'}:${file.path}`,
        title: file.path,
        subtitle: `${staged ? 'Indexé' : 'Non indexé'}${file.oldPath ? ` · renommé depuis ${file.oldPath}` : ''}`,
        selectable: partial ? (staged ? 'unstage' : 'stage') : null,
      },
      () =>
        this.git<Diff>('git_diff_file', {
          path: file.path,
          oldPath: file.oldPath,
          staged,
          untracked: file.kind === 'untracked',
        }),
    );
  }

  openCommitDiff(file: FileChange) {
    const commit = this.current;
    if (!commit || commit.wip) return;
    void this.#showDiff(
      {
        key: `c:${file.path}`,
        title: file.path,
        subtitle: `${commit.sha.slice(0, 7)} · ${commit.subject}${file.oldPath ? ` · renommé depuis ${file.oldPath}` : ''}`,
        selectable: null,
      },
      () =>
        this.git<Diff>('git_diff_commit', {
          sha: commit.sha,
          parent: commit.parents[0] ?? null,
          path: file.path,
          oldPath: file.oldPath,
        }),
    );
  }

  openStashDiff(index: number, message: string) {
    void this.#showDiff(
      { key: `stash:${index}`, title: `Remise ${index}`, subtitle: message, selectable: null },
      () => this.git<Diff>('git_stash_show', { index }),
    );
  }

  /** Après une modification de l'index : le diff ouvert suit son fichier, ou se ferme s'il a quitté sa liste. */
  #syncOpenDiff() {
    const key = this.openFile?.key;
    if (!key || !this.status || !(key.startsWith('u:') || key.startsWith('s:'))) return;
    const staged = key.startsWith('s:');
    const path = key.slice(2);
    const file = (staged ? this.status.staged : this.status.unstaged).find((f) => f.path === path);
    if (file) this.openWorkDiff(file, staged);
    else this.closeDiff();
  }

  // --- Détail du commit sélectionné -------------------------------------------------------

  /**
   * À appeler quand la sélection change. Le chargement attend une courte pause, pour ne pas
   * lancer git à chaque ligne survolée au clavier.
   */
  loadDetail() {
    window.clearTimeout(this.#detailTimer);
    this.closeDiff();
    this.detail = null;
    this.detailError = '';
    const commit = this.current;
    if (!commit || commit.wip || !this.result) return;
    const key = this.currentKey;
    this.#detailTimer = window.setTimeout(async () => {
      try {
        const data = await this.git<CommitDetail>('git_commit_detail', {
          sha: commit.sha,
          parent: commit.parents[0] ?? null,
        });
        if (key === this.currentKey) this.detail = data;
      } catch (e) {
        if (key === this.currentKey) this.detailError = errorText(e);
      }
    }, 120);
  }
}
