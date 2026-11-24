// The conflict editor in its own window (guidelines, 9.26, "Its own window"): what the two windows
// say to one another. The window of the editor is a second page of the application, opened on the
// same repository; the editor moves between the two with its choices.

import type { Pick } from './conflicts';
import type { ResultSnapshot } from './ConflictResult.svelte';
import type { FileChange } from './types';

/** The editor as it is, carried to the other window: everything that is not saved in the file. */
export interface EditorSnapshot {
  path: string;
  picks: Pick[];
  edited: boolean[];
  /** Each choice, with what it changed and the depth of the typing history when it was made. */
  undo: { depth: number; items: { block: number; pick: Pick; edited: boolean; text: string }[] }[];
  index: number;
  showBase: boolean;
  text: string;
  result: ResultSnapshot | null;
}

/** A file to open in the editor, with what was not saved of it when it comes from the other window. */
export interface Moved {
  file: FileChange;
  snapshot: EditorSnapshot | null;
}

/** The events between the main window and the window of the editor. */
export const EVENT = {
  /** Window → main: it is listening; main answers with `open`. */
  ready: 'conflict-window:ready',
  /** Main → window: open this file, with what was not saved of it. */
  open: 'conflict-window:open',
  /** Window → main: the editor comes back, with what was not saved. */
  back: 'conflict-window:back',
  /** Window → main: the window is closed; `path` is the file it had, to give it the keyboard. */
  closed: 'conflict-window:closed',
  /** Window → main: whether the editor holds choices not saved, and on which file. */
  state: 'conflict-window:state',
  /** Window → main: the window wrote in the repository: read its status again. */
  changed: 'conflict-window:changed',
  /** Main → window: give the editor back to the main window (the shortcut or the palette). */
  recall: 'conflict-window:recall',
  /** Main → window: the operation is over (continued, skipped or aborted from the main window). */
  ended: 'conflict-window:ended',
  /** Main → window: close now, the main window is closing. */
  quit: 'conflict-window:quit',
} as const;

/** The label of the window of a repository: one window per repository, at most. */
export function labelOf(repo: string): string {
  let hash = 0;
  for (const c of repo) hash = (Math.imul(hash, 31) + c.charCodeAt(0)) | 0;
  return `conflict-${(hash >>> 0).toString(36)}`;
}

/** The address of the page of the window: the same application, told what to show. */
export function urlOf(repo: string, path: string): string {
  return `index.html?window=conflict&repo=${encodeURIComponent(repo)}&file=${encodeURIComponent(path)}`;
}

/** The repository and the file asked for, read in the address of the window; null in the main window. */
export function askedOf(search: string): { repo: string; file: string } | null {
  const params = new URLSearchParams(search);
  if (params.get('window') !== 'conflict') return null;
  const repo = params.get('repo');
  const file = params.get('file');
  return repo && file ? { repo, file } : null;
}
