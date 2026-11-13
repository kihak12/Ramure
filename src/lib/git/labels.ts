// Labels of the refs shown in the graph and in the sidebar.
import type { CommitRow, RefKind } from './types';

export interface Label {
  /** Displayed name. */
  name: string;
  kind: RefKind;
  /** The local branch it stands for, if there is one. */
  local: string | null;
  /** Remote branches on the same commit (`origin/main`). */
  remotes: string[];
  tag: string | null;
  title: string;
}

/** Name of a remote branch without its remote: `origin/feat/x` → `feat/x`. */
export const bare = (remote: string) => remote.slice(remote.indexOf('/') + 1);

const ORDER: Record<RefKind, number> = { head: 0, detached: 0, local: 1, remote: 2, tag: 3 };

/**
 * Labels of a commit, the most important first. A local branch and its remote counterparts on
 * the same commit are gathered into one label.
 * `shortRemotes`: with a single remote, its name adds nothing and is not displayed.
 */
export function labelsOf(commit: CommitRow, shortRemotes: boolean): Label[] {
  const locals = new Set(commit.refs.filter((r) => r.kind === 'head' || r.kind === 'local').map((r) => r.name));
  const out: Label[] = [];
  for (const ref of commit.refs) {
    if (ref.kind === 'head' || ref.kind === 'local') {
      const twins = commit.refs.filter((r) => r.kind === 'remote' && bare(r.name) === ref.name).map((r) => r.name);
      const what = ref.kind === 'head' ? 'Current branch' : 'Local branch';
      out.push({
        name: ref.name,
        kind: ref.kind,
        local: ref.name,
        remotes: twins,
        tag: null,
        title: twins.length ? `${what} ${ref.name}, on the same commit as ${twins.join(', ')}` : `${what} ${ref.name}`,
      });
    } else if (ref.kind === 'remote') {
      if (locals.has(bare(ref.name))) continue;
      out.push({
        name: shortRemotes ? bare(ref.name) : ref.name,
        kind: ref.kind,
        local: null,
        remotes: [ref.name],
        tag: null,
        title: `Remote branch ${ref.name}`,
      });
    } else if (ref.kind === 'tag') {
      out.push({ name: ref.name, kind: ref.kind, local: null, remotes: [], tag: ref.name, title: `Tag ${ref.name}` });
    } else {
      out.push({ name: 'HEAD', kind: ref.kind, local: null, remotes: [], tag: null, title: 'Detached HEAD' });
    }
  }
  return out.sort((a, b) => ORDER[a.kind] - ORDER[b.kind]);
}
