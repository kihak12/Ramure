// Étiquettes de références affichées dans le graphe et la barre latérale.
import type { CommitRow, RefKind } from './types';

export interface Label {
  /** Nom affiché. */
  name: string;
  kind: RefKind;
  /** Branche locale désignée, s'il y en a une. */
  local: string | null;
  /** Branches distantes au même commit (`origin/main`). */
  remotes: string[];
  tag: string | null;
  title: string;
}

/** Nom d'une branche distante sans son dépôt : `origin/feat/x` → `feat/x`. */
export const bare = (remote: string) => remote.slice(remote.indexOf('/') + 1);

const ORDER: Record<RefKind, number> = { head: 0, detached: 0, local: 1, remote: 2, tag: 3 };

/**
 * Étiquettes d'un commit, la plus importante d'abord. Une branche locale et ses homologues
 * distantes posées sur le même commit sont réunies en une seule étiquette.
 * `shortRemotes` : avec un seul dépôt distant, son nom n'apporte rien et n'est pas affiché.
 */
export function labelsOf(commit: CommitRow, shortRemotes: boolean): Label[] {
  const locals = new Set(commit.refs.filter((r) => r.kind === 'head' || r.kind === 'local').map((r) => r.name));
  const out: Label[] = [];
  for (const ref of commit.refs) {
    if (ref.kind === 'head' || ref.kind === 'local') {
      const twins = commit.refs.filter((r) => r.kind === 'remote' && bare(r.name) === ref.name).map((r) => r.name);
      const what = ref.kind === 'head' ? 'Branche courante' : 'Branche locale';
      out.push({
        name: ref.name,
        kind: ref.kind,
        local: ref.name,
        remotes: twins,
        tag: null,
        title: twins.length ? `${what} ${ref.name}, au même commit que ${twins.join(', ')}` : `${what} ${ref.name}`,
      });
    } else if (ref.kind === 'remote') {
      if (locals.has(bare(ref.name))) continue;
      out.push({
        name: shortRemotes ? bare(ref.name) : ref.name,
        kind: ref.kind,
        local: null,
        remotes: [ref.name],
        tag: null,
        title: `Branche distante ${ref.name}`,
      });
    } else if (ref.kind === 'tag') {
      out.push({ name: ref.name, kind: ref.kind, local: null, remotes: [], tag: ref.name, title: `Tag ${ref.name}` });
    } else {
      out.push({ name: 'HEAD', kind: ref.kind, local: null, remotes: [], tag: null, title: 'HEAD détaché' });
    }
  }
  return out.sort((a, b) => ORDER[a.kind] - ORDER[b.kind]);
}
