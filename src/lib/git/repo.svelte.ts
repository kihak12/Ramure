// State of an open repository and every operation that can be applied to it.
// The components read this state and call these methods; none of them invokes the Rust core itself.

import { invoke } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import type { EditorSnapshot } from './conflictWindow';
import { openInWindow, operationEnded } from './editorWindow.svelte';
import { focusBack, focusSoon, usable as visibleOne } from '../ui/focus';
import { besideList } from './fileLists';
import { focusWithRing } from '../ui/focus';
import { toasts } from '../ui/toasts.svelte';
import { revealDetail } from './layout.svelte';
import { prefs, type Settings } from './prefs.svelte';
import { RepoTerminals } from './terminals.svelte';
import type {
  CommandLog,
  CommitDetail,
  CommitRow,
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
  Reworded,
  Status,
  TodoItem,
} from './types';

const JOURNAL_MAX = 300; // commands kept in the command log

export interface OpenFile {
  /** `u:path`, `s:path` (working directory), `c:path` (commit) or `stash:n`. */
  key: string;
  title: string;
  subtitle: string;
  /** Selectable lines, to stage or unstage part of the file. */
  selectable: 'stage' | 'unstage' | null;
}

export type SyncAction = 'fetch' | 'pull' | 'push' | 'push-force';

const SYNC: Record<SyncAction, { done: string; failed: string }> = {
  fetch: { done: 'Fetch complete', failed: 'Fetch failed' },
  pull: { done: 'Branch updated', failed: 'Pull failed' },
  push: { done: 'Branch pushed', failed: 'Push failed' },
  'push-force': { done: 'Branch force-pushed', failed: 'Force push failed' },
};

export const OPERATION: Record<Operation, string> = {
  merge: 'Merge',
  rebase: 'Rebase',
  'cherry-pick': 'Cherry-pick',
  revert: 'Revert',
};

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
/** A push refused because the branch and its remote counterpart have diverged, after a rebase, an
 *  amend or a reword: git says so in English (src-tauri/src/git/work.rs, `sync`). A force push is
 *  then offered rather than an error (guidelines, 9.2). */
const REJECTED = /\(non-fast-forward\)/;
/** A push rejected because the remote has commits that were not fetched: say what to do. */
const FETCH_FIRST = /\(fetch first\)/;
const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/** What rewording a commit would do, or why it cannot be done (guidelines, 9.25). */
export interface RewordCheck {
  /** Why it cannot be done; empty when it can. */
  reason: string;
  /** The commit is HEAD: only its message is amended. */
  head: boolean;
  /** The commits after it, replayed with new SHAs. */
  replayed: number;
  /** It is on a remote: a force push will be needed. */
  pushed: boolean;
}

/** What opened a diff: the row, its list and its place in it, and the view of the repository. */
type Opener = { element: HTMLElement; list: HTMLElement | null; at: number; view: Element | null; panel: Element | null };

/** The rows of a list of files, in order: where a row that left it is replaced. */
const rowsOf = (list: HTMLElement | null) => (list?.matches('ul.files') ? [...list.querySelectorAll<HTMLElement>('button.open')] : []);

export class Repo {
  // `$state.raw`: 50,000 rows must not be wrapped in reactive proxies.
  result = $state.raw<LogResult | null>(null);
  status = $state.raw<Status | null>(null);
  refs = $state.raw<Refs | null>(null);
  journal = $state.raw<CommandLog[]>([]);
  loading = $state(false);
  /** Operation running (`fetch`, `commit`, `index`…); empty when the repository is free. */
  busy = $state('');
  /** Branch targeted by the switch in progress, to show the wait where it was asked for. */
  switching = $state<string | null>(null);
  error = $state('');
  selected = $state(-1);
  perf = $state({ ipc: 0, paint: 0 });
  /** Folder asked for, then root of the repository once it is open; empty for a new tab. */
  path = $state('');
  /** Goes up at each opening or full reload: the graph plays its entrance again. */
  loadCount = $state(0);
  lastFetch = $state<{ at: number; ok: boolean } | null>(null);
  /** Smoothness of the last scroll of the graph, measured by the graph. */
  frames = $state<{ avg: number; worst: number } | null>(null);

  message = $state('');
  amend = $state(false);
  detail = $state.raw<CommitDetail | null>(null);
  /** The commit `detail` belongs to (`currentKey` when it was read). */
  detailFor = $state('');
  detailError = $state('');
  openFile = $state<OpenFile | null>(null);
  diff = $state.raw<Diff | null>(null);
  /** The lines picked in the open diff of the working tree, not staged yet: the commit box says
   *  so, and can stage them (guidelines, 9.14). */
  picked = $state.raw<number[]>([]);
  diffLoading = $state(false);
  diffError = $state('');
  /** File open in the conflict editor. */
  conflictFile = $state.raw<FileChange | null>(null);
  /** The window of the conflict editor, while it is open: the file it has, and whether choices
   *  are not saved there (guidelines, 9.26, "Its own window"). */
  editorWindow = $state.raw<{ label: string; file: string | null; modified: boolean } | null>(null);
  /** The editor went to its window during this operation: the next files open there too. */
  preferWindow = false;
  /** What the editor carries back from its window, read once as it opens in the main window. */
  restoreEditor: EditorSnapshot | null = null;
  /** In the window of the editor, the page decides what follows a file that left the conflicts:
   *  the next file, or the strip that keeps its choices (guidelines, 9.26, "Its ends"). */
  holdConflictFile = false;
  /** Moves the editor open in the main window to its window: set by the view while it is open. */
  moveEditor: (() => void) | null = null;
  /** Plan of an interactive rebase being prepared; the dialog opens when it exists. */
  rebasePlan = $state.raw<RebasePlan | null>(null);
  /** The message being reworded in the detail panel (guidelines, 9.25), kept as a draft while
   *  another commit is looked at. `text` is null until the whole message is known. */
  reword = $state<{ sha: string; text: string | null; original: string } | null>(null);
  /** Where the focus goes back when the rewording ends: the row of the graph, or the button. */
  #rewordFrom: HTMLElement | null = null;
  /** Counts the requests to reword: each one gives the keyboard to the field, a draft kept too. */
  rewordAsk = $state(0);
  /** The last request that the field has answered by taking the keyboard. */
  rewordServed = 0;
  /** Counts the requests to reword a commit that cannot be: its reason is brought forward. */
  rewordRefused = $state(0);
  /** Local terminals opened in the folder of the repository, under the graph (guidelines, section 9.20). */
  readonly terminals = new RepoTerminals();

  /** Set by the workspace: told each time a repository has just been opened. */
  onopened: (repo: Repo) => void = () => {};
  /** Set by the graph: bring a row into view, go back to the top. */
  reveal: (index: number) => void = () => {};
  resetScroll: () => void = () => {};

  /** Settings, shared by all the open repositories. */
  get settings(): Settings {
    return prefs;
  }

  commits = $derived(this.result?.commits ?? []);
  current = $derived(this.selected >= 0 ? (this.commits[this.selected] ?? null) : null);
  /** Key of the selected commit: does not change when the history is merely reloaded. */
  currentKey = $derived(this.current ? `${this.current.sha}|${this.current.parents[0] ?? ''}` : '');
  locked = $derived(this.loading || this.busy !== '');
  /** With no known remote branch, telling “local” from “remote” makes no sense. */
  hasRemote = $derived(this.commits.some((c) => c.refs.some((r) => r.kind === 'remote')));
  shortRemotes = $derived((this.refs?.remoteNames.length ?? 1) <= 1);
  wipCount = $derived(
    this.status
      ? new Set([...this.status.staged, ...this.status.unstaged].map((f) => f.path)).size
      : (this.result?.wipFiles ?? 0),
  );
  conflicts = $derived(this.status?.unstaged.filter((f) => f.kind === 'conflict') ?? []);
  /** Commit HEAD is on. */
  head = $derived(
    this.commits.find((c) => !c.wip && c.refs.some((r) => r.kind === 'head' || r.kind === 'detached')) ?? null,
  );
  branch = $derived(this.status?.branch ?? this.result?.branch ?? null);
  /** The loaded commits that are on the current branch: reachable from HEAD. */
  onBranch = $derived.by(() => {
    const reached = new Set<string>();
    if (!this.head) return reached;
    const bySha = new Map(this.commits.map((c) => [c.sha, c]));
    const pending = [this.head.sha];
    while (pending.length) {
      const sha = pending.pop()!;
      if (reached.has(sha)) continue;
      reached.add(sha);
      const commit = bySha.get(sha);
      if (commit) pending.push(...commit.parents);
    }
    return reached;
  });

  // Current commit when the history was last loaded; `undefined` while it is unknown.
  #logHead: string | null | undefined;
  #opened = Date.now();
  #diffToken = 0;
  #detailTimer = 0;

  // --- Calls to the Rust core -------------------------------------------------------------

  record(commands: CommandLog[], readsDiff = false) {
    if (readsDiff) commands = commands.map((c) => ({ ...c, readsDiff }));
    if (commands.length) this.journal = [...this.journal, ...commands].slice(-JOURNAL_MAX);
  }

  /**
   * Calls a git command of the Rust core and adds its commands to the log, success or not.
   * `readsDiff`: the command only reads a diff to show it, which the status bar does not name.
   */
  async git<T>(command: string, args: Record<string, unknown> = {}, readsDiff = false): Promise<T> {
    try {
      const reply = await invoke<Done<T>>(command, { repo: this.result?.repo, ...args });
      this.record(reply.commands, readsDiff);
      return reply.data;
    } catch (e) {
      if (e && typeof e === 'object' && 'message' in e) {
        const failed = e as { message: string; commands?: CommandLog[] };
        this.record(failed.commands ?? [], readsDiff);
        throw new Error(failed.message);
      }
      throw new Error(typeof e === 'string' ? e : String(e));
    }
  }

  // --- Loading ----------------------------------------------------------------------------

  open(path: string) {
    return this.load(path, false);
  }

  /** Reloads the history, keeping the selection and the position. */
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
      // A draft of a commit that is no longer there goes with it.
      if (this.reword && !data.commits.some((c) => c.sha === this.reword?.sha)) this.reword = null;
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
      // Two frames later, the graph is really on screen.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => (this.perf = { ipc: t1 - t0, paint: performance.now() - t0 })),
      );
    } catch (e) {
      const text = typeof e === 'string' ? e : String(e);
      if (keep && this.result) {
        toasts.err('Could not reload history', text);
      } else {
        this.error = text;
        this.result = null;
        this.path = '';
      }
    } finally {
      this.loading = false;
    }
  }

  /** Records a new status; reloads the history if it no longer matches it. */
  applyStatus(next: Status, fromLoad = false) {
    const ended = !!this.status?.operation && !next.operation;
    this.status = next;
    // The operation is over: its window closes, or warns of the choices it holds (9.26).
    if (ended) operationEnded(this);
    // The file open in the conflict editor may have been resolved or discarded in the meantime.
    const editing = this.conflictFile?.path;
    if (editing && !this.holdConflictFile && !next.unstaged.some((f) => f.kind === 'conflict' && f.path === editing)) this.conflictFile = null;
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
      if (!fromLoad) toasts.err('Could not read repository status', errorText(e));
    }
  }

  async refreshRefs() {
    const repo = this.result?.repo;
    if (!repo) return;
    try {
      const next = await this.git<Refs>('git_refs');
      if (repo === this.result?.repo) this.refs = next;
    } catch {
      /* the sidebar keeps its last list; the error is in the command log */
    }
  }

  /** Selects the commit `sha` and brings it into view, if it is loaded. */
  selectSha(sha: string) {
    const index = this.commits.findIndex((c) => c.sha === sha);
    if (index < 0) {
      toasts.warn('Commit not loaded', 'Load more commits in the settings to reach it.');
      return;
    }
    this.selected = index;
    this.reveal(index);
  }

  /**
   * Selects the working changes. It is a request to see them: the detail panel comes back if it
   * was hidden. `quiet`: the call does not come from the user, the panel stays as they left it
   * (guidelines, section 9.17).
   */
  showWip(quiet = false) {
    if (this.commits[0]?.wip) {
      this.selected = 0;
      this.reveal(0);
      if (!quiet) revealDetail();
    }
  }

  // --- Index and commit -------------------------------------------------------------------

  /** Runs a command that changes the index and returns the new status. */
  async #mutate(command: string, args: Record<string, unknown>) {
    if (!this.result || this.busy) return;
    this.busy = 'index';
    try {
      this.applyStatus(await this.git<Status>(command, args));
      this.#syncOpenDiff();
    } catch (e) {
      toasts.err('Operation failed', errorText(e));
      void this.refreshStatus();
    } finally {
      this.busy = '';
    }
  }

  stage(files: FileChange[]) {
    return this.#mutate('git_stage', { paths: files.map((f) => f.path) });
  }

  /** Unstaging a rename concerns both its paths. */
  unstage(files: FileChange[]) {
    return this.#mutate('git_unstage', { paths: files.flatMap((f) => (f.oldPath ? [f.oldPath, f.path] : [f.path])) });
  }

  discard(files: FileChange[]) {
    return this.#mutate('git_discard', {
      tracked: files.filter((f) => f.kind !== 'untracked').map((f) => f.path),
      untracked: files.filter((f) => f.kind === 'untracked').map((f) => f.path),
    });
  }

  /** The lines picked in the open diff: only those to stage concern the commit. */
  pick(lines: number[]) {
    const next = this.openFile?.selectable === 'stage' ? lines : [];
    if (next.length || this.picked.length) this.picked = next;
  }

  /** Stages or unstages the lines picked in the open diff. */
  applyLines(lines: number[]) {
    if (!this.diff || !this.openFile?.selectable) return Promise.resolve();
    return this.#mutate('git_apply_lines', {
      diff: this.diff.text,
      lines,
      staged: this.openFile.selectable === 'unstage',
    });
  }

  /** Resolves a conflict by keeping one side. */
  resolve(file: FileChange, side: 'ours' | 'theirs') {
    return this.#mutate('git_resolve', { path: file.path, side });
  }

  /**
   * The row that opened the conflict editor, and its list: closing the editor gives one of them
   * the keyboard back (guidelines, 9.26).
   */
  conflictOpener: { element: HTMLElement; list: HTMLElement | null; at: number } | null = null;

  /** Opens a conflicted file in the three-way editor. */
  openConflict(file: FileChange) {
    // The editor is in its window, or went there during this operation: the file opens there.
    if (this.editorWindow || this.preferWindow) {
      void openInWindow(this, file, null);
      return;
    }
    const focused = typeof document === 'undefined' ? null : document.activeElement;
    const list = focused instanceof HTMLElement ? focused.closest<HTMLElement>('ul.files') : null;
    const at = list ? [...list.querySelectorAll('button.open')].indexOf(focused as HTMLElement) : -1;
    this.conflictOpener = focused instanceof HTMLElement && focused !== document.body ? { element: focused, list, at } : null;
    this.closeDiff();
    this.conflictFile = file;
  }

  /** Versions of a conflicted file: base, both sides, and the file with its markers. */
  readConflict(file: FileChange) {
    return this.git<ConflictFile>('git_conflict_read', { path: file.path });
  }

  /** Writes the result of the resolution and marks the file as resolved. */
  async saveConflict(file: FileChange, content: string) {
    if (!this.result || this.busy) return false;
    this.busy = 'index';
    try {
      this.applyStatus(await this.git<Status>('git_conflict_write', { path: file.path, content }));
      this.conflictFile = null;
      toasts.ok('Conflict resolved', file.path);
      return true;
    } catch (e) {
      toasts.err('Could not save file', errorText(e));
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
      /* the field stays empty: the user types their message */
    }
  }

  async commit() {
    if (!this.result || this.busy) return;
    this.busy = 'commit';
    try {
      await this.git<null>('git_commit', { message: this.message, amend: this.amend });
      toasts.ok(this.amend ? 'Last commit amended' : 'Commit created');
      this.message = '';
      this.amend = false;
      this.closeDiff();
    } catch (e) {
      toasts.err('Commit failed', errorText(e));
    } finally {
      this.busy = '';
    }
    await this.reload();
    // The WIP row is gone: show the commit just created.
    if (this.selected < 0 && this.commits.length) this.selected = 0;
  }

  // --- Sync, references, history ----------------------------------------------------------

  #settle(outcome: Outcome, done: string) {
    if (outcome.stopped) {
      toasts.warn(`${OPERATION[outcome.stopped]} stopped: resolve the conflicts`, outcome.output);
    } else {
      toasts.ok(done, outcome.output);
    }
  }

  /** Runs a long operation, shows its result, then reloads the history. */
  /** `spared`: a failure it recognizes is not notified: the caller offers a way through. */
  async #task(label: string, failed: string, action: () => Promise<Outcome | string>, done: string, spared?: (e: unknown) => boolean) {
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
      if (!spared?.(e)) toasts.err(failed, errorText(e));
    } finally {
      this.busy = '';
    }
    await this.reload();
    if (stopped) this.showWip(true);
    return ok;
  }

  /** `quiet`: automatic fetch, with no notification; a failure is read in the status bar.
   *  Returns `'rejected'` for a push refused because the branches have diverged: the caller offers
   *  a force push (actions.ts, `push`), and no error is notified. */
  async sync(action: SyncAction, quiet = false): Promise<'rejected' | undefined> {
    if (!this.result || this.busy) return;
    if (!quiet) {
      let rejected = false;
      const spared = (e: unknown) => {
        if (action !== 'push') return false;
        if (FETCH_FIRST.test(errorText(e))) {
          toasts.err('Push rejected', 'The remote branch has commits that you have not fetched. Pull, then push again.');
          return true;
        }
        return (rejected = REJECTED.test(errorText(e)));
      };
      await this.#task(action, SYNC[action].failed, async () => {
        try {
          const outcome = await this.git<Outcome>('git_sync', { action, pullMode: this.settings.pullMode });
          if (action === 'fetch') this.lastFetch = { at: Date.now(), ok: true };
          return outcome;
        } catch (e) {
          if (action === 'fetch') this.lastFetch = { at: Date.now(), ok: false };
          throw e;
        }
      }, SYNC[action].done, spared);
      return rejected ? 'rejected' : undefined;
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
    // Nothing new: no need to redraw the graph.
    if (changed) await this.reload();
  }

  async refOp(op: RefOp, done: string, failed = 'Operation failed') {
    if (this.busy) return false;
    if (op.op === 'switch' || op.op === 'switch-remote') this.switching = op.name;
    try {
      return await this.#task(op.op, failed, () => this.git<string>('git_ref_op', { op }), done);
    } finally {
      this.switching = null;
    }
  }

  history(op: HistoryOp, done: string, failed = 'Operation failed') {
    return this.#task(op.op, failed, () => this.git<Outcome>('git_history', { op }), done);
  }

  /** Continues, skips or aborts the operation that stopped. */
  operate(action: 'continue' | 'skip' | 'abort') {
    const name = this.status?.operation ? OPERATION[this.status.operation] : 'Operation';
    const done = action === 'abort' ? `${name} aborted` : `${name} complete`;
    return this.#task(action, 'The operation could not proceed', () => this.git<Outcome>('git_operate', { action }), done);
  }

  /** Prepares an interactive rebase from `sha` up to HEAD; the dialog takes over. */
  async planRebase(sha: string) {
    if (!this.result || this.busy) return;
    try {
      this.rebasePlan = await this.git<RebasePlan>('git_rebase_plan', { sha });
    } catch (e) {
      toasts.err('Could not start interactive rebase', errorText(e));
    }
  }

  rebaseInteractive(base: string | null, todo: TodoItem[]) {
    return this.#task(
      'rebase',
      'Interactive rebase failed',
      () => this.git<Outcome>('git_rebase_interactive', { base, todo }),
      'History rewritten',
    );
  }

  // --- Rewording a commit (guidelines, 9.25) ------------------------------------------------

  /** What rewording `commit` would do, or why it cannot be done, from what is loaded: said before
   *  any click. The core checks it again. */
  rewordCheck(commit: CommitRow): RewordCheck {
    const status = this.status;
    const out: RewordCheck = { reason: '', head: false, replayed: 0, pushed: this.hasRemote && !commit.localOnly };
    if (commit.wip) return { ...out, reason: 'These changes are not committed yet.' };
    if (status?.operation) return { ...out, reason: `A ${status.operation} is in progress. Continue or abort it before rewording.` };
    if (!this.onBranch.has(commit.sha)) {
      return { ...out, reason: `This commit is not on ${this.branch ?? 'the current branch'}. Switch to a branch that contains it to reword it.` };
    }
    if (commit.sha === this.head?.sha) return { ...out, head: true };
    if (commit.parents.length > 1) return { ...out, reason: 'This commit is a merge: only the last commit can be reworded when it is a merge.' };
    // The commits after it: on the branch, and not reachable from it.
    const bySha = new Map(this.commits.map((c) => [c.sha, c]));
    const below = new Set<string>();
    const pending = [commit.sha];
    while (pending.length) {
      const sha = pending.pop()!;
      if (below.has(sha)) continue;
      below.add(sha);
      pending.push(...(bySha.get(sha)?.parents ?? []));
    }
    const after = [...this.onBranch].filter((sha) => !below.has(sha)).map((sha) => bySha.get(sha));
    if (after.some((c) => c && c.parents.length > 1)) {
      return { ...out, reason: 'A merge commit lies between this commit and HEAD: rewording would flatten it. Choose a more recent commit.' };
    }
    if (status && !status.branch) return { ...out, reason: 'HEAD is detached: switch to a branch to reword an older commit.' };
    if (status && (status.staged.length || status.unstaged.some((f) => f.kind !== 'untracked'))) {
      return { ...out, reason: 'Uncommitted changes are in the way: stash or commit them first.' };
    }
    return { ...out, replayed: after.length };
  }

  /** Opens the rewording of `commit`, the selected one by default, in the detail panel, which
   *  comes back if it was hidden. True if it opened. */
  startReword(of?: CommitRow) {
    if (of && of.sha !== this.current?.sha) {
      const index = this.commits.findIndex((c) => c.sha === of.sha);
      if (index < 0) return false;
      this.selected = index;
      this.reveal(index);
    }
    const commit = this.current;
    revealDetail();
    if (!commit || this.locked) return false;
    if (this.rewordCheck(commit).reason) {
      this.rewordRefused++;
      return false;
    }
    const from = document.activeElement;
    this.#rewordFrom = from instanceof HTMLElement && from !== document.body ? from : null;
    // The draft of this commit, if one was left: it is taken up again.
    if (this.reword?.sha !== commit.sha) this.reword = { sha: commit.sha, text: null, original: '' };
    this.rewordAsk++;
    return true;
  }

  /** The focus goes back to where the rewording started. */
  #rewordBack() {
    const from = this.#rewordFrom;
    this.#rewordFrom = null;
    void tick().then(() => {
      // "Reword" gives way to the form while it is open: the button of the panel shown now takes
      // the focus back. Otherwise the graph does.
      const view = document.querySelector('section.view:not(.hidden)');
      const button = from?.classList.contains('reword-open') ? view?.querySelector<HTMLElement>('button.reword-open') : null;
      const graph = view?.querySelector<HTMLElement>('[aria-label="Commit history"]');
      focusWithRing(from?.isConnected ? from : (button ?? graph));
    });
  }

  /** "Cancel" or Escape: the draft is dropped, as in a dialog. */
  cancelReword() {
    this.reword = null;
    this.#rewordBack();
  }

  /** "Save message": the commit gets its new message, and stays selected under its new SHA. */
  async saveReword() {
    const draft = this.reword;
    if (!this.result || this.busy || !draft || draft.text === null || !draft.text.trim()) return;
    this.busy = 'reword';
    let done: Reworded | null = null;
    try {
      done = await this.git<Reworded>('git_reword', { sha: draft.sha, message: draft.text });
    } catch (e) {
      toasts.err('The commit could not be reworded', errorText(e));
    } finally {
      this.busy = '';
    }
    if (!done) {
      // A rebase that stopped leaves its banner: the history is read again.
      await this.reload();
      return;
    }
    this.reword = null;
    const detail = [`New SHA ${done.sha.slice(0, 7)}.`];
    if (done.replayed) detail.push(`${plural(done.replayed, 'later commit')} ${done.replayed === 1 ? 'was' : 'were'} replayed.`);
    if (done.pushed) detail.push('Force push to update the remote.');
    toasts.ok('Commit reworded', detail.join(' '));
    await this.reload();
    this.selectSha(done.sha);
    this.#rewordBack();
  }

  /** Starts the automatic fetch; returns the function that stops it. */
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

  /**
   * The element that opened the diff, a file row most of the time, and the list it is in: closing
   * the diff gives one of them the keyboard back (guidelines, 9.14).
   */
  #opener: Opener | null = null;

  async #showDiff(file: OpenFile, fetch: () => Promise<Diff>) {
    const token = ++this.#diffToken;
    const focused = typeof document === 'undefined' ? null : document.activeElement;
    // A diff opened from another diff keeps its opener; one opened with no focus has none.
    if (focused instanceof HTMLElement && focused !== document.body && !focused.closest('.diff')) {
      const list = focused.closest<HTMLElement>('ul.files, [role="listbox"], [role="tree"]');
      this.#opener = {
        element: focused,
        list,
        at: rowsOf(list).indexOf(focused),
        view: focused.closest('[data-repo-view]'),
        panel: focused.closest('[data-file-lists]'),
      };
    } else if (!focused || focused === document.body) {
      this.#opener = null;
    }
    if (this.openFile?.key !== file.key) {
      this.diff = null;
      this.picked = [];
    }
    this.openFile = file;
    // The diff opens on the card of the graph: asking for it brings the card back if it was hidden.
    this.terminals.setGraph(true);
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
    const open = !!this.openFile;
    const opener = this.#opener;
    this.#opener = null;
    this.#diffToken++;
    this.openFile = null;
    this.diff = null;
    this.picked = [];
    this.diffError = '';
    this.diffLoading = false;
    if (open && typeof document !== 'undefined') void tick().then(() => this.#giveKeyboardBack(opener));
  }

  /**
   * However the diff closed (Escape, "Close", or by itself when its file left its list), the
   * keyboard it took away goes back: to the row that opened the diff, otherwise to the current
   * row of its list, otherwise to the list of the graph that takes its place (guidelines, 9.14).
   */
  #giveKeyboardBack(opener: Opener | null) {
    // Still in the diff that leaves (its animation keeps it on the page for a moment), on the body
    // after a click, or gone with its file: the keyboard is lost. Elsewhere, it was taken: it stays.
    const now = document.activeElement;
    const leaving = !!now?.closest('.diff') && !this.openFile;
    if (now && now !== document.body && now.isConnected && !leaving) return;
    if (!opener) return;
    const view = opener.view;
    const usable = (el: Element | null | undefined): el is HTMLElement =>
      el instanceof HTMLElement && el.isConnected && el !== document.body && !el.closest('[inert], [hidden]');
    // The row gone with its file: the row that takes its place in the list; the list left empty,
    // the next list of the changes, as for a key in the list (9.6).
    const rows = rowsOf(opener.list);
    const instead = opener.at >= 0 ? rows[Math.min(opener.at, rows.length - 1)] : undefined;
    const emptied = opener.list && !opener.list.isConnected && opener.panel ? besideList(opener.panel, Number(opener.list.dataset.order ?? 0)) : null;
    const target = [opener.element, instead, opener.list?.querySelector('[tabindex="0"]'), emptied, view?.querySelector('.center [role="listbox"]')].find(usable);
    // Nothing found where it was: the graph, which is always there (11).
    focusSoon(target ?? visibleOne('[role="listbox"][aria-label="Commit history"]'));
  }

  openWorkDiff(file: FileChange, staged: boolean) {
    // Partial staging only holds for a modified text file.
    const partial = file.kind === 'modified';
    void this.#showDiff(
      {
        key: `${staged ? 's' : 'u'}:${file.path}`,
        title: file.path,
        subtitle: `${staged ? 'Staged' : 'Unstaged'}${file.oldPath ? ` · renamed from ${file.oldPath}` : ''}`,
        selectable: partial ? (staged ? 'unstage' : 'stage') : null,
      },
      () =>
        this.git<Diff>(
          'git_diff_file',
          { path: file.path, oldPath: file.oldPath, staged, untracked: file.kind === 'untracked' },
          true,
        ),
    );
  }

  openCommitDiff(file: FileChange) {
    const commit = this.current;
    if (!commit || commit.wip) return;
    void this.#showDiff(
      {
        key: `c:${file.path}`,
        title: file.path,
        subtitle: `${commit.sha.slice(0, 7)} · ${commit.subject}${file.oldPath ? ` · renamed from ${file.oldPath}` : ''}`,
        selectable: null,
      },
      () =>
        this.git<Diff>(
          'git_diff_commit',
          { sha: commit.sha, parent: commit.parents[0] ?? null, path: file.path, oldPath: file.oldPath },
          true,
        ),
    );
  }

  openStashDiff(index: number, message: string) {
    void this.#showDiff(
      { key: `stash:${index}`, title: `Stash ${index}`, subtitle: message, selectable: null },
      () => this.git<Diff>('git_stash_show', { index }, true),
    );
  }

  /** After a change to the index: the open diff follows its file, or closes if the file left its list. */
  #syncOpenDiff() {
    const key = this.openFile?.key;
    if (!key || !this.status || !(key.startsWith('u:') || key.startsWith('s:'))) return;
    const staged = key.startsWith('s:');
    const path = key.slice(2);
    const file = (staged ? this.status.staged : this.status.unstaged).find((f) => f.path === path);
    if (file) this.openWorkDiff(file, staged);
    else this.closeDiff();
  }

  // --- Detail of the selected commit -----------------------------------------------------

  /**
   * To call when the selection changes. The loading waits for a short pause, so as not to run
   * git for every row passed over from the keyboard.
   */
  loadDetail() {
    window.clearTimeout(this.#detailTimer);
    this.closeDiff();
    this.detail = null;
    this.detailFor = '';
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
        if (key === this.currentKey) {
          this.detail = data;
          this.detailFor = key;
        }
      } catch (e) {
        if (key === this.currentKey) this.detailError = errorText(e);
      }
    }, 120);
  }
}
