<script lang="ts">
  import { gitCommands } from './lib/git/commands';
  import GitWorkspace from './lib/git/GitWorkspace.svelte';
  import SshView from './lib/ssh/SshView.svelte';
  import ContextMenu from './lib/ui/ContextMenu.svelte';
  import Dialog from './lib/ui/Dialog.svelte';
  import Icon from './lib/ui/Icon.svelte';
  import Palette from './lib/ui/Palette.svelte';
  import { palette, type Command } from './lib/ui/palette.svelte';
  import Toasts from './lib/ui/Toasts.svelte';

  type View = 'git' | 'ssh';
  let view = $state<View>('git');

  function openPalette() {
    const views: Command[] = [
      { id: 'view-git', group: 'Modules', icon: 'branch', title: 'Afficher les dépôts', disabled: view === 'git', run: () => (view = 'git') },
      { id: 'view-ssh', group: 'Modules', icon: 'terminal', title: 'Afficher le terminal SSH', disabled: view === 'ssh', run: () => (view = 'ssh') },
    ];
    // Les actions Git n'ont de sens que devant un dépôt.
    palette.show(view === 'git' ? [...gitCommands(), ...views] : views);
  }

  function onKey(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    const key = e.key.toLowerCase();
    // Dans un terminal, Ctrl + K efface la fin de la ligne : on le lui laisse.
    const inTerminal = e.target instanceof Element && e.target.closest('.xterm') !== null;
    if ((key === 'k' && !e.shiftKey && !inTerminal) || (key === 'p' && e.shiftKey)) {
      e.preventDefault();
      e.stopPropagation();
      if (palette.commands) palette.hide();
      else openPalette();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="shell">
  <nav class="rail" aria-label="Modules">
    <button type="button" aria-pressed={view === 'git'} title="Dépôts" aria-label="Dépôts" onclick={() => (view = 'git')}>
      <Icon name="branch" size={19} />
    </button>
    <button type="button" aria-pressed={view === 'ssh'} title="Terminal SSH" aria-label="Terminal SSH" onclick={() => (view = 'ssh')}>
      <Icon name="terminal" size={19} />
    </button>
    <button class="last" type="button" title="Palette de commandes (Ctrl + K)" aria-label="Palette de commandes" onclick={openPalette}>
      <Icon name="search" size={18} />
    </button>
  </nav>

  <!-- Les deux vues restent montées : une session SSH survit au passage par le graphe. -->
  <main>
    <section class="pane" class:hidden={view !== 'git'} inert={view !== 'git'}><GitWorkspace active={view === 'git'} /></section>
    <section class="pane" class:hidden={view !== 'ssh'} inert={view !== 'ssh'}><SshView active={view === 'ssh'} /></section>
  </main>
</div>

<!-- Couches communes à toute l'application. -->
<Palette />
<Dialog />
<ContextMenu />
<Toasts />

<style>
  .shell {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr);
    height: 100%;
  }
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    background: var(--side);
    border-right: 1px solid var(--line);
  }
  .rail button {
    position: relative;
    width: 36px;
    height: 36px;
    border-radius: 9px;
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
    background: var(--hover);
    color: var(--ink);
  }
  .rail .last {
    margin-top: auto;
  }
  .rail button:active {
    transform: scale(0.93);
  }
  .rail button[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--accent);
  }
  /* Repère du module actif, qui glisse le long du rail. */
  .rail button::before {
    content: '';
    position: absolute;
    left: -6px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
    transform: scaleY(0);
    transition: transform var(--t-med) var(--ease);
  }
  .rail button[aria-pressed='true']::before {
    transform: scaleY(1);
  }
  main {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  /* Les vues sont superposées et se fondent l'une dans l'autre. */
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
