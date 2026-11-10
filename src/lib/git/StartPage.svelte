<script lang="ts">
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import type { Repo } from './repo.svelte';
  import { workspace } from './workspace.svelte';

  let { repo }: { repo: Repo } = $props();

  const recents = $derived(workspace.recents);
  const relative = new Intl.RelativeTimeFormat('fr-FR', { numeric: 'auto' });

  function ago(at: number) {
    const minutes = Math.round((at - Date.now()) / 60_000);
    if (Math.abs(minutes) < 60) return relative.format(minutes, 'minute');
    const hours = Math.round(minutes / 60);
    if (Math.abs(hours) < 24) return relative.format(hours, 'hour');
    return relative.format(Math.round(hours / 24), 'day');
  }
</script>

{#snippet action(icon: IconName, title: string, text: string, run: () => void)}
  <button class="action" type="button" onclick={run}>
    <span class="glyph"><Icon name={icon} size={20} /></span>
    <span class="words"><b>{title}</b><small>{text}</small></span>
  </button>
{/snippet}

<div class="start">
  <div class="inner">
    <header>
      <span class="mark"><Icon name="branch" size={30} /></span>
      <div>
        <h1>Ramure</h1>
        <p>Ouvre un dépôt pour afficher son graphe.</p>
      </div>
    </header>

    {#if repo.error}
      <p class="error" role="alert"><Icon name="warning" /> <span>{repo.error}</span></p>
    {/if}

    <div class="actions">
      {@render action('folder', 'Ouvrir un dépôt', 'Un dossier déjà présent sur cette machine', () => void workspace.pick())}
      {@render action('cloud', 'Cloner un dépôt', 'Depuis une adresse HTTPS ou SSH', () => (workspace.cloning = true))}
      {@render action('plus', 'Initialiser un dépôt', 'Dans un dossier existant', () => void workspace.init())}
    </div>

    {#if recents.length}
      <section>
        <span class="label">Dépôts récents</span>
        <ul>
          {#each recents as recent, i (recent.path)}
            <li style:--i={i}>
              <button class="recent" type="button" onclick={() => void workspace.openPath(recent.path)} title={recent.path}>
                <Icon name="branch" size={15} />
                <b>{recent.name}</b>
                <span class="path mono">{recent.path}</span>
                <span class="when">{ago(recent.at)}</span>
              </button>
              <button class="forget" type="button" aria-label="Retirer {recent.name} de la liste" title="Retirer de la liste" onclick={() => workspace.forget(recent.path)}>
                <Icon name="x" size={13} />
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  </div>
</div>

<style>
  .start {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    place-items: start center;
    margin: 8px;
    padding: 9vh 24px 32px;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  .inner {
    width: min(680px, 100%);
    display: grid;
    gap: 22px;
    animation: rise var(--t-slow) var(--ease) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border-radius: var(--radius-card);
    color: var(--accent);
    background: var(--accent-soft);
  }
  h1 {
    margin: 0;
    font-size: var(--fs-display);
    font-weight: var(--fw-strong);
    letter-spacing: -0.01em;
  }
  header p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: var(--fs-body);
  }
  .error {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--bad-soft);
    color: var(--bad);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .error span {
    color: var(--ink);
  }
  .actions {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }
  .action {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-soft);
    cursor: pointer;
    text-align: left;
    transition:
      transform var(--t-med) var(--ease),
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-med) var(--ease);
  }
  .action:hover {
    transform: translateY(-2px);
    border-color: var(--accent);
    box-shadow: var(--shadow-pop);
  }
  .action:active {
    transform: scale(0.98);
  }
  .glyph {
    flex: none;
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: var(--radius);
    color: var(--accent);
    background: var(--accent-soft);
  }
  .words {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }
  .words small {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  section {
    display: grid;
    gap: 8px;
  }
  ul {
    list-style: none;
    margin: 0 -6px;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    animation: rise var(--t-slow) var(--ease) both;
    animation-delay: calc(var(--i) * var(--t-step) + var(--t-fast));
    transition: background var(--t-fast) var(--ease);
  }
  li:hover,
  li:focus-within {
    background: var(--hover);
  }
  .recent {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 10px;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
    color: var(--ink);
  }
  .recent :global(svg) {
    color: var(--muted);
  }
  .recent b {
    flex: none;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .when {
    flex: none;
    color: var(--faint);
    font-size: var(--fs-small);
  }
  .forget {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-right: 6px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  li:hover .forget,
  .forget:focus-visible {
    opacity: 1;
  }
  .forget:hover {
    background: var(--line);
    color: var(--ink);
  }
</style>
