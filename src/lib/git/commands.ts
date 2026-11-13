// Git commands offered in the palette (Ctrl + K): actions on the repository of the current tab,
// branches, tags, open tabs and recent repositories.

import type { Command } from '../ui/palette.svelte';
import { abortOperation, createBranch, forcePush, stashChanges, switchBranch, switchToRemote } from './actions';
import { bare } from './labels';
import { layout, toggleSide, toggleSides } from './layout.svelte';
import { OPERATION } from './repo.svelte';
import { workspace } from './workspace.svelte';

const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? '' : 's'}`;

export function gitCommands(): Command[] {
  const tab = workspace.active;
  const repo = tab?.repo;
  const open = !!repo?.result;
  const idle = open && !repo.locked;
  const status = repo?.status ?? null;
  const branch = repo?.branch ?? null;
  const out: Command[] = [];
  const add = (command: Command) => out.push(command);

  if (repo && open) {
    const operation = status?.operation;
    if (operation) {
      const name = OPERATION[operation].toLowerCase();
      add({ id: 'op-continue', group: 'Actions', icon: 'play', title: `Continue ${name}`, disabled: !idle || repo.conflicts.length > 0, hint: repo.conflicts.length ? 'conflicts to resolve' : undefined, run: () => void repo.operate('continue') });
      add({ id: 'op-abort', group: 'Actions', icon: 'stop', title: `Abort ${name}…`, disabled: !idle, run: () => void abortOperation(repo) });
    }
    add({ id: 'fetch', group: 'Actions', icon: 'fetch', title: 'Fetch', keywords: 'remote download update', hint: 'git fetch --all --prune', disabled: !idle, run: () => void repo.sync('fetch') });
    add({ id: 'pull', group: 'Actions', icon: 'pull', title: 'Pull', keywords: 'remote update', hint: status?.upstream ?? 'no remote branch', disabled: !idle || !status?.upstream, run: () => void repo.sync('pull') });
    add({ id: 'push', group: 'Actions', icon: 'push', title: status?.upstream || !branch ? 'Push' : 'Publish branch', keywords: 'remote upload publish', hint: status?.ahead ? `${plural(status.ahead, 'commit')} to push` : undefined, disabled: !idle || !branch, run: () => void repo.sync('push') });
    add({ id: 'push-force', group: 'Actions', icon: 'push', title: 'Force push…', keywords: 'push force lease', disabled: !idle || !status?.upstream, run: () => void forcePush(repo) });
    add({ id: 'branch', group: 'Actions', icon: 'branch', title: 'Create branch…', keywords: 'new branch', disabled: !idle || !repo.head, run: () => repo.head && void createBranch(repo, repo.head.sha, branch ?? 'HEAD') });
    add({ id: 'wip', group: 'Actions', icon: 'pencil', title: 'Show working changes', keywords: 'stage index wip commit', hint: repo.wipCount ? plural(repo.wipCount, 'file') : 'none', disabled: repo.wipCount === 0, run: () => repo.showWip() });
    add({ id: 'stage-all', group: 'Actions', icon: 'plus', title: 'Stage all', keywords: 'add index', disabled: !idle || !status?.unstaged.length, run: () => void repo.stage(status?.unstaged ?? []) });
    add({ id: 'stash', group: 'Actions', icon: 'stash', title: 'Stash changes…', keywords: 'set aside save', disabled: !idle || repo.wipCount === 0, run: () => void stashChanges(repo) });
    add({ id: 'pop', group: 'Actions', icon: 'pop', title: 'Pop latest stash', keywords: 'stash pop apply', disabled: !idle || !repo.refs?.stashes.length, run: () => void repo.refOp({ op: 'stash-pop', index: 0 }, 'Stash popped', 'Could not pop stash') });
    add({ id: 'reload', group: 'Actions', icon: 'refresh', title: 'Reload repository', disabled: !idle, run: () => void repo.reload() });
    // The title says what the command will do: it follows the state of the card.
    add({ id: 'toggle-sidebar', group: 'View', icon: 'panel-left', title: layout.sidebarHidden ? 'Show branch list' : 'Hide branch list', keywords: 'panel sidebar collapse', run: () => toggleSide('sidebar') });
    add({ id: 'toggle-detail', group: 'View', icon: 'panel-right', title: layout.detailHidden ? 'Show detail panel' : 'Hide detail panel', keywords: 'panel commit collapse', run: () => toggleSide('detail') });
    // The terminal of the repository (guidelines, section 9.20). The shortcut does more than the
    // palette: it also gives the keyboard to the terminal, and hides it if the keyboard is already there.
    const terminals = repo.terminals;
    add({
      id: 'toggle-terminal',
      group: 'View',
      icon: 'terminal',
      title: terminals.shown ? 'Hide terminal' : 'Show terminal',
      keywords: 'console shell command line panel',
      // The editor of a conflict covers the view, terminal included: nothing opens under it.
      disabled: !!repo.conflictFile,
      run: () => terminals.toggle(repo.path),
      key: () => terminals.key(repo.path, !!document.activeElement?.closest('[data-terminals]')),
    });
    add({
      id: 'toggle-graph',
      group: 'View',
      icon: 'commit',
      title: terminals.alone ? 'Show commit graph' : 'Hide commit graph',
      keywords: 'terminal full height history',
      // The center is never empty: the graph only hides in front of the terminal.
      hint: terminals.shown ? undefined : 'the terminal must be shown',
      disabled: !terminals.shown || !!repo.conflictFile,
      run: () => terminals.toggleGraph(),
    });
    add({ id: 'terminal-repo', group: 'View', icon: 'laptop', title: 'New terminal in this repository', keywords: 'console shell command line', disabled: !!repo.conflictFile, run: () => terminals.open(repo.path) });
    add({ id: 'toggle-sides', group: 'View', icon: 'panels', title: layout.sidebarHidden && layout.detailHidden ? 'Show side panels' : 'Hide side panels', keywords: 'graph full width collapse', run: toggleSides });
  }
  add({ id: 'open', group: 'Actions', icon: 'folder', title: 'Open repository…', run: () => void workspace.pick() });
  add({ id: 'clone', group: 'Actions', icon: 'cloud', title: 'Clone repository…', keywords: 'clone', run: () => (workspace.cloning = true) });
  add({ id: 'init', group: 'Actions', icon: 'plus', title: 'Initialize repository…', keywords: 'init new create', run: () => void workspace.init() });
  add({ id: 'tab-new', group: 'Actions', icon: 'plus', title: 'New tab', run: () => workspace.newTab() });
  if (tab) add({ id: 'tab-close', group: 'Actions', icon: 'x', title: 'Close tab', run: () => workspace.closeTab(tab.id) });
  if (workspace.tabs.length > 1) {
    add({ id: 'tab-next', group: 'Actions', icon: 'chevron', title: 'Next tab', run: () => workspace.cycle(1) });
    add({ id: 'tab-prev', group: 'Actions', icon: 'chevron', title: 'Previous tab', run: () => workspace.cycle(-1) });
  }

  if (repo && open && repo.refs) {
    const locals = new Set(repo.refs.branches.map((b) => b.name));
    for (const b of repo.refs.branches) {
      add({
        id: `branch:${b.name}`,
        group: 'Branches',
        icon: b.current ? 'check' : 'branch',
        title: b.current ? `${b.name} (current branch)` : `Switch to ${b.name}`,
        keywords: 'checkout switch',
        hint: [b.ahead ? `↑ ${b.ahead}` : '', b.behind ? `↓ ${b.behind}` : ''].filter(Boolean).join('  ') || undefined,
        disabled: b.current || !idle,
        run: () => void switchBranch(repo, b.name),
      });
    }
    for (const r of repo.refs.remotes) {
      // A remote branch that already has its local branch is covered by it.
      if (locals.has(bare(r.name))) continue;
      add({
        id: `remote:${r.name}`,
        group: 'Branches',
        icon: 'cloud',
        title: `Switch to ${r.name}`,
        keywords: 'checkout switch remote',
        hint: repo.refs.branches.some((b) => b.name === bare(r.name)) ? 'updates the local branch' : 'creates the local branch',
        disabled: !idle,
        run: () => void switchToRemote(repo, r.name),
      });
    }
    for (const t of [...repo.refs.tags].reverse()) {
      add({ id: `tag:${t.name}`, group: 'Tags', icon: 'tag', title: `Go to tag ${t.name}`, run: () => repo.selectSha(t.sha) });
    }
  }

  for (const other of workspace.tabs) {
    if (other === tab || !other.repo.path) continue;
    add({ id: `tab:${other.id}`, group: 'Tabs', icon: 'branch', title: `Go to tab ${workspace.title(other)}`, hint: other.repo.path, run: () => workspace.activate(other.id) });
  }
  const opened = new Set(workspace.tabs.map((t) => t.repo.path));
  for (const recent of workspace.recents) {
    if (opened.has(recent.path)) continue;
    add({ id: `recent:${recent.path}`, group: 'Recent repositories', icon: 'folder', title: `Open ${recent.name}`, hint: recent.path, run: () => void workspace.openPath(recent.path) });
  }
  return out;
}
