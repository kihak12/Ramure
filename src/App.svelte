<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import { gitCommands } from './lib/git/commands';
  import { workspace } from './lib/git/workspace.svelte';
  import { mayQuit } from './lib/ssh/guard';
  import { sshCommands } from './lib/ssh/commands';
  import { ssh } from './lib/ssh/ssh.svelte';
  import GitWorkspace from './lib/git/GitWorkspace.svelte';
  import SshWorkspace from './lib/ssh/SshWorkspace.svelte';
  import FilesWorkspace from './lib/files/FilesWorkspace.svelte';
  import { filesCommands } from './lib/files/commands';
  import { files } from './lib/files/files.svelte';
  import { modules, type Module } from './lib/ui/modules.svelte';
  import ContextMenu from './lib/ui/ContextMenu.svelte';
  import Dialog from './lib/ui/Dialog.svelte';
  import Icon from './lib/ui/Icon.svelte';
  import { allowedInTerminal, comboOf, display, keys } from './lib/ui/keys.svelte';
  import CommandPalette from './lib/ui/CommandPalette.svelte';
  import { palette, type Command } from './lib/ui/palette.svelte';
  import Notifications from './lib/ui/Notifications.svelte';
  import SettingsPage from './lib/settings/SettingsPage.svelte';
  import { settingsPage } from './lib/settings/settings.svelte';
  import { updates } from './lib/ui/updates.svelte';
  import { keepFocusInDialogs, keepFocusThroughDisabling } from './lib/ui/focus';
  import { closeEditorWindows, listenEditorWindows } from './lib/git/editorWindow.svelte';

  // Closing the window stops every local terminal: if a program runs in one, ask first.
  // An error must never prevent quitting: when in doubt, the window closes.
  onMount(() => {
    const stop = getCurrentWindow().onCloseRequested(async (event) => {
      try {
        const locals = [...ssh.sessions, ...workspace.tabs.flatMap((tab) => tab.repo.terminals.sessions)];
        const editors = workspace.tabs.flatMap((tab) => (tab.repo.editorWindow?.modified && tab.repo.editorWindow.file ? [tab.repo.editorWindow.file] : []));
        if (!(await mayQuit(locals, files.running, editors, files.unsent))) event.preventDefault();
        else {
          await closeEditorWindows(workspace.tabs.map((tab) => tab.repo));
          await files.cleanCopies();
        }
      } catch {
        /* let it close */
      }
    });
    return () => void stop.then((off) => off());
  });

  // Tab never leaves a modal dialog for a stop that cannot be seen (guidelines, 9.10 and 11).
  onMount(() => keepFocusInDialogs());
  onMount(() => keepFocusThroughDisabling());

  // The windows of the conflict editor (guidelines, 9.26, "Its own window").
  onMount(() => listenEditorWindows(() => workspace.tabs.map((tab) => tab.repo)));

  // Updates (guidelines, 9.23): what this copy is, then the automatic check.
  onMount(() => {
    void updates.start();
    return () => updates.stop();
  });

  const view = $derived(modules.view);
  // The settings page takes the place of the module; leaving it brings back the one that was shown.
  const shown = $derived(settingsPage.open ? 'settings' : view);

  let settingsButton = $state<HTMLButtonElement>();

  /** The settings open on "General", whatever the module (guidelines, section 9.18). */
  const toggleSettings = () => settingsPage.toggle('general');

  const show = (module: Module) => modules.show(module);

  /** Every command available now, with its shortcut. */
  function commands(): Command[] {
    const general: Command[] = [
      { id: 'view-git', group: 'Modules', icon: 'branch', title: 'Show repositories', disabled: shown === 'git', run: () => show('git') },
      { id: 'view-ssh', group: 'Modules', icon: 'terminal', title: 'Show SSH terminal', disabled: shown === 'ssh', run: () => show('ssh') },
      { id: 'view-files', group: 'Modules', icon: 'folder', title: 'Show files', keywords: 'sftp explorer browse transfer upload download', disabled: shown === 'files', run: () => show('files') },
      { id: 'settings', group: 'Modules', icon: 'settings', title: settingsPage.open ? 'Close settings' : 'Settings', keywords: 'preferences options theme dark light colors style graph', run: toggleSettings },
      {
        id: 'terminal-new',
        group: 'Modules',
        icon: 'laptop',
        title: 'New local terminal',
        keywords: 'shell console command line powershell bash',
        run: () => {
          show('ssh');
          ssh.openLocal();
        },
      },
      { id: 'shortcuts', group: 'Modules', icon: 'settings', title: 'Keyboard shortcuts', keywords: 'keys keyboard settings', run: () => settingsPage.show('keys') },
    ];
    // The Git actions only make sense in front of a repository.
    const all =
      shown === 'git'
        ? [...gitCommands(), ...general]
        : shown === 'ssh'
          ? [...sshCommands(), ...general]
          : shown === 'files'
            ? [...filesCommands(), ...general]
            : general;
    return all.map((command) => {
      const combo = keys.binding(command.id);
      return combo ? { ...command, shortcut: display(combo) } : command;
    });
  }

  const openPalette = () => palette.show(commands());

  function onKey(e: KeyboardEvent) {
    // A new combination is being typed in the settings: it triggers nothing.
    if (keys.recording) return;
    const modal = document.querySelector('dialog[open]') !== null;
    // Escape leaves the settings page. A search field that holds text is emptied first.
    if (e.key === 'Escape' && settingsPage.open && !modal) {
      if (e.target instanceof HTMLInputElement && e.target.type === 'search' && e.target.value) return;
      e.preventDefault();
      settingsPage.hide();
      // The page becomes inert: the focus goes back to the button that opens it, so it is not lost.
      settingsButton?.focus();
      return;
    }
    const combo = comboOf(e);
    if (!combo) return;
    // Ctrl + Shift + P always opens the palette, whatever the shortcut chosen for it.
    const id = combo === 'Ctrl+Shift+P' ? 'palette' : keys.find(combo, view);
    if (!id) return;
    const inTerminal = e.target instanceof Element && e.target.closest('.xterm') !== null;
    if (inTerminal && !allowedInTerminal(combo)) return;
    // An open dialog keeps the keyboard; only the palette closes with its shortcut.
    if (modal && !(id === 'palette' && palette.commands)) return;
    e.preventDefault();
    e.stopPropagation();
    if (id === 'palette') {
      if (palette.commands) palette.hide();
      else openPalette();
      return;
    }
    const command = commands().find((c) => c.id === id);
    if (command && !command.disabled) (command.key ?? command.run)();
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="shell">
  <nav class="rail" aria-label="Modules">
    <button type="button" aria-pressed={shown === 'git'} title="Repositories" aria-label="Repositories" onclick={() => show('git')}>
      <Icon name="branch" size={18} />
    </button>
    <button type="button" aria-pressed={shown === 'ssh'} title="SSH terminal" aria-label="SSH terminal" onclick={() => show('ssh')}>
      <Icon name="terminal" size={18} />
    </button>
    <button type="button" aria-pressed={shown === 'files'} title="Files" aria-label="Files" onclick={() => show('files')}>
      <Icon name="folder" size={18} />
    </button>
    <button class="last" type="button" title="Command palette{keys.binding('palette') ? ` (${display(keys.binding('palette'))})` : ''}" aria-label="Command palette" onclick={openPalette}>
      <Icon name="search" size={18} />
    </button>
    <button
      bind:this={settingsButton}
      type="button"
      aria-pressed={settingsPage.open}
      title="Settings{keys.binding('settings') ? ` (${display(keys.binding('settings'))})` : ''}{updates.waiting ? ' · Update available' : ''}"
      aria-label={updates.waiting ? 'Settings, update available' : 'Settings'}
      onclick={toggleSettings}
    >
      <Icon name="settings" size={18} />
      <!-- A version waits: said by the tooltip and the name of the button too (guidelines, 9.23). -->
      {#if updates.waiting}<span class="dot" aria-hidden="true"></span>{/if}
    </button>
  </nav>

  <!-- The views stay mounted: an SSH session survives a visit to the graph or to the settings. -->
  <main>
    <section class="pane" class:hidden={shown !== 'git'} inert={shown !== 'git'}><GitWorkspace active={shown === 'git'} /></section>
    <section class="pane" class:hidden={shown !== 'ssh'} inert={shown !== 'ssh'}><SshWorkspace active={shown === 'ssh'} /></section>
    <section class="pane" class:hidden={shown !== 'files'} inert={shown !== 'files'} aria-label="Files"><FilesWorkspace /></section>
    <section class="pane" class:hidden={shown !== 'settings'} inert={shown !== 'settings'} aria-label="Settings"><SettingsPage /></section>
  </main>
</div>

<!-- Layers shared by the whole application. -->
<CommandPalette />
<Dialog />
<ContextMenu />
<Notifications />

<style>
  .shell {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr);
    height: 100%;
    /* The window never scrolls: what scrolls is inside the cards. A content a few pixels too tall
       let the page be scrolled by a script, CodeMirror bringing a line into view, and the top of the
       window was cut. `clip`, unlike `hidden`, makes no box that can be scrolled. */
    overflow: clip;
  }
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    background: var(--side);
  }
  .rail button {
    position: relative;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    display: grid;
    place-items: center;
    color: var(--muted);
    background: none;
    border: 0;
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .rail button:hover {
    background: var(--hover-side);
    color: var(--ink);
  }
  .rail .last {
    margin-top: auto;
  }
  .rail button:active {
    transform: scale(0.97);
  }
  .rail button[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--accent);
  }
  /* Marker of the active module, which slides along the rail. */
  .rail button::before {
    content: '';
    position: absolute;
    left: -6px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 var(--radius-pill) var(--radius-pill) 0;
    background: var(--accent);
    transform: scaleY(0);
    transition: transform var(--t-slow) var(--ease);
  }
  .rail button[aria-pressed='true']::before {
    transform: scaleY(1);
  }
  /* At the top right of the icon, as the badge of a toolbar button (guidelines, 9.2 and 9.23). */
  .dot {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--accent);
  }
  main {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  /* The views are stacked and fade into one another. */
  .pane {
    position: absolute;
    inset: 0;
    min-height: 0;
    transition:
      opacity var(--t-med) var(--ease),
      transform var(--t-med) var(--ease),
      visibility 0s;
  }
  .pane.hidden {
    opacity: 0;
    visibility: hidden;
    transform: translateY(6px);
    pointer-events: none;
    transition:
      opacity var(--t-med) var(--ease),
      transform var(--t-med) var(--ease),
      visibility 0s var(--t-med);
  }
</style>
