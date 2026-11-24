<script lang="ts">
  // The window of the conflict editor (guidelines, 9.26, "Its own window"): the banner of the
  // operation, the files in conflict, and the editor, for the repository of the main window.
  import { emitTo, listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount, tick, untrack } from 'svelte';
  import ContextMenu from '../ui/ContextMenu.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import { dialogs } from '../ui/dialogs.svelte';
  import { focusWithRing, keepFocusInDialogs } from '../ui/focus';
  import { comboOf, keys } from '../ui/keys.svelte';
  import Notifications from '../ui/Notifications.svelte';
  import ConflictEditor from './ConflictEditor.svelte';
  import { askedOf, EVENT, type EditorSnapshot } from './conflictWindow';
  import FileList from './FileList.svelte';
  import OperationBanner from './OperationBanner.svelte';
  import { OPERATION, Repo } from './repo.svelte';
  import type { FileChange } from './types';

  const asked = askedOf(location.search);
  const window = getCurrentWindow();
  const label = window.label;
  const repo = new Repo();

  let editor = $state<ReturnType<typeof ConflictEditor>>();
  /** What the editor carries from the main window, for the file it opens. */
  let restore = $state.raw<EditorSnapshot | null>(null);
  let modified = false;
  /** The operation ended from the main window while choices were not saved (9.26, "Its ends"). */
  let orphaned = $state(false);
  /** The window is leaving: its close asks nothing more. */
  let leaving = false;
  let loaded = $state(false);
  /** The last file open, and the list it was in: a file saved, the row in its place opens. */
  let lastPath: string | null = null;
  let lastList: string[] = [];

  /** The main window is told which file is open, and whether its choices are not saved. */
  function report() {
    void emitTo('main', EVENT.state, { label, path: repo.conflictFile?.path ?? null, modified });
  }

  /** Opens `file`, with what was not saved of it when it comes from the main window (9.26, "One editor"). */
  async function openFile(file: FileChange, snapshot: EditorSnapshot | null) {
    const open = repo.conflictFile;
    if (open && open.path === file.path && !snapshot) {
      await window.unminimize();
      await window.setFocus();
      return;
    }
    if (open && open.path !== file.path && modified) {
      await window.setFocus();
      const sure = await dialogs.confirm({
        title: `Leave ${open.path} without saving?`,
        body: 'The choices and corrections made in this editor will be lost for good. The file keeps its conflict markers.',
        confirmLabel: 'Leave without saving',
        danger: true,
      });
      if (!sure) return;
    }
    restore = snapshot;
    modified = !!snapshot;
    orphaned = false;
    repo.conflictFile = file;
    report();
  }

  /** Back to the main window, with the choices: a move, nothing is asked. */
  async function back() {
    const file = repo.conflictFile;
    if (!file) return close();
    leaving = true;
    await emitTo('main', EVENT.back, { label, file, snapshot: editor?.snapshot() ?? null });
    await window.destroy();
  }

  /** Closes the window, once nothing is to be lost: the main window takes the keyboard back. */
  async function close() {
    leaving = true;
    await emitTo('main', EVENT.closed, { label, path: repo.conflictFile?.path ?? lastPath });
    await window.destroy();
  }

  onMount(() => {
    const stops = [
      listen<{ label: string; file: FileChange; snapshot: EditorSnapshot | null }>(EVENT.open, (event) => {
        if (event.payload.label === label) void openFile(event.payload.file, event.payload.snapshot);
      }),
      listen<{ label: string }>(EVENT.recall, (event) => {
        if (event.payload.label === label) void back();
      }),
      listen<{ label: string }>(EVENT.ended, async (event) => {
        if (event.payload.label !== label) return;
        await repo.refreshStatus();
        if (modified) orphaned = true;
        else void close();
      }),
      listen<{ label: string }>(EVENT.quit, (event) => {
        if (event.payload.label !== label) return;
        leaving = true;
        void window.destroy();
      }),
    ];
    // The button of the system: with choices not saved, the question of "Close" (9.26, "Its ends").
    const closing = window.onCloseRequested(async (event) => {
      if (leaving) return;
      if (modified && !orphaned) {
        const sure = await dialogs.confirm({
          title: 'Close without saving?',
          body: 'The choices and corrections made in this editor will be lost for good. The file keeps its conflict markers.',
          confirmLabel: 'Close without saving',
          danger: true,
        });
        if (!sure) {
          event.preventDefault();
          return;
        }
      }
      leaving = true;
      await emitTo('main', EVENT.closed, { label, path: repo.conflictFile?.path ?? lastPath });
    });
    const trap = keepFocusInDialogs();
    void (async () => {
      if (asked) await repo.open(asked.repo);
      loaded = true;
      // Painted: the window shows, with no flash of an empty page (guidelines, 4.2).
      requestAnimationFrame(() => void window.show());
      // Listening: the main window sends the file, with its choices.
      await Promise.all(stops);
      await emitTo('main', EVENT.ready, { label });
    })();
    return () => {
      stops.forEach((stop) => void stop.then((off) => off()));
      void closing.then((off) => off());
      trap?.();
    };
  });

  // What the window writes, the main window reads again.
  let statusSeen = false;
  $effect(() => {
    void repo.status;
    untrack(() => {
      if (!repo.status) return;
      if (statusSeen) void emitTo('main', EVENT.changed, { label });
      statusSeen = true;
    });
  });

  // Saved, the editor goes on to the file in its place; the operation over, the window closes
  // (9.26, "Saved, it goes on").
  $effect(() => {
    const open = repo.conflictFile;
    const list = repo.conflicts.map((file) => file.path);
    const operation = repo.status?.operation;
    untrack(() => {
      if (!loaded || !repo.status || leaving) return;
      if (!operation) {
        if (!orphaned) void close();
        return;
      }
      if (open) {
        lastPath = open.path;
        lastList = list;
        return;
      }
      if (!list.length) {
        void tick().then(() => focusWithRing(document.querySelector<HTMLElement>('.banner .btn.primary')));
        report();
        return;
      }
      const at = Math.max(0, lastList.indexOf(lastPath ?? ''));
      const next = repo.conflicts[Math.min(at, repo.conflicts.length - 1)];
      restore = null;
      modified = false;
      repo.conflictFile = next;
      report();
    });
  });

  /** Ctrl + Shift + W, or what the settings give it: back to the main window (9.26). */
  function onKey(e: KeyboardEvent) {
    if (document.querySelector('dialog[open]')) return;
    const combo = comboOf(e);
    if (combo && keys.find(combo, 'git') === 'conflict-window') {
      e.preventDefault();
      void back();
    }
  }

  const operation = $derived(repo.status?.operation ?? null);
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="window">
  {#if repo.status}
    <OperationBanner {repo} place="window" />
  {/if}
  <div class="body">
    <section class="list" aria-label="Files in conflict">
      <header><span class="label">In conflict · {repo.conflicts.length}</span></header>
      <div class="rows">
        <FileList files={repo.conflicts} active={repo.conflictFile?.path ?? null} disabled={repo.locked} onopen={(file) => void openFile(file, null)} />
      </div>
    </section>
    <div class="editor">
      {#if repo.conflictFile}
        <!-- First in the order of the keyboard, as on the screen: the editor under it waits. -->
        {#if orphaned}
          <div class="orphaned" role="alert">
            <span>This file is no longer in conflict. The choices made here cannot be saved.</span>
            <button class="btn small" type="button" onclick={() => void close()}>Close</button>
          </div>
        {/if}
        <div class="host" inert={orphaned}>
          <ConflictEditor
            bind:this={editor}
            {repo}
            file={repo.conflictFile}
            place="window"
            {restore}
            onmove={() => void back()}
            onclose={() => void close()}
            onmodified={(value) => {
              modified = value;
              report();
            }}
          />
        </div>
      {:else if loaded && operation && !repo.conflicts.length}
        <div class="done">
          <p><b>All conflicts are resolved.</b></p>
          <p>Continue the {OPERATION[operation].toLowerCase()} to finish.</p>
        </div>
      {/if}
    </div>
  </div>
</div>

<Dialog />
<ContextMenu />
<Notifications />

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding-top: 8px;
    /* The window never scrolls: what scrolls is inside the cards (guidelines, 4.2). */
    overflow: clip;
  }
  .body {
    flex: 1;
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    min-height: 0;
  }
  /* The files in conflict: a card on the left, its rows as in the changes panel (9.6). */
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    margin: 0 0 8px 8px;
    border-radius: var(--radius-card);
    background: var(--panel);
    overflow: hidden;
  }
  .list header {
    flex: none;
    padding: 10px 12px 6px;
  }
  .rows {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 6px 6px;
  }
  /* The editor sits in this cell, 8 px from its edges, as over the cards of the main window. */
  .editor {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  /* The editor keeps its own place in the cell. */
  .host {
    display: contents;
  }
  .done {
    position: absolute;
    inset: 0 8px 8px;
    display: grid;
    place-content: center;
    gap: 4px;
    border-radius: var(--radius-card);
    background: var(--panel);
    text-align: center;
  }
  .done p {
    margin: 0;
  }
  .done p + p {
    color: var(--muted);
  }
  /* Over the editor: what was chosen there can no longer be saved (9.26, "Its ends"). */
  .orphaned {
    position: absolute;
    z-index: 7;
    top: 8px;
    left: 16px;
    right: 16px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: var(--warn-soft);
    color: var(--ink);
  }
  .orphaned span {
    flex: 1;
    min-width: 0;
  }
</style>
