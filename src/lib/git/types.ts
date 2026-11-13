// Mirror of the structures returned by the `git_log` command (src-tauri/src/git/log.rs).

export type RefKind = 'head' | 'local' | 'remote' | 'tag' | 'detached';

export interface RefLabel {
  name: string;
  kind: RefKind;
}

export interface CommitRow {
  sha: string;
  parents: string[];
  author: string;
  /** Author date, in Unix seconds. */
  time: number;
  refs: RefLabel[];
  subject: string;
  lane: number;
  /** Segments to the next row: [start column, end column, lane]. */
  edges: [number, number, number][];
  wip: boolean;
  /** Commit absent from every remote branch: not pushed yet. */
  localOnly: boolean;
}

export interface CommandLog {
  command: string;
  ms: number;
  ok: boolean;
}

export interface LogResult {
  repo: string;
  name: string;
  branch: string | null;
  commits: CommitRow[];
  maxLanes: number;
  truncated: boolean;
  wipFiles: number;
  timings: { gitMs: number; parseMs: number; layoutMs: number };
  commands: CommandLog[];
}

// --- Working directory and index (src-tauri/src/git/work.rs) -----------------------------

export type ChangeKind =
  | 'added'
  | 'modified'
  | 'deleted'
  | 'renamed'
  | 'copied'
  | 'typechange'
  | 'untracked'
  | 'conflict';

export interface FileChange {
  path: string;
  oldPath: string | null;
  kind: ChangeKind;
}

export type Operation = 'merge' | 'rebase' | 'cherry-pick' | 'revert';

export interface Status {
  head: string | null;
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  staged: FileChange[];
  unstaged: FileChange[];
  /** Operation that stopped, to continue or to abort. */
  operation: Operation | null;
}

export interface Diff {
  text: string;
  truncated: boolean;
}

export interface CommitDetail {
  message: string;
  files: FileChange[];
}

/** Envelope of the git commands: the data, and the commands run to get it. */
export interface Done<T> {
  data: T;
  commands: CommandLog[];
}

export interface Failed {
  message: string;
  commands: CommandLog[];
}

// --- References and history (src-tauri/src/git/ops.rs) ------------------------------------

/** Position of the local branch that bears the name of a remote branch, relative to it. */
export interface Relation {
  /** Name of the local branch; `null` if it does not exist yet. */
  local: string | null;
  /** Commits of the local branch absent from the remote branch. */
  ahead: number;
  /** Commits of the remote branch absent from the local branch. */
  behind: number;
}

export interface Branch {
  name: string;
  sha: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  /** The tracked remote branch no longer exists. */
  gone: boolean;
  current: boolean;
}

export interface NamedRef {
  name: string;
  sha: string;
}

export interface Stash {
  index: number;
  sha: string;
  message: string;
  time: number;
}

export interface Refs {
  branches: Branch[];
  remotes: NamedRef[];
  tags: NamedRef[];
  stashes: Stash[];
  remoteNames: string[];
}

export type RefOp =
  | { op: 'switch'; name: string }
  | { op: 'switch-remote'; name: string }
  | { op: 'detach'; sha: string }
  | { op: 'branch-create'; name: string; start: string; switch: boolean }
  | { op: 'branch-rename'; name: string; newName: string }
  | { op: 'branch-delete'; name: string; force: boolean }
  | { op: 'remote-branch-delete'; name: string }
  | { op: 'tag-create'; name: string; sha: string; message: string }
  | { op: 'tag-delete'; name: string }
  | { op: 'tag-push'; name: string }
  | { op: 'tag-delete-remote'; name: string }
  | { op: 'stash-push'; message: string }
  | { op: 'stash-pop'; index: number }
  | { op: 'stash-apply'; index: number }
  | { op: 'stash-drop'; index: number };

export type ResetMode = 'soft' | 'mixed' | 'hard';

export type HistoryOp =
  | { op: 'merge'; target: string }
  | { op: 'rebase'; onto: string }
  | { op: 'cherry-pick'; sha: string; mainline: boolean }
  | { op: 'revert'; sha: string; mainline: boolean }
  | { op: 'reset'; sha: string; mode: ResetMode };

/** Result of an operation that can stop on a conflict. */
export interface Outcome {
  output: string;
  stopped: Operation | null;
}

export interface RebasePlan {
  base: string | null;
  /** From the oldest to the newest. */
  commits: { sha: string; subject: string; message: string }[];
  pushed: boolean;
}

export type TodoAction = 'pick' | 'reword' | 'squash' | 'fixup' | 'drop';

export interface TodoItem {
  action: TodoAction;
  sha: string;
  message?: string;
}

/** Versions of a conflicted file (src-tauri/src/git/conflict.rs). */
export interface ConflictFile {
  base: string | null;
  ours: string | null;
  theirs: string | null;
  /** File of the working directory, with the conflict markers. */
  merged: string | null;
  /** A version is not text, or is too large for the editor. */
  binary: boolean;
}
