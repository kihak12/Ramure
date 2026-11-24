// The conflict editor in its own window, seen from the main window (guidelines, 9.26, "Its own
// window"): opening the window, sending it a file, and hearing it come back, close or write.

import { emitTo, listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { tick } from 'svelte';
import { focusBack } from '../ui/focus';
import { toasts } from '../ui/toasts.svelte';
import { EVENT, labelOf, urlOf, type EditorSnapshot, type Moved } from './conflictWindow';
import type { Repo } from './repo.svelte';
import type { FileChange } from './types';

/** What the window is told first, once it listens. */
const pending = new Map<string, Moved>();

/**
 * Moves the editor of `repo` to its window, or sends the window another file: the same file only
 * brings it forward; another one, the window opens it, after its question if choices are not saved.
 */
export async function openInWindow(repo: Repo, file: FileChange, snapshot: EditorSnapshot | null) {
  const label = labelOf(repo.path);
  // Once in its window, the other files of the operation open there too (9.26).
  repo.preferWindow = true;
  repo.conflictFile = null;
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await emitTo(label, EVENT.open, { label, file, snapshot });
    await existing.unminimize();
    await existing.setFocus();
    return;
  }
  pending.set(label, { file, snapshot });
  repo.editorWindow = { label, file: file.path, modified: !!snapshot };
  const window = new WebviewWindow(label, {
    url: urlOf(repo.path, file.path),
    title: 'Conflict editor',
    width: 1440,
    height: 880,
    minWidth: 1000,
    minHeight: 600,
  });
  void window.once('tauri://error', (event) => {
    // The window could not open: the editor stays in the main window, with its choices.
    pending.delete(label);
    repo.editorWindow = null;
    repo.preferWindow = false;
    repo.restoreEditor = snapshot;
    repo.conflictFile = file;
    toasts.err('Could not open the window', String(event.payload));
  });
}

/** Asks the window to give the editor back to the main window, with its choices. */
export function recallEditor(repo: Repo) {
  if (repo.editorWindow) void emitTo(repo.editorWindow.label, EVENT.recall, { label: repo.editorWindow.label });
}

/** Brings the window of the editor forward. */
export async function showEditorWindow(repo: Repo) {
  if (!repo.editorWindow) return;
  const window = await WebviewWindow.getByLabel(repo.editorWindow.label);
  await window?.unminimize();
  await window?.setFocus();
}

/** Closes the windows of the editors at once: the main window closes, or a repository tab. */
export async function closeEditorWindows(repos: Repo[]) {
  for (const repo of repos) {
    if (!repo.editorWindow) continue;
    const window = await WebviewWindow.getByLabel(repo.editorWindow.label);
    repo.editorWindow = null;
    await window?.destroy();
  }
}

/** The operation is over in the main window: the window closes, or warns if choices are not saved. */
export function operationEnded(repo: Repo) {
  repo.preferWindow = false;
  if (repo.editorWindow) void emitTo(repo.editorWindow.label, EVENT.ended, { label: repo.editorWindow.label });
}

/** Gives the keyboard back to the main window, on the row of `path` in the list of the conflicts. */
async function backToRow(path: string | null) {
  await getCurrentWindow().setFocus();
  await tick();
  const rows = [...document.querySelectorAll<HTMLElement>('[data-file-lists] ul.files button.open')];
  focusBack(rows.find((row) => row.title === path) ?? rows[0]);
}

/** Listens to the windows of the editors, for the repositories of `repos`. Returns how to stop. */
export function listenEditorWindows(repos: () => Repo[]): () => void {
  const repoOf = (label: string) => repos().find((repo) => repo.editorWindow?.label === label || labelOf(repo.path) === label);
  const stops = [
    listen<{ label: string }>(EVENT.ready, (event) => {
      const { label } = event.payload;
      const moved = pending.get(label);
      pending.delete(label);
      if (moved) void emitTo(label, EVENT.open, { label, ...moved });
    }),
    listen<{ label: string; file: FileChange; snapshot: EditorSnapshot | null }>(EVENT.back, (event) => {
      const repo = repoOf(event.payload.label);
      if (!repo) return;
      repo.editorWindow = null;
      repo.preferWindow = false;
      repo.restoreEditor = event.payload.snapshot;
      repo.conflictFile = event.payload.file;
      void getCurrentWindow().setFocus();
    }),
    listen<{ label: string; path: string | null }>(EVENT.closed, (event) => {
      const repo = repoOf(event.payload.label);
      if (!repo) return;
      repo.editorWindow = null;
      void repo.refreshStatus().then(() => backToRow(event.payload.path));
    }),
    listen<{ label: string; path: string | null; modified: boolean }>(EVENT.state, (event) => {
      const repo = repoOf(event.payload.label);
      if (repo?.editorWindow) repo.editorWindow = { ...repo.editorWindow, file: event.payload.path, modified: event.payload.modified };
    }),
    listen<{ label: string }>(EVENT.changed, (event) => {
      const repo = repoOf(event.payload.label);
      if (!repo) return;
      void repo.refreshStatus();
      void repo.refreshRefs();
    }),
  ];
  return () => stops.forEach((stop) => void stop.then((off) => off()));
}
