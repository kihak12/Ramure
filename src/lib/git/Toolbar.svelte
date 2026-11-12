<script lang="ts">
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import { display, keys } from '../ui/keys.svelte';
  import { menu, type MenuItem } from '../ui/menu.svelte';
  import { createBranch, stashChanges } from './actions';
  import { layout, toggleSide } from './layout.svelte';
  import type { Repo } from './repo.svelte';

  let { repo, onpick }: { repo: Repo; onpick: () => void } = $props();

  const status = $derived(repo.status);
  const ready = $derived(!!repo.result && !repo.locked);
  const stashes = $derived(repo.refs?.stashes.length ?? 0);
  // Terminal du dépôt (charte, section 9.20) : l'action est enfoncée quand il est affiché ; masqué,
  // sa pastille compte les shells qui tournent encore.
  const terminals = $derived(repo.terminals);
  const terminalCombo = $derived(keys.binding('toggle-terminal'));
  const terminalBadge = $derived(!terminals.shown && terminals.running ? String(terminals.running) : undefined);

  // Menu « Affichage » : une case par carte de la vue, de gauche à droite (charte, sections 9.2 et 9.17).
  // Le centre garde toujours le graphe ou le terminal : la case de celui qui y est seul est désactivée.
  let viewsEl = $state<HTMLButtonElement>();
  function views(): MenuItem[] {
    const graphAlone = !terminals.shown;
    return [
      { label: 'Liste des branches', icon: 'panel-left', checked: !layout.sidebarHidden, keys: keys.binding('toggle-sidebar'), run: () => toggleSide('sidebar') },
      {
        label: 'Graphe des commits',
        icon: 'commit',
        checked: !terminals.alone,
        keys: keys.binding('toggle-graph'),
        disabled: graphAlone,
        hint: graphAlone ? 'Le graphe reste affiché tant que le terminal ne l’est pas.' : undefined,
        run: () => terminals.toggleGraph(),
      },
      {
        label: 'Terminal',
        icon: 'terminal',
        checked: terminals.shown,
        keys: terminalCombo,
        disabled: terminals.alone,
        hint: terminals.alone ? 'Le terminal reste affiché tant que le graphe ne l’est pas.' : undefined,
        run: () => terminals.toggle(repo.path),
      },
      { label: 'Panneau de détail', icon: 'panel-right', checked: !layout.detailHidden, keys: keys.binding('toggle-detail'), run: () => toggleSide('detail') },
    ];
  }

  function popStash() {
    void repo.refOp({ op: 'stash-pop', index: 0 }, 'Remise appliquée', 'La remise n’a pas pu être appliquée');
  }
</script>

{#snippet tool(icon: IconName, label: string, title: string, disabled: boolean, run: () => void, badge?: string, spin?: boolean, pressed?: boolean)}
  <button class="tool" type="button" {title} {disabled} aria-pressed={pressed} onclick={run}>
    <span class="glyph" class:spin>
      <Icon name={icon} size={18} />
      {#if badge}<span class="badge">{badge}</span>{/if}
    </span>
    <span class="text">{label}</span>
  </button>
{/snippet}

<header class="toolbar">
  <button
    class="crumb"
    type="button"
    onclick={onpick}
    disabled={repo.locked}
    title={repo.result ? `${repo.result.name} — ouvrir un autre dépôt…` : 'Ouvrir un dépôt…'}
  >
    <span class="small">dépôt</span>
    <b>{repo.result?.name ?? 'Aucun dépôt'}</b>
  </button>
  <span class="sep"><Icon name="chevron" size={14} /></span>
  <div class="crumb static" title={repo.result ? (repo.branch ?? 'HEAD détaché') : undefined}>
    <span class="small">branche</span>
    <b class="mono">{repo.result ? (repo.branch ?? 'HEAD détaché') : '—'}</b>
  </div>

  <div class="tools" role="group" aria-label="Synchronisation">
    {@render tool('fetch', 'Récupérer', 'git fetch --all --prune', !ready, () => void repo.sync('fetch'), undefined, repo.busy === 'fetch')}
    {@render tool(
      'pull',
      'Tirer',
      status?.upstream ? `git pull depuis ${status.upstream}` : 'Cette branche n’a pas de branche distante associée',
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
    {@render tool(
      'terminal',
      'Terminal',
      `${terminals.shown ? 'Masquer' : 'Afficher'} le terminal du dépôt${terminalCombo ? ` (${display(terminalCombo)})` : ''}`,
      !repo.result || !!repo.conflictFile,
      () => terminals.toggle(repo.path),
      terminalBadge,
      false,
      terminals.shown,
    )}
    {@render tool('refresh', 'Recharger', 'Relire l’historique du dépôt', !ready, () => void repo.reload())}
    {@render tool('folder', 'Ouvrir', 'Ouvrir un autre dépôt…', repo.locked, onpick)}
  </div>
  <!-- « Affichage » : le seul bouton de vue, il déroule le choix des cartes. Menu ouvert, son
       infobulle se tait : elle couvrirait le premier élément. L'éditeur d'un conflit couvre les
       cartes : il n'y a rien à choisir tant qu'il est ouvert. -->
  <button
    class="view"
    type="button"
    bind:this={viewsEl}
    aria-label="Affichage"
    aria-haspopup="menu"
    aria-expanded={menu.isUnder(viewsEl)}
    title={menu.isUnder(viewsEl) ? undefined : 'Choisir les cartes à afficher'}
    disabled={!repo.result || !!repo.conflictFile}
    onclick={(event) => viewsEl && menu.openUnder(viewsEl, views, event.detail === 0)}
  >
    <Icon name="panels" size={18} />
  </button>
</header>

<style>
  .toolbar {
    flex: none;
    display: flex;
    align-items: stretch;
    gap: 4px;
    /* 8 px sur les côtés : le fil d'Ariane et « Affichage » s'alignent sur les bords des cartes. */
    padding: 6px 8px;
    min-width: 0;
  }
  .crumb {
    display: flex;
    flex-direction: column;
    justify-content: center;
    /* Largeur fixe : les actions gardent leur place, quel que soit le nom. Elle ne cède qu'à une fenêtre étroite. */
    flex: 0 1 140px;
    min-width: 70px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    text-align: left;
    line-height: 1.25;
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .crumb.static {
    flex-basis: 200px;
    cursor: default;
  }
  button.crumb:hover:not(:disabled) {
    background: var(--hover-side);
  }
  .crumb b,
  .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .crumb b {
    font-size: var(--fs-body);
    font-weight: var(--fw-strong);
  }
  .small {
    color: var(--muted);
    font-size: var(--fs-small);
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
    padding: 4px 6px;
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
    background: var(--hover-side);
  }
  .tool:hover:not(:disabled) .glyph {
    transform: translateY(-1px);
  }
  .tool:active:not(:disabled) {
    transform: scale(0.97);
  }
  /* « Terminal » enfoncé : la teinte du module actif dans le rail, l'icône en accent, le libellé tel quel. */
  .tool[aria-pressed='true'] {
    background: var(--accent-soft);
  }
  .tool[aria-pressed='true'] .glyph {
    color: var(--accent);
  }
  .tool[aria-pressed='true']:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 12%, var(--accent-soft));
  }
  .tool:disabled,
  .crumb:disabled,
  .view:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  /* « Affichage », au bout droit de la barre : une icône seule (charte, section 9.2). */
  .view {
    flex: none;
    align-self: center;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .view:hover:not(:disabled),
  .view[aria-expanded='true'] {
    background: var(--hover-side);
    color: var(--ink);
  }
  .view:active:not(:disabled) {
    transform: scale(0.97);
  }
  .glyph {
    position: relative;
    display: grid;
    transition: transform var(--t-med) var(--ease);
  }
  .glyph.spin :global(svg) {
    animation: spin var(--t-spin) linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .text {
    font-size: var(--fs-small);
    color: var(--muted);
    white-space: nowrap;
  }
  .badge {
    position: absolute;
    top: -5px;
    left: 13px;
    min-width: 15px;
    padding: 0 4px;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--on-accent);
    font-size: var(--fs-micro);
    font-weight: var(--fw-strong);
    line-height: 15px;
    font-variant-numeric: tabular-nums;
    animation: pop-in var(--t-med) var(--ease);
  }
</style>
