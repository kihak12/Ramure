// Actions déclenchées par les menus et la barre d'outils : elles posent les questions
// nécessaires (nom, confirmation), puis délèguent au dépôt.

import { dialogs } from '../ui/dialogs.svelte';
import type { MenuItem } from '../ui/menu.svelte';
import { toasts } from '../ui/toasts.svelte';
import { bare, type Label } from './labels';
import type { Repo, Settings } from './repo.svelte';
import type { CommitRow, FileChange, ResetMode, Stash } from './types';

const short = (sha: string) => sha.slice(0, 7);

// --- Branches -----------------------------------------------------------------------------

export async function createBranch(repo: Repo, start: string, from: string) {
  const answers = await dialogs.ask({
    title: 'Nouvelle branche',
    body: `À partir de ${from}.`,
    fields: [
      { name: 'name', label: 'Nom de la branche', type: 'text', required: true, mono: true, placeholder: 'feat/ma-branche' },
      { name: 'switch', label: 'Basculer dessus', type: 'checkbox', value: true },
    ],
    confirmLabel: 'Créer la branche',
  });
  if (!answers) return;
  const name = String(answers.name);
  await repo.refOp({ op: 'branch-create', name, start, switch: answers.switch === true }, `Branche ${name} créée`);
}

export async function renameBranch(repo: Repo, name: string) {
  const answers = await dialogs.ask({
    title: `Renommer ${name}`,
    fields: [{ name: 'name', label: 'Nouveau nom', type: 'text', required: true, mono: true, value: name }],
    confirmLabel: 'Renommer',
  });
  if (!answers || answers.name === name) return;
  await repo.refOp({ op: 'branch-rename', name, newName: String(answers.name) }, `Branche renommée en ${answers.name}`);
}

export async function deleteBranch(repo: Repo, name: string) {
  const answers = await dialogs.ask({
    title: `Supprimer la branche ${name} ?`,
    body: 'Seule la branche locale est supprimée. Ses commits restent accessibles tant qu’une autre branche ou un tag les contient.',
    fields: [{ name: 'force', label: 'Supprimer même si elle n’est pas fusionnée', type: 'checkbox', value: false }],
    confirmLabel: 'Supprimer',
    danger: true,
  });
  if (!answers) return;
  await repo.refOp({ op: 'branch-delete', name, force: answers.force === true }, `Branche ${name} supprimée`);
}

export async function deleteRemoteBranch(repo: Repo, name: string) {
  const confirmed = await dialogs.confirm({
    title: `Supprimer ${name} sur le dépôt distant ?`,
    warning: 'La branche disparaît pour tout le monde. Cette action ne s’annule pas depuis Ramure.',
    confirmLabel: 'Supprimer sur le dépôt distant',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'remote-branch-delete', name }, `${name} supprimée sur le dépôt distant`);
}

// --- Tags ---------------------------------------------------------------------------------

export async function createTag(repo: Repo, sha: string) {
  const answers = await dialogs.ask({
    title: 'Nouveau tag',
    body: `Sur le commit ${short(sha)}.`,
    fields: [
      { name: 'name', label: 'Nom du tag', type: 'text', required: true, mono: true, placeholder: 'v1.0.0' },
      { name: 'message', label: 'Message', type: 'textarea', hint: 'Avec un message, le tag est annoté. Sans message, il est léger.' },
      { name: 'push', label: 'Pousser le tag vers le dépôt distant', type: 'checkbox', value: false },
    ],
    confirmLabel: 'Créer le tag',
  });
  if (!answers) return;
  const name = String(answers.name);
  const created = await repo.refOp({ op: 'tag-create', name, sha, message: String(answers.message ?? '') }, `Tag ${name} créé`);
  if (created && answers.push === true) await repo.refOp({ op: 'tag-push', name }, `Tag ${name} poussé`);
}

export async function deleteTag(repo: Repo, name: string) {
  const confirmed = await dialogs.confirm({
    title: `Supprimer le tag ${name} ?`,
    body: 'Le tag est supprimé sur cette machine seulement.',
    confirmLabel: 'Supprimer',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'tag-delete', name }, `Tag ${name} supprimé`);
}

export async function deleteRemoteTag(repo: Repo, name: string) {
  const confirmed = await dialogs.confirm({
    title: `Supprimer le tag ${name} sur le dépôt distant ?`,
    warning: 'Le tag disparaît pour tout le monde.',
    confirmLabel: 'Supprimer sur le dépôt distant',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'tag-delete-remote', name }, `Tag ${name} supprimé sur le dépôt distant`);
}

// --- Remises ------------------------------------------------------------------------------

export async function stashChanges(repo: Repo) {
  const answers = await dialogs.ask({
    title: 'Remiser les modifications',
    body: 'Toutes les modifications en cours sont mises de côté, fichiers non suivis compris. Le répertoire de travail redevient propre.',
    fields: [{ name: 'message', label: 'Nom de la remise (facultatif)', type: 'text', placeholder: 'travail en cours sur…' }],
    confirmLabel: 'Remiser',
  });
  if (answers) await repo.refOp({ op: 'stash-push', message: String(answers.message ?? '') }, 'Modifications remisées');
}

export async function dropStash(repo: Repo, stash: Stash) {
  const confirmed = await dialogs.confirm({
    title: `Supprimer la remise ${stash.index} ?`,
    body: stash.message,
    warning: 'Les modifications qu’elle contient seront perdues.',
    confirmLabel: 'Supprimer la remise',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'stash-drop', index: stash.index }, 'Remise supprimée');
}

// --- Historique ---------------------------------------------------------------------------

/** Vrai si le sommet de la branche courante existe déjà sur un dépôt distant. */
const headIsPushed = (repo: Repo) => repo.hasRemote && !!repo.head && !repo.head.localOnly;

const REWRITE_WARNING =
  'Des commits de cette branche sont déjà sur le dépôt distant. Les réécrire obligera à pousser en forçant, et gênera ceux qui les ont récupérés.';

export async function rebaseOnto(repo: Repo, onto: string) {
  const branch = repo.branch ?? 'HEAD';
  const confirmed = await dialogs.confirm({
    title: `Rebaser ${branch} sur ${onto} ?`,
    body: `Les commits de ${branch} absents de ${onto} sont rejoués un par un au-dessus de ${onto}. La position actuelle est sauvegardée dans refs/ramure/backup.`,
    warning: headIsPushed(repo) ? REWRITE_WARNING : undefined,
    confirmLabel: 'Rebaser',
  });
  if (confirmed) await repo.history({ op: 'rebase', onto }, `${branch} rebasée sur ${onto}`, 'Le rebase a échoué');
}

export async function resetTo(repo: Repo, commit: CommitRow) {
  const branch = repo.branch;
  if (!branch) return;
  const answers = await dialogs.ask({
    title: `Réinitialiser ${branch} sur ${short(commit.sha)}`,
    body: `« ${commit.subject} » devient le dernier commit de ${branch}. La position actuelle est sauvegardée dans refs/ramure/backup.`,
    warning: headIsPushed(repo) ? REWRITE_WARNING : undefined,
    fields: [
      {
        name: 'mode',
        label: 'Que deviennent les changements des commits retirés ?',
        type: 'select',
        value: 'mixed',
        options: [
          { value: 'soft', label: 'Ils restent indexés (soft)' },
          { value: 'mixed', label: 'Ils restent dans les fichiers (mixed)' },
          { value: 'hard', label: 'Ils sont supprimés (hard)' },
        ],
      },
    ],
    confirmLabel: 'Réinitialiser',
    danger: true,
  });
  if (!answers) return;
  const mode = answers.mode as ResetMode;
  if (mode === 'hard') {
    const sure = await dialogs.confirm({
      title: 'Confirmer le reset --hard',
      warning: 'Les modifications en cours et celles des commits retirés seront supprimées des fichiers.',
      confirmLabel: 'Tout supprimer et réinitialiser',
      danger: true,
    });
    if (!sure) return;
  }
  await repo.history({ op: 'reset', sha: commit.sha, mode }, `${branch} réinitialisée sur ${short(commit.sha)}`, 'Le reset a échoué');
}

export async function forcePush(repo: Repo) {
  const confirmed = await dialogs.confirm({
    title: `Pousser ${repo.branch ?? ''} en forçant ?`,
    body: 'La branche distante est remplacée par la tienne (git push --force-with-lease). Le push est refusé si quelqu’un a poussé entre-temps un travail que tu n’as pas récupéré.',
    warning: 'Les commits distants absents de ta branche seront perdus.',
    confirmLabel: 'Pousser en forçant',
    danger: true,
  });
  if (confirmed) await repo.sync('push-force');
}

export async function discardFiles(repo: Repo, files: FileChange[]) {
  if (!files.length) return;
  const what = files.length === 1 ? `« ${files[0].path} »` : `${files.length} fichiers`;
  const onlyNew = files.every((f) => f.kind === 'untracked');
  const confirmed = await dialogs.confirm({
    title: onlyNew ? `Supprimer ${what} ?` : `Abandonner les modifications de ${what} ?`,
    warning: onlyNew
      ? 'Ces fichiers sont nouveaux : git ne pourra pas les restaurer.'
      : 'Les modifications non indexées seront perdues définitivement.',
    confirmLabel: onlyNew ? 'Supprimer' : 'Abandonner',
    danger: true,
  });
  if (confirmed) await repo.discard(files);
}

export async function editSettings(repo: Repo) {
  const current = repo.settings;
  const answers = await dialogs.ask({
    title: 'Réglages',
    fields: [
      {
        name: 'autoFetch',
        label: 'Fetch automatique, en minutes',
        type: 'number',
        value: current.autoFetch,
        min: 0,
        max: 60,
        hint: 'De 1 à 60 minutes ; 0 le désactive.',
      },
      {
        name: 'pullMode',
        label: 'Tirer (pull)',
        type: 'select',
        value: current.pullMode,
        options: [
          { value: 'ff-only', label: 'Avance rapide seulement' },
          { value: 'rebase', label: 'Rebaser mes commits' },
          { value: 'merge', label: 'Fusionner la branche distante' },
        ],
        hint: 'En avance rapide, le pull est refusé si ta branche et la branche distante ont divergé.',
      },
      {
        name: 'limit',
        label: 'Commits chargés dans le graphe',
        type: 'select',
        value: current.limit,
        options: [
          { value: 1000, label: '1 000' },
          { value: 10000, label: '10 000' },
          { value: 50000, label: '50 000' },
          { value: 0, label: 'Tout l’historique' },
        ],
      },
    ],
    confirmLabel: 'Enregistrer',
  });
  if (!answers) return;
  const minutes = Math.round(Number(answers.autoFetch) || 0);
  const next: Settings = {
    autoFetch: Math.max(0, Math.min(60, minutes)),
    pullMode: answers.pullMode as Settings['pullMode'],
    limit: Number(answers.limit),
  };
  repo.saveSettings(next);
}

async function copy(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    toasts.ok(`${what} copié`);
  } catch {
    toasts.err('Copie impossible', 'Le presse-papiers a refusé l’écriture.');
  }
}

// --- Menus contextuels --------------------------------------------------------------------

export function commitMenu(repo: Repo, commit: CommitRow): MenuItem[] {
  if (commit.wip) {
    return [
      { label: 'Remiser les modifications…', icon: 'stash', run: () => void stashChanges(repo) },
      { label: 'Tout indexer', icon: 'plus', disabled: !repo.status?.unstaged.length, run: () => void repo.stage(repo.status?.unstaged ?? []) },
      { separator: true },
      {
        label: 'Abandonner toutes les modifications non indexées…',
        icon: 'undo',
        danger: true,
        disabled: !repo.status?.unstaged.length,
        run: () => void discardFiles(repo, repo.status?.unstaged ?? []),
      },
    ];
  }
  const branch = repo.branch;
  const isHead = commit.sha === repo.head?.sha;
  const merge = commit.parents.length > 1;
  const name = `le commit ${short(commit.sha)}`;
  return [
    { label: 'Créer une branche ici…', icon: 'branch', run: () => void createBranch(repo, commit.sha, name) },
    { label: 'Créer un tag ici…', icon: 'tag', run: () => void createTag(repo, commit.sha) },
    { separator: true },
    {
      label: 'Cherry-pick de ce commit',
      icon: 'pick',
      disabled: isHead,
      hint: `Rejoue ce commit sur ${branch ?? 'HEAD'}`,
      run: () => void repo.history({ op: 'cherry-pick', sha: commit.sha, mainline: merge }, `Commit ${short(commit.sha)} repris`, 'Le cherry-pick a échoué'),
    },
    {
      label: 'Annuler ce commit (revert)',
      icon: 'undo',
      hint: 'Crée un commit qui défait celui-ci',
      run: () => void repo.history({ op: 'revert', sha: commit.sha, mainline: merge }, `Commit ${short(commit.sha)} annulé`, 'Le revert a échoué'),
    },
    { separator: true },
    {
      label: branch ? `Réinitialiser ${branch} ici…` : 'Réinitialiser ici…',
      icon: 'reset',
      disabled: !branch || isHead,
      run: () => void resetTo(repo, commit),
    },
    {
      label: 'Rebase interactif à partir d’ici…',
      icon: 'list',
      hint: 'Réordonner, fusionner, reformuler ou supprimer les commits de celui-ci jusqu’à HEAD',
      run: () => void repo.planRebase(commit.sha),
    },
    { label: 'Basculer sur ce commit (HEAD détaché)', icon: 'commit', disabled: isHead, run: () => void repo.refOp({ op: 'detach', sha: commit.sha }, `HEAD détaché sur ${short(commit.sha)}`) },
    { separator: true },
    { label: 'Copier le SHA', icon: 'copy', run: () => void copy(commit.sha, 'SHA') },
  ];
}

/** Menu d'une étiquette de branche ou de tag. `sha` : le commit qu'elle désigne. */
export function labelMenu(repo: Repo, label: Label, sha: string): MenuItem[] {
  const current = repo.branch;
  const here = current ?? 'HEAD';
  const items: MenuItem[] = [];

  if (label.tag) {
    return [
      { label: `Pousser le tag ${label.tag}`, icon: 'push', disabled: !repo.refs?.remoteNames.length, run: () => void repo.refOp({ op: 'tag-push', name: label.tag! }, `Tag ${label.tag} poussé`) },
      { label: 'Créer une branche ici…', icon: 'branch', run: () => void createBranch(repo, sha, `le tag ${label.tag}`) },
      { separator: true },
      { label: 'Supprimer le tag…', icon: 'trash', danger: true, run: () => void deleteTag(repo, label.tag!) },
      { label: 'Supprimer le tag sur le dépôt distant…', icon: 'trash', danger: true, disabled: !repo.refs?.remoteNames.length, run: () => void deleteRemoteTag(repo, label.tag!) },
    ];
  }
  if (label.kind === 'detached') return commitMenuForSha(repo, sha);

  const local = label.local;
  const remote = label.remotes[0] ?? null;
  const target = local ?? remote!;
  const isCurrent = local !== null && local === current;

  if (isCurrent) {
    items.push(
      { label: 'Tirer', icon: 'pull', disabled: !repo.status?.upstream, run: () => void repo.sync('pull') },
      { label: repo.status?.upstream ? 'Pousser' : 'Publier la branche', icon: 'push', run: () => void repo.sync('push') },
      { label: 'Pousser en forçant…', icon: 'push', danger: true, disabled: !repo.status?.upstream, run: () => void forcePush(repo) },
    );
  } else {
    items.push(
      {
        label: `Basculer sur ${local ?? bare(target)}`,
        icon: 'check',
        hint: local ? undefined : 'Crée la branche locale qui suit cette branche distante',
        run: () =>
          void (local
            ? repo.refOp({ op: 'switch', name: local }, `Basculé sur ${local}`, 'Impossible de basculer')
            : repo.refOp({ op: 'switch-remote', name: target }, `Basculé sur ${bare(target)}`, 'Impossible de basculer')),
      },
      { separator: true },
      {
        label: `Fusionner ${target} dans ${here}`,
        icon: 'merge',
        run: () => void repo.history({ op: 'merge', target }, `${target} fusionnée dans ${here}`, 'La fusion a échoué'),
      },
      { label: `Rebaser ${here} sur ${target}…`, icon: 'branch', disabled: !current, run: () => void rebaseOnto(repo, target) },
    );
  }
  items.push({ separator: true }, { label: 'Créer une branche ici…', icon: 'branch', run: () => void createBranch(repo, sha, target) });
  if (local) {
    items.push({ label: 'Renommer…', icon: 'pencil', run: () => void renameBranch(repo, local) });
    items.push({ label: 'Supprimer la branche…', icon: 'trash', danger: true, disabled: isCurrent, run: () => void deleteBranch(repo, local) });
  }
  for (const name of label.remotes) {
    items.push({ label: `Supprimer ${name} sur le dépôt distant…`, icon: 'trash', danger: true, run: () => void deleteRemoteBranch(repo, name) });
  }
  return items;
}

function commitMenuForSha(repo: Repo, sha: string): MenuItem[] {
  const commit = repo.commits.find((c) => c.sha === sha);
  return commit ? commitMenu(repo, commit) : [];
}

export function stashMenu(repo: Repo, stash: Stash): MenuItem[] {
  return [
    { label: 'Voir le contenu', icon: 'eye', run: () => repo.openStashDiff(stash.index, stash.message) },
    { separator: true },
    { label: 'Appliquer et supprimer (pop)', icon: 'pop', run: () => void repo.refOp({ op: 'stash-pop', index: stash.index }, 'Remise appliquée', 'La remise n’a pas pu être appliquée') },
    { label: 'Appliquer en la gardant', icon: 'stash', run: () => void repo.refOp({ op: 'stash-apply', index: stash.index }, 'Remise appliquée', 'La remise n’a pas pu être appliquée') },
    { separator: true },
    { label: 'Supprimer la remise…', icon: 'trash', danger: true, run: () => void dropStash(repo, stash) },
  ];
}
