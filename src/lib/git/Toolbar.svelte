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
  // Terminal of the repository (guidelines, section 9.20): the action is pressed while it is shown;
  // hidden, its badge counts the shells still running.
  const terminals = $derived(repo.terminals);
  const terminalCombo = $derived(keys.binding('toggle-terminal'));
  const terminalBadge = $derived(!terminals.shown && terminals.running ? String(terminals.running) : undefined);

  // “View” menu: one item per card of the view, from left to right (guidelines, sections 9.2 and
  // 9.17). The center always keeps the graph or the terminal: the item of the one alone there is disabled.
  let viewsEl = $state<HTMLButtonElement>();
  function views(): MenuItem[] {
    const graphAlone = !terminals.shown;
    return [
      { label: 'Branch list', icon: 'panel-left', checked: !layout.sidebarHidden, keys: keys.binding('toggle-sidebar'), run: () => toggleSide('sidebar') },
      {
        label: 'Commit graph',
        icon: 'commit',
        checked: !terminals.alone,
        keys: keys.binding('toggle-graph'),
        disabled: graphAlone,
        hint: graphAlone ? 'The graph stays shown while the terminal is hidden.' : undefined,
        run: () => terminals.toggleGraph(),
      },
      {
        label: 'Terminal',
        icon: 'terminal',
        checked: terminals.shown,
        keys: terminalCombo,
        disabled: terminals.alone,
        hint: terminals.alone ? 'The terminal stays shown while the graph is hidden.' : undefined,
        run: () => terminals.toggle(repo.path),
      },
      { label: 'Detail panel', icon: 'panel-right', checked: !layout.detailHidden, keys: keys.binding('toggle-detail'), run: () => toggleSide('detail') },
    ];
  }

  function popStash() {
    void repo.refOp({ op: 'stash-pop', index: 0 }, 'Stash popped', 'Could not pop stash');
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
    title={repo.result ? `${repo.result.name} — open another repository…` : 'Open a repository…'}
  >
    <span class="small">Repository</span>
    <b>{repo.result?.name ?? 'No repository'}</b>
  </button>
  <span class="sep"><Icon name="chevron" size={14} /></span>
  <div class="crumb static" title={repo.result ? (repo.branch ?? 'Detached HEAD') : undefined}>
    <span class="small">Branch</span>
    <b class="mono">{repo.result ? (repo.branch ?? 'detached HEAD') : '—'}</b>
  </div>

  <div class="tools" role="group" aria-label="Sync">
    {@render tool('fetch', 'Fetch', 'git fetch --all --prune', !ready, () => void repo.sync('fetch'), undefined, repo.busy === 'fetch')}
    {@render tool(
      'pull',
      'Pull',
      status?.upstream ? `git pull from ${status.upstream}` : 'This branch is not linked to a remote branch',
      !ready || !status?.upstream,
      () => void repo.sync('pull'),
      status?.behind ? String(status.behind) : undefined,
    )}
    {@render tool(
      'push',
      status && status.branch && !status.upstream ? 'Publish' : 'Push',
      status?.upstream ? `git push to ${status.upstream}` : 'Publish the branch and link it to its counterpart on the remote',
      !ready || !status?.branch,
      () => void repo.sync('push'),
      status?.ahead ? String(status.ahead) : undefined,
    )}
  </div>
  <div class="tools" role="group" aria-label="Branches and stashes">
    {@render tool('branch', 'Branch', 'Create a branch from the current commit', !ready || !repo.head, () => {
      if (repo.head) void createBranch(repo, repo.head.sha, repo.branch ?? 'HEAD');
    })}
    {@render tool('stash', 'Stash', 'Set the working changes aside', !ready || repo.wipCount === 0, () => void stashChanges(repo))}
    {@render tool('pop', 'Pop', 'Apply the latest stash and drop it', !ready || stashes === 0, popStash, stashes ? String(stashes) : undefined)}
  </div>

  <div class="tools right">
    {@render tool(
      'terminal',
      'Terminal',
      `${terminals.shown ? 'Hide' : 'Show'} the terminal of this repository${terminalCombo ? ` (${display(terminalCombo)})` : ''}`,
      !repo.result || !!repo.conflictFile,
      () => terminals.toggle(repo.path),
      terminalBadge,
      false,
      terminals.shown,
    )}
    {@render tool('refresh', 'Reload', 'Read the history of the repository again', !ready, () => void repo.reload())}
    {@render tool('folder', 'Open', 'Open another repository…', repo.locked, onpick)}
  </div>
  <!-- “View”: the one view button, it opens the choice of the cards. While its menu is open, its
       tooltip keeps quiet: it would cover the first item. The editor of a conflict covers the
       cards: there is nothing to choose while it is open. -->
  <button
    class="view"
    type="button"
    bind:this={viewsEl}
    aria-label="View"
    aria-haspopup="menu"
    aria-expanded={menu.isUnder(viewsEl)}
    title={menu.isUnder(viewsEl) ? undefined : 'Choose the panels to show'}
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
    /* 8 px on the sides: the breadcrumb and “View” line up with the edges of the cards. */
    padding: 6px 8px;
    min-width: 0;
  }
  .crumb {
    display: flex;
    flex-direction: column;
    justify-content: center;
    /* Fixed width: the actions keep their place, whatever the name. It only yields to a narrow window. */
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
  /* “Terminal” pressed: the tint of the active module in the rail, the icon in accent, the label as it is. */
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
  /* “View”, at the right end of the bar: an icon alone (guidelines, section 9.2). */
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
