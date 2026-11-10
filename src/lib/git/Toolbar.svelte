<script lang="ts">
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import { createBranch, editSettings, stashChanges } from './actions';
  import type { Repo } from './repo.svelte';

  let { repo, onpick }: { repo: Repo; onpick: () => void } = $props();

  const status = $derived(repo.status);
  const ready = $derived(!!repo.result && !repo.locked);
  const stashes = $derived(repo.refs?.stashes.length ?? 0);

  function popStash() {
    void repo.refOp({ op: 'stash-pop', index: 0 }, 'Remise appliquée', "La remise n'a pas pu être appliquée");
  }
</script>

{#snippet tool(icon: IconName, label: string, title: string, disabled: boolean, run: () => void, badge?: string, spin?: boolean)}
  <button class="tool" type="button" {title} {disabled} onclick={run}>
    <span class="glyph" class:spin>
      <Icon name={icon} size={18} />
      {#if badge}<span class="badge">{badge}</span>{/if}
    </span>
    <span class="text">{label}</span>
  </button>
{/snippet}

<header class="toolbar">
  <button class="crumb" type="button" onclick={onpick} disabled={repo.locked} title="Ouvrir un autre dépôt…">
    <span class="small">dépôt</span>
    <b>{repo.result?.name ?? 'Aucun dépôt'}</b>
  </button>
  <span class="sep"><Icon name="chevron" size={14} /></span>
  <div class="crumb static">
    <span class="small">branche</span>
    <b class="mono">{repo.result ? (repo.branch ?? 'HEAD détaché') : '—'}</b>
  </div>

  <div class="tools" role="group" aria-label="Synchronisation">
    {@render tool('fetch', 'Récupérer', 'git fetch --all --prune', !ready, () => void repo.sync('fetch'), undefined, repo.busy === 'fetch')}
    {@render tool(
      'pull',
      'Tirer',
      status?.upstream ? `git pull depuis ${status.upstream}` : "Cette branche n'a pas de branche distante associée",
      !ready || !status?.upstream,
      () => void repo.sync('pull'),
      status?.behind ? String(status.behind) : undefined,
    )}
    {@render tool(
      'push',
      status && status.branch && !status.upstream ? 'Publier' : 'Pousser',
      status?.upstream ? `git push vers ${status.upstream}` : 'Publie la branche et la relie à son homologue distante',
      !ready || !status?.branch,
      () => void repo.sync('push'),
      status?.ahead ? String(status.ahead) : undefined,
    )}
  </div>
  <div class="tools" role="group" aria-label="Branches et remises">
    {@render tool('branch', 'Branche', 'Créer une branche à partir du commit courant', !ready || !repo.head, () => {
      if (repo.head) void createBranch(repo, repo.head.sha, repo.branch ?? 'HEAD');
    })}
    {@render tool('stash', 'Remiser', 'Mettre de côté les modifications en cours', !ready || repo.wipCount === 0, () => void stashChanges(repo))}
    {@render tool('pop', 'Dépiler', 'Appliquer la dernière remise et la supprimer', !ready || stashes === 0, popStash, stashes ? String(stashes) : undefined)}
  </div>

  <div class="tools right">
    {@render tool('refresh', 'Recharger', "Relire l'historique du dépôt", !ready, () => void repo.reload())}
    {@render tool('folder', 'Ouvrir', 'Ouvrir un autre dépôt…', repo.locked, onpick)}
    {@render tool('settings', 'Réglages', 'Fetch automatique, stratégie de pull, taille du graphe', false, () => void editSettings(repo))}
  </div>
</header>

<style>
  .toolbar {
    flex: none;
    display: flex;
    align-items: stretch;
    gap: 4px;
    padding: 4px 8px 4px 6px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    min-width: 0;
  }
  .crumb {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 70px;
    max-width: 220px;
    padding: 3px 8px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    text-align: left;
    line-height: 1.25;
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .crumb.static {
    cursor: default;
  }
  button.crumb:hover:not(:disabled) {
    background: var(--hover);
  }
  .crumb b,
  .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .crumb b {
    font-size: 13.5px;
  }
  .small {
    color: var(--muted);
    font-size: 10.5px;
  }
  .sep {
    display: grid;
    place-items: center;
    color: var(--faint);
  }
  .tools {
    display: flex;
    gap: 2px;
    margin-left: 8px;
    padding-left: 8px;
    border-left: 1px solid var(--line);
  }
  .tools.right {
    margin-left: auto;
    border-left: 0;
  }
  .tool {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-width: 58px;
    padding: 4px 6px 3px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    cursor: pointer;
    color: var(--ink);
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease),
      opacity var(--t-fast) var(--ease);
  }
  .tool:hover:not(:disabled) {
    background: var(--hover);
  }
  .tool:hover:not(:disabled) .glyph {
    transform: translateY(-1px);
  }
  .tool:active:not(:disabled) {
    transform: scale(0.95);
  }
  .tool:disabled {
    opacity: 0.38;
    cursor: not-allowed;
  }
  .glyph {
    position: relative;
    display: grid;
    transition: transform var(--t-med) var(--ease);
  }
  .glyph.spin :global(svg) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .text {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .badge {
    position: absolute;
    top: -5px;
    left: 13px;
    min-width: 15px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 9.5px;
    font-weight: 700;
    line-height: 15px;
    font-variant-numeric: tabular-nums;
    animation: pop-in var(--t-med) var(--ease);
  }
</style>
