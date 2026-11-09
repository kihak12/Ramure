<script lang="ts">
  import GraphView from './lib/git/GraphView.svelte';
  import SshView from './lib/ssh/SshView.svelte';

  type View = 'git' | 'ssh';
  let view = $state<View>('git');
</script>

<div class="shell">
  <nav class="rail" aria-label="Modules">
    <button type="button" aria-pressed={view === 'git'} title="Dépôts" aria-label="Dépôts" onclick={() => (view = 'git')}>
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="6" cy="4.5" r="2" /><circle cx="6" cy="15.5" r="2" /><circle cx="14" cy="6.5" r="2" />
        <path d="M6 6.5v7M14 8.5c0 4-8 2.5-8 5" />
      </svg>
    </button>
    <button type="button" aria-pressed={view === 'ssh'} title="Terminal SSH" aria-label="Terminal SSH" onclick={() => (view = 'ssh')}>
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <rect x="2.5" y="3.5" width="15" height="13" rx="2" /><path d="m6 8 3 2-3 2M10.5 13H14" />
      </svg>
    </button>
  </nav>

  <!-- Les deux vues restent montées : une session SSH survit au passage par le graphe. -->
  <main>
    <section class="pane" hidden={view !== 'git'}><GraphView /></section>
    <section class="pane" hidden={view !== 'ssh'}><SshView active={view === 'ssh'} /></section>
  </main>
</div>

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
    width: 36px;
    height: 36px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: var(--muted);
    background: none;
    border: 0;
    cursor: pointer;
  }
  .rail button:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .rail button[aria-pressed='true'] {
    background: var(--sel);
    color: var(--ink);
  }
  .rail svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  main {
    min-width: 0;
    min-height: 0;
  }
  .pane {
    height: 100%;
    min-height: 0;
  }
  .pane[hidden] {
    display: none;
  }
</style>
