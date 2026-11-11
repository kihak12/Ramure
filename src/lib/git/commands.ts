// Commandes Git proposées dans la palette (Ctrl + K) : actions sur le dépôt de l'onglet
// courant, branches, tags, onglets ouverts et dépôts récents.

import type { Command } from '../ui/palette.svelte';
import { createBranch, forcePush, stashChanges, switchBranch, switchToRemote } from './actions';
import { bare } from './labels';
import { layout, toggleSide, toggleSides } from './layout.svelte';
import { OPERATION } from './repo.svelte';
import { workspace } from './workspace.svelte';

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
      const name = OPERATION[operation];
      add({ id: 'op-continue', group: 'Actions', icon: 'play', title: `Continuer : ${name.toLowerCase()}`, disabled: !idle || repo.conflicts.length > 0, hint: repo.conflicts.length ? 'conflits à résoudre' : undefined, run: () => void repo.operate('continue') });
      add({ id: 'op-abort', group: 'Actions', icon: 'stop', title: `Abandonner : ${name.toLowerCase()}`, disabled: !idle, run: () => void repo.operate('abort') });
    }
    add({ id: 'fetch', group: 'Actions', icon: 'fetch', title: 'Récupérer', keywords: 'fetch', hint: 'git fetch --all --prune', disabled: !idle, run: () => void repo.sync('fetch') });
    add({ id: 'pull', group: 'Actions', icon: 'pull', title: 'Tirer', keywords: 'pull', hint: status?.upstream ?? 'pas de branche distante', disabled: !idle || !status?.upstream, run: () => void repo.sync('pull') });
    add({ id: 'push', group: 'Actions', icon: 'push', title: status?.upstream || !branch ? 'Pousser' : 'Publier la branche', keywords: 'push', hint: status?.ahead ? `${status.ahead} commit(s) à pousser` : undefined, disabled: !idle || !branch, run: () => void repo.sync('push') });
    add({ id: 'push-force', group: 'Actions', icon: 'push', title: 'Pousser en forçant…', keywords: 'push force lease', disabled: !idle || !status?.upstream, run: () => void forcePush(repo) });
    add({ id: 'branch', group: 'Actions', icon: 'branch', title: 'Nouvelle branche…', keywords: 'creer branch', disabled: !idle || !repo.head, run: () => repo.head && void createBranch(repo, repo.head.sha, branch ?? 'HEAD') });
    add({ id: 'wip', group: 'Actions', icon: 'pencil', title: 'Voir les modifications en cours', keywords: 'index wip commit', hint: repo.wipCount ? `${repo.wipCount} fichier(s)` : 'aucune', disabled: repo.wipCount === 0, run: () => repo.showWip() });
    add({ id: 'stage-all', group: 'Actions', icon: 'plus', title: 'Tout indexer', keywords: 'stage add', disabled: !idle || !status?.unstaged.length, run: () => void repo.stage(status?.unstaged ?? []) });
    add({ id: 'stash', group: 'Actions', icon: 'stash', title: 'Remiser les modifications…', keywords: 'stash', disabled: !idle || repo.wipCount === 0, run: () => void stashChanges(repo) });
    add({ id: 'pop', group: 'Actions', icon: 'pop', title: 'Dépiler la dernière remise', keywords: 'stash pop', disabled: !idle || !repo.refs?.stashes.length, run: () => void repo.refOp({ op: 'stash-pop', index: 0 }, 'Remise appliquée', "La remise n'a pas pu être appliquée") });
    add({ id: 'reload', group: 'Actions', icon: 'refresh', title: 'Recharger le dépôt', disabled: !idle, run: () => void repo.reload() });
    // Le titre dit ce que la commande fera : il suit l'état de la carte.
    add({ id: 'toggle-sidebar', group: 'Affichage', icon: 'panel-left', title: layout.sidebarHidden ? 'Afficher la liste des branches' : 'Masquer la liste des branches', keywords: 'panneau barre laterale replier', run: () => toggleSide('sidebar') });
    add({ id: 'toggle-detail', group: 'Affichage', icon: 'panel-right', title: layout.detailHidden ? 'Afficher le panneau de détail' : 'Masquer le panneau de détail', keywords: 'panneau commit replier', run: () => toggleSide('detail') });
    // Le terminal du dépôt (charte, section 9.20). Le raccourci en fait plus que la palette : il
    // donne aussi le clavier au terminal, et le masque si le clavier y est déjà.
    const terminals = repo.terminals;
    add({
      id: 'toggle-terminal',
      group: 'Affichage',
      icon: 'terminal',
      title: terminals.shown ? 'Masquer le terminal' : 'Afficher le terminal',
      keywords: 'console shell ligne de commande panneau',
      run: () => terminals.toggle(repo.path),
      key: () => terminals.key(repo.path, !!document.activeElement?.closest('[data-terminals]')),
    });
    add({ id: 'terminal-repo', group: 'Affichage', icon: 'laptop', title: 'Nouveau terminal dans le dépôt', keywords: 'console shell ligne de commande', run: () => terminals.open(repo.path) });
    add({ id: 'toggle-sides', group: 'Affichage', icon: 'panels', title: layout.sidebarHidden && layout.detailHidden ? 'Réafficher les panneaux latéraux' : 'Masquer les panneaux latéraux', keywords: 'graphe en grand replier', run: toggleSides });
  }
  add({ id: 'open', group: 'Actions', icon: 'folder', title: 'Ouvrir un dépôt…', run: () => void workspace.pick() });
  add({ id: 'clone', group: 'Actions', icon: 'cloud', title: 'Cloner un dépôt…', keywords: 'clone', run: () => (workspace.cloning = true) });
  add({ id: 'init', group: 'Actions', icon: 'plus', title: 'Initialiser un dépôt…', keywords: 'init nouveau', run: () => void workspace.init() });
  add({ id: 'tab-new', group: 'Actions', icon: 'plus', title: 'Nouvel onglet', run: () => workspace.newTab() });
  if (tab) add({ id: 'tab-close', group: 'Actions', icon: 'x', title: "Fermer l'onglet", run: () => workspace.closeTab(tab.id) });
  if (workspace.tabs.length > 1) {
    add({ id: 'tab-next', group: 'Actions', icon: 'chevron', title: 'Onglet suivant', run: () => workspace.cycle(1) });
    add({ id: 'tab-prev', group: 'Actions', icon: 'chevron', title: 'Onglet précédent', run: () => workspace.cycle(-1) });
  }

  if (repo && open && repo.refs) {
    const locals = new Set(repo.refs.branches.map((b) => b.name));
    for (const b of repo.refs.branches) {
      add({
        id: `branch:${b.name}`,
        group: 'Branches',
        icon: b.current ? 'check' : 'branch',
        title: b.current ? `${b.name} (branche courante)` : `Basculer sur ${b.name}`,
        keywords: 'checkout switch',
        hint: [b.ahead ? `↑ ${b.ahead}` : '', b.behind ? `↓ ${b.behind}` : ''].filter(Boolean).join('  ') || undefined,
        disabled: b.current || !idle,
        run: () => void switchBranch(repo, b.name),
      });
    }
    for (const r of repo.refs.remotes) {
      // Une branche distante qui a déjà sa branche locale est couverte par celle-ci.
      if (locals.has(bare(r.name))) continue;
      add({
        id: `remote:${r.name}`,
        group: 'Branches',
        icon: 'cloud',
        title: `Basculer sur ${r.name}`,
        keywords: 'checkout switch distant',
        hint: repo.refs.branches.some((b) => b.name === bare(r.name)) ? 'met à jour la branche locale' : 'crée la branche locale',
        disabled: !idle,
        run: () => void switchToRemote(repo, r.name),
      });
    }
    for (const t of [...repo.refs.tags].reverse()) {
      add({ id: `tag:${t.name}`, group: 'Tags', icon: 'tag', title: `Aller au tag ${t.name}`, run: () => repo.selectSha(t.sha) });
    }
  }

  for (const other of workspace.tabs) {
    if (other === tab || !other.repo.path) continue;
    add({ id: `tab:${other.id}`, group: 'Onglets', icon: 'branch', title: `Aller à l'onglet ${workspace.title(other)}`, hint: other.repo.path, run: () => workspace.activate(other.id) });
  }
  const opened = new Set(workspace.tabs.map((t) => t.repo.path));
  for (const recent of workspace.recents) {
    if (opened.has(recent.path)) continue;
    add({ id: `recent:${recent.path}`, group: 'Dépôts récents', icon: 'folder', title: `Ouvrir ${recent.name}`, hint: recent.path, run: () => void workspace.openPath(recent.path) });
  }
  return out;
}
