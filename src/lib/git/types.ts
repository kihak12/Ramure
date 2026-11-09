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

export interface Status {
  head: string | null;
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  staged: FileChange[];
  unstaged: FileChange[];
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
