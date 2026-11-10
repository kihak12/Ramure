// Miroir des structures renvoyées par la commande `git_log` (src-tauri/src/git/log.rs).

export type RefKind = 'head' | 'local' | 'remote' | 'tag' | 'detached';

export interface RefLabel {
  name: string;
  kind: RefKind;
}

export interface CommitRow {
  sha: string;
  parents: string[];
  author: string;
  /** Date de l'auteur, en secondes Unix. */
  time: number;
  refs: RefLabel[];
  subject: string;
  lane: number;
  /** Segments vers la ligne suivante : [colonne de départ, colonne d'arrivée, lane]. */
  edges: [number, number, number][];
  wip: boolean;
  /** Commit absent de toute branche distante : pas encore poussé. */
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

// --- Répertoire de travail et index (src-tauri/src/git/work.rs) ---------------------------

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
  /** Opération en suspens, à continuer ou à abandonner. */
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

/** Enveloppe des commandes git : la donnée, et les commandes lancées pour l'obtenir. */
export interface Done<T> {
  data: T;
  commands: CommandLog[];
}

export interface Failed {
  message: string;
  commands: CommandLog[];
}

// --- Références et historique (src-tauri/src/git/ops.rs) ----------------------------------

export interface Branch {
  name: string;
  sha: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  /** La branche distante suivie n'existe plus. */
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

/** Résultat d'une opération qui peut s'arrêter sur un conflit. */
export interface Outcome {
  output: string;
  stopped: Operation | null;
}

export interface RebasePlan {
  base: string | null;
  /** Du plus ancien au plus récent. */
  commits: { sha: string; subject: string; message: string }[];
  pushed: boolean;
}

export type TodoAction = 'pick' | 'reword' | 'squash' | 'fixup' | 'drop';

export interface TodoItem {
  action: TodoAction;
  sha: string;
  message?: string;
}

/** Versions d'un fichier en conflit (src-tauri/src/git/conflict.rs). */
export interface ConflictFile {
  base: string | null;
  ours: string | null;
  theirs: string | null;
  /** Fichier du répertoire de travail, avec les marqueurs de conflit. */
  merged: string | null;
  /** Une version n'est pas du texte, ou est trop volumineuse pour l'éditeur. */
  binary: boolean;
}
