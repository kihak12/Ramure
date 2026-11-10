<script lang="ts">
  import GitWorkspace from './lib/git/GitWorkspace.svelte';
  import SshView from './lib/ssh/SshView.svelte';
  import ContextMenu from './lib/ui/ContextMenu.svelte';
  import Dialog from './lib/ui/Dialog.svelte';
  import Icon from './lib/ui/Icon.svelte';
  import Toasts from './lib/ui/Toasts.svelte';

  type View = 'git' | 'ssh';
  let view = $state<View>('git');
</script>

<div class="shell">
  <nav class="rail" aria-label="Modules">
    <button type="button" aria-pressed={view === 'git'} title="Dépôts" aria-label="Dépôts" onclick={() => (view = 'git')}>
      <Icon name="branch" size={19} />
    </button>
    <button type="button" aria-pressed={view === 'ssh'} title="Terminal SSH" aria-label="Terminal SSH" onclick={() => (view = 'ssh')}>
      <Icon name="terminal" size={19} />
    </button>
  </nav>

  <!-- Les deux vues restent montées : une session SSH survit au passage par le graphe. -->
  <main>
    <section class="pane" class:hidden={view !== 'git'} inert={view !== 'git'}><GitWorkspace active={view === 'git'} /></section>
    <section class="pane" class:hidden={view !== 'ssh'} inert={view !== 'ssh'}><SshView active={view === 'ssh'} /></section>
  </main>
</div>

<!-- Couches communes à toute l'application. -->
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
