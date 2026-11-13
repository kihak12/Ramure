// Actions triggered by the menus and the toolbar: they ask the questions they need (a name, a
// confirmation), then hand over to the repository.

import { dialogs } from '../ui/dialogs.svelte';
import type { MenuItem } from '../ui/menu.svelte';
import { toasts } from '../ui/toasts.svelte';
import { refName } from './drag.svelte';
import { bare, type Label } from './labels';
import { OPERATION, type Repo } from './repo.svelte';
import type { CommitRow, FileChange, Relation, ResetMode, Stash } from './types';

const short = (sha: string) => sha.slice(0, 7);
const commits = (count: number) => `${count} commit${count === 1 ? '' : 's'}`;

// --- Branches -----------------------------------------------------------------------------

/**
 * Before a switch: if working changes would be overwritten, git would refuse. The user is then
 * offered to stash them. Returns `false` when the user gives up, or when the stash fails.
 * `label`: the name to show, when `name` is a remote branch.
 */
async function stashFirst(repo: Repo, name: string, remote: boolean, label = name): Promise<boolean> {
  let blockers: string[];
  try {
    blockers = await repo.git<string[]>('git_switch_blockers', { name, remote });
  } catch {
    // The check could not be made: the switch itself will say what is wrong.
    return true;
  }
  if (!blockers.length) return true;
  const what = blockers.length === 1 ? blockers[0] : `${blockers.length} files`;
  const sure = await dialogs.confirm({
    title: `Switch to ${label}?`,
    body:
      `Switching would overwrite your changes to ${what}, and git would refuse it.\n\n` +
      'Stash all your working changes first: you can pop them afterward, on the branch of your choice.',
    confirmLabel: 'Stash and switch',
  });
  if (!sure) return false;
  return repo.refOp({ op: 'stash-push', message: `Before switching to ${label}` }, 'Changes stashed', 'Could not stash changes');
}

/** Switches to a local branch, offering to stash the changes that would prevent it. */
export async function switchBranch(repo: Repo, name: string, done = `Switched to ${name}`): Promise<boolean> {
  if (repo.locked || name === repo.branch) return false;
  if (!(await stashFirst(repo, name, false))) return false;
  return repo.refOp({ op: 'switch', name }, done, `Could not switch to ${name}`);
}

/**
 * Switches to the local branch that tracks the remote branch `remote`, and updates it.
 * - No local branch yet: it is created.
 * - Local branch behind: it is fast-forwarded to the remote branch.
 * - Local branch ahead: nothing to fetch, it only switches.
 * - Diverged branches: asks before rebasing, since a rebase rewrites the local commits.
 */
export async function switchToRemote(repo: Repo, remote: string) {
  if (repo.locked) return;
  const local = bare(remote);
  const failed = `Could not switch to ${local}`;
  let relation: Relation;
  try {
    relation = await repo.git<Relation>('git_remote_relation', { remote });
  } catch (e) {
    toasts.err(failed, e instanceof Error ? e.message : String(e));
    return;
  }

  const { ahead, behind } = relation;
  if (relation.local === null || ahead === 0 || behind === 0) {
    let done = `Switched to ${local}`;
    if (relation.local !== null && behind > 0) done += `, updated with ${remote} (${commits(behind)})`;
    else if (ahead > 0) done += `, ${commits(ahead)} ahead of ${remote}`;
    // The destination is the remote branch when the local one does not exist or will be moved up to it.
    const viaRemote = relation.local === null || behind > 0;
    if (await stashFirst(repo, viaRemote ? remote : local, viaRemote, local)) {
      await repo.refOp({ op: 'switch-remote', name: remote }, done, failed);
    }
    return;
  }

  const stay = () => void switchBranch(repo, local, `Switched to ${local}, without updating it`);
  const rebase = await dialogs.confirm({
    title: `Update ${local}?`,
    body:
      `${local} and ${remote} have diverged: ${commits(ahead)} of ${local} ${ahead === 1 ? 'is' : 'are'} not on ${remote}, ` +
      `and ${commits(behind)} of ${remote} ${behind === 1 ? 'is' : 'are'} missing from ${local}.\n\n` +
      `Rebasing replays your commits on top of ${remote}. A backup of the branch is kept.`,
    confirmLabel: 'Switch and rebase',
    extra: { label: 'Switch without updating', run: stay },
  });
  if (!rebase) return;
  if (repo.branch !== local && !(await switchBranch(repo, local))) return;
  await repo.history({ op: 'rebase', onto: remote }, `${local} rebased onto ${remote}`, 'Rebase failed');
}

export async function createBranch(repo: Repo, start: string, from: string) {
  const answers = await dialogs.ask({
    title: 'New branch',
    body: `From ${from}.`,
    fields: [
      { name: 'name', label: 'Branch name', type: 'text', required: true, mono: true, placeholder: 'feat/my-branch' },
      { name: 'switch', label: 'Switch to it', type: 'checkbox', value: true },
    ],
    confirmLabel: 'Create branch',
  });
  if (!answers) return;
  const name = String(answers.name);
  await repo.refOp({ op: 'branch-create', name, start, switch: answers.switch === true }, `Branch ${name} created`);
}

export async function renameBranch(repo: Repo, name: string) {
  const answers = await dialogs.ask({
    title: `Rename ${name}`,
    fields: [{ name: 'name', label: 'New name', type: 'text', required: true, mono: true, value: name }],
    confirmLabel: 'Rename',
  });
  if (!answers || answers.name === name) return;
  await repo.refOp({ op: 'branch-rename', name, newName: String(answers.name) }, `Branch renamed to ${answers.name}`);
}

export async function deleteBranch(repo: Repo, name: string) {
  const answers = await dialogs.ask({
    title: `Delete branch ${name}?`,
    body: 'Only the local branch is deleted. Its commits stay reachable as long as another branch or a tag contains them.',
    fields: [{ name: 'force', label: 'Delete even if it is not merged', type: 'checkbox', value: false }],
    confirmLabel: 'Delete',
    danger: true,
  });
  if (!answers) return;
  await repo.refOp({ op: 'branch-delete', name, force: answers.force === true }, `Branch ${name} deleted`);
}

export async function deleteRemoteBranch(repo: Repo, name: string) {
  const confirmed = await dialogs.confirm({
    title: `Delete ${name} from the remote?`,
    warning: 'The branch disappears for everyone. This cannot be undone from Ramure.',
    confirmLabel: 'Delete from the remote',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'remote-branch-delete', name }, `${name} deleted from the remote`);
}

// --- Tags ---------------------------------------------------------------------------------

export async function createTag(repo: Repo, sha: string) {
  const answers = await dialogs.ask({
    title: 'New tag',
    body: `On commit ${short(sha)}.`,
    fields: [
      { name: 'name', label: 'Tag name', type: 'text', required: true, mono: true, placeholder: 'v1.0.0' },
      { name: 'message', label: 'Message', type: 'textarea', hint: 'With a message, the tag is annotated. Without one, it is lightweight.' },
      { name: 'push', label: 'Push tag to the remote', type: 'checkbox', value: false },
    ],
    confirmLabel: 'Create tag',
  });
  if (!answers) return;
  const name = String(answers.name);
  const created = await repo.refOp({ op: 'tag-create', name, sha, message: String(answers.message ?? '') }, `Tag ${name} created`);
  if (created && answers.push === true) await repo.refOp({ op: 'tag-push', name }, `Tag ${name} pushed`);
}

export async function deleteTag(repo: Repo, name: string) {
  const confirmed = await dialogs.confirm({
    title: `Delete tag ${name}?`,
    body: 'The tag is deleted on this machine only.',
    confirmLabel: 'Delete',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'tag-delete', name }, `Tag ${name} deleted`);
}

export async function deleteRemoteTag(repo: Repo, name: string) {
  const confirmed = await dialogs.confirm({
    title: `Delete tag ${name} from the remote?`,
    warning: 'The tag disappears for everyone.',
    confirmLabel: 'Delete from the remote',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'tag-delete-remote', name }, `Tag ${name} deleted from the remote`);
}

// --- Stashes ------------------------------------------------------------------------------

export async function stashChanges(repo: Repo) {
  const answers = await dialogs.ask({
    title: 'Stash changes',
    body: 'All working changes are set aside, untracked files included. The working directory is clean again.',
    fields: [{ name: 'message', label: 'Stash name (optional)', type: 'text', placeholder: 'work in progress on…' }],
    confirmLabel: 'Stash',
  });
  if (answers) await repo.refOp({ op: 'stash-push', message: String(answers.message ?? '') }, 'Changes stashed');
}

export async function dropStash(repo: Repo, stash: Stash) {
  const confirmed = await dialogs.confirm({
    title: `Drop stash ${stash.index}?`,
    body: stash.message,
    warning: 'The changes it holds will be lost.',
    confirmLabel: 'Drop stash',
    danger: true,
  });
  if (confirmed) await repo.refOp({ op: 'stash-drop', index: stash.index }, 'Stash dropped');
}

// --- History ------------------------------------------------------------------------------

/** True if the tip of the current branch is already on a remote. */
const headIsPushed = (repo: Repo) => repo.hasRemote && !!repo.head && !repo.head.localOnly;

const REWRITE_WARNING =
  'Some commits of this branch are already on the remote. Rewriting them will require a force push, and will get in the way of anyone who has fetched them.';

export async function rebaseOnto(repo: Repo, onto: string) {
  const branch = repo.branch ?? 'HEAD';
  const confirmed = await dialogs.confirm({
    title: `Rebase ${branch} onto ${onto}?`,
    body: `The commits of ${branch} that are not in ${onto} are replayed one by one on top of ${onto}. The current position is saved in refs/ramure/backup.`,
    warning: headIsPushed(repo) ? REWRITE_WARNING : undefined,
    confirmLabel: 'Rebase',
  });
  if (confirmed) await repo.history({ op: 'rebase', onto }, `${branch} rebased onto ${onto}`, 'Rebase failed');
}

export async function resetTo(repo: Repo, commit: CommitRow) {
  const branch = repo.branch;
  if (!branch) return;
  const answers = await dialogs.ask({
    title: `Reset ${branch} to ${short(commit.sha)}`,
    body: `“${commit.subject}” becomes the last commit of ${branch}. The current position is saved in refs/ramure/backup.`,
    warning: headIsPushed(repo) ? REWRITE_WARNING : undefined,
    fields: [
      {
        name: 'mode',
        label: 'What happens to the changes of the removed commits?',
        type: 'select',
        value: 'mixed',
        options: [
          { value: 'soft', label: 'They stay staged (soft)' },
          { value: 'mixed', label: 'They stay in the files (mixed)' },
          { value: 'hard', label: 'They are deleted (hard)' },
        ],
      },
    ],
    confirmLabel: 'Reset',
    danger: true,
  });
  if (!answers) return;
  const mode = answers.mode as ResetMode;
  if (mode === 'hard') {
    const sure = await dialogs.confirm({
      title: `Reset ${branch} with --hard?`,
      warning: 'Your working changes and those of the removed commits will be deleted from the files.',
      confirmLabel: 'Delete all and reset',
      danger: true,
    });
    if (!sure) return;
  }
  await repo.history({ op: 'reset', sha: commit.sha, mode }, `${branch} reset to ${short(commit.sha)}`, 'Reset failed');
}

export async function forcePush(repo: Repo) {
  const confirmed = await dialogs.confirm({
    title: `Force push ${repo.branch ?? ''}?`,
    body: 'The remote branch is replaced by yours (git push --force-with-lease). The push is refused if someone has pushed work in the meantime that you have not fetched.',
    warning: 'The remote commits that are not on your branch will be lost.',
    confirmLabel: 'Force push',
    danger: true,
  });
  if (confirmed) await repo.sync('push-force');
}

/** Aborts the operation in progress, after a confirmation that says what is lost (guidelines, 9.1). */
export async function abortOperation(repo: Repo) {
  const operation = repo.status?.operation;
  if (!operation) return;
  // The operation inside a sentence: “Abort rebase”.
  const what = OPERATION[operation].toLowerCase();
  // `git <operation> --abort` gives the branches back their original commits: only the work done
  // during the operation, the resolved conflicts, is lost, and git keeps it nowhere.
  const confirmed = await dialogs.confirm({
    title: `Abort ${what}?`,
    body: `The repository goes back to its state before the ${what} (git ${operation} --abort).`,
    warning: 'The conflicts already resolved will be lost for good.',
    confirmLabel: `Abort ${what}`,
    danger: true,
  });
  if (confirmed) await repo.operate('abort');
}

export async function discardFiles(repo: Repo, files: FileChange[]) {
  if (!files.length) return;
  const one = files.length === 1;
  const what = one ? `“${files[0].path}”` : `${files.length} files`;
  const onlyNew = files.every((f) => f.kind === 'untracked');
  const confirmed = await dialogs.confirm({
    title: onlyNew ? `Delete ${what}?` : `Discard changes to ${what}?`,
    warning: onlyNew
      ? one
        ? 'This file is new: git cannot restore it.'
        : 'These files are new: git cannot restore them.'
      : 'The unstaged changes will be lost for good.',
    confirmLabel: onlyNew ? 'Delete' : 'Discard',
    danger: true,
  });
  if (confirmed) await repo.discard(files);
}

async function copy(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    toasts.ok(`${what} copied`);
  } catch {
    toasts.err('Could not copy', 'The clipboard refused the text.');
  }
}

// --- Context menus ------------------------------------------------------------------------

export function commitMenu(repo: Repo, commit: CommitRow): MenuItem[] {
  if (commit.wip) {
    return [
      { label: 'Stash changes…', icon: 'stash', run: () => void stashChanges(repo) },
      { label: 'Stage all', icon: 'plus', disabled: !repo.status?.unstaged.length, run: () => void repo.stage(repo.status?.unstaged ?? []) },
      { separator: true },
      {
        label: 'Discard all unstaged changes…',
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
  const name = `commit ${short(commit.sha)}`;
  return [
    { label: 'Create branch here…', icon: 'branch', run: () => void createBranch(repo, commit.sha, name) },
    { label: 'Create tag here…', icon: 'tag', run: () => void createTag(repo, commit.sha) },
    { separator: true },
    {
      label: 'Cherry-pick this commit',
      icon: 'pick',
      disabled: isHead,
      hint: `Replay this commit on ${branch ?? 'HEAD'}`,
      run: () => void repo.history({ op: 'cherry-pick', sha: commit.sha, mainline: merge }, `Commit ${short(commit.sha)} cherry-picked`, 'Cherry-pick failed'),
    },
    {
      label: 'Revert this commit',
      icon: 'undo',
      hint: 'Create a commit that undoes this one',
      run: () => void repo.history({ op: 'revert', sha: commit.sha, mainline: merge }, `Commit ${short(commit.sha)} reverted`, 'Revert failed'),
    },
    { separator: true },
    {
      label: branch ? `Reset ${branch} to here…` : 'Reset to here…',
      icon: 'reset',
      disabled: !branch || isHead,
      run: () => void resetTo(repo, commit),
    },
    {
      label: 'Rebase interactively from here…',
      icon: 'list',
      hint: 'Reorder, squash, reword or drop the commits from this one up to HEAD',
      run: () => void repo.planRebase(commit.sha),
    },
    { label: 'Switch to this commit (detached HEAD)', icon: 'commit', disabled: isHead, run: () => void repo.refOp({ op: 'detach', sha: commit.sha }, `HEAD detached at ${short(commit.sha)}`) },
    { separator: true },
    { label: 'Copy SHA', icon: 'copy', run: () => void copy(commit.sha, 'SHA') },
  ];
}

/** Menu of a branch or tag label. `sha`: the commit it points to. */
export function labelMenu(repo: Repo, label: Label, sha: string): MenuItem[] {
  const current = repo.branch;
  const here = current ?? 'HEAD';
  const items: MenuItem[] = [];

  if (label.tag) {
    return [
      { label: `Push tag ${label.tag}`, icon: 'push', disabled: !repo.refs?.remoteNames.length, run: () => void repo.refOp({ op: 'tag-push', name: label.tag! }, `Tag ${label.tag} pushed`) },
      { label: 'Create branch here…', icon: 'branch', run: () => void createBranch(repo, sha, `tag ${label.tag}`) },
      { separator: true },
      { label: 'Delete tag…', icon: 'trash', danger: true, run: () => void deleteTag(repo, label.tag!) },
      { label: 'Delete tag from the remote…', icon: 'trash', danger: true, disabled: !repo.refs?.remoteNames.length, run: () => void deleteRemoteTag(repo, label.tag!) },
    ];
  }
  if (label.kind === 'detached') return commitMenuForSha(repo, sha);

  const local = label.local;
  const remote = label.remotes[0] ?? null;
  const target = local ?? remote!;
  const isCurrent = local !== null && local === current;

  if (isCurrent) {
    items.push(
      { label: 'Pull', icon: 'pull', disabled: !repo.status?.upstream, run: () => void repo.sync('pull') },
      { label: repo.status?.upstream ? 'Push' : 'Publish branch', icon: 'push', run: () => void repo.sync('push') },
      { label: 'Force push…', icon: 'push', danger: true, disabled: !repo.status?.upstream, run: () => void forcePush(repo) },
    );
  } else {
    items.push(
      {
        label: `Switch to ${local ?? bare(target)}`,
        icon: 'check',
        hint: local
          ? undefined
          : repo.refs?.branches.some((b) => b.name === bare(target))
            ? `Switch to the local branch and update it with ${target}`
            : 'Create the local branch that tracks this remote branch',
        run: () =>
          void (local ? switchBranch(repo, local) : switchToRemote(repo, target)),
      },
      { separator: true },
      {
        label: `Merge ${target} into ${here}`,
        icon: 'merge',
        run: () => void repo.history({ op: 'merge', target }, `${target} merged into ${here}`, 'Merge failed'),
      },
      { label: `Rebase ${here} onto ${target}…`, icon: 'branch', disabled: !current, run: () => void rebaseOnto(repo, target) },
    );
  }
  items.push({ separator: true }, { label: 'Create branch here…', icon: 'branch', run: () => void createBranch(repo, sha, target) });
  if (local) {
    items.push({ label: 'Rename branch…', icon: 'pencil', run: () => void renameBranch(repo, local) });
    items.push({ label: 'Delete branch…', icon: 'trash', danger: true, disabled: isCurrent, run: () => void deleteBranch(repo, local) });
  }
  for (const name of label.remotes) {
    items.push({ label: `Delete ${name} from the remote…`, icon: 'trash', danger: true, run: () => void deleteRemoteBranch(repo, name) });
  }
  return items;
}

/**
 * Menu offered when the branch `source` is dropped onto the branch `target`.
 * git only merges into the current branch, and only rebases the current branch: when a switch
 * has to come first, the label says so.
 */
export function dropMenu(repo: Repo, source: Label, target: Label): MenuItem[] {
  const [from, onto] = [refName(source), refName(target)];
  const current = repo.branch;
  const switchThen = async (name: string, next: () => Promise<unknown>) => {
    if (await switchBranch(repo, name)) await next();
  };
  const merge = () => repo.history({ op: 'merge', target: from }, `${from} merged into ${onto}`, 'Merge failed');

  const items: MenuItem[] = [];
  if (target.local === null) {
    items.push({ label: `Merge ${from} into ${onto}`, icon: 'merge', disabled: true, hint: 'You can only merge into a local branch', run: () => {} });
  } else if (target.local === current) {
    items.push({ label: `Merge ${from} into ${onto}`, icon: 'merge', run: () => void merge() });
  } else {
    items.push({ label: `Switch to ${onto}, then merge ${from} into it`, icon: 'merge', run: () => void switchThen(onto, merge) });
  }
  if (source.local === null) {
    items.push({ label: `Rebase ${from} onto ${onto}…`, icon: 'branch', disabled: true, hint: 'You can only rebase a local branch', run: () => {} });
  } else if (source.local === current) {
    items.push({ label: `Rebase ${from} onto ${onto}…`, icon: 'branch', run: () => void rebaseOnto(repo, onto) });
  } else {
    items.push({ label: `Switch to ${from}, then rebase it onto ${onto}…`, icon: 'branch', run: () => void switchThen(from, () => rebaseOnto(repo, onto)) });
  }
  return items;
}

function commitMenuForSha(repo: Repo, sha: string): MenuItem[] {
  const commit = repo.commits.find((c) => c.sha === sha);
  return commit ? commitMenu(repo, commit) : [];
}

export function stashMenu(repo: Repo, stash: Stash): MenuItem[] {
  return [
    { label: 'Show contents', icon: 'eye', run: () => repo.openStashDiff(stash.index, stash.message) },
    { separator: true },
    { label: 'Pop stash', icon: 'pop', run: () => void repo.refOp({ op: 'stash-pop', index: stash.index }, 'Stash popped', 'Could not pop stash') },
    { label: 'Apply stash', icon: 'stash', run: () => void repo.refOp({ op: 'stash-apply', index: stash.index }, 'Stash applied', 'Could not apply stash') },
    { separator: true },
    { label: 'Drop stash…', icon: 'trash', danger: true, run: () => void dropStash(repo, stash) },
  ];
}
