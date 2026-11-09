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
