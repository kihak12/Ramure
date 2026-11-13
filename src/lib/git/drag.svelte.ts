// Branch being dragged: dropping it on another offers to merge them or to rebase one onto the
// other.
import type { Label } from './labels';

export const drag = $state<{ label: Label | null }>({ label: null });

/** Only branches are dragged and receive a drop: neither tags nor a detached HEAD. */
export const isBranch = (label: Label) => label.local !== null || label.remotes.length > 0;

/** Git name of a branch label: the local branch if there is one, otherwise the remote one. */
export const refName = (label: Label) => label.local ?? label.remotes[0] ?? label.name;

/** True if `target` can receive the branch being dragged. */
export const accepts = (target: Label) =>
  drag.label !== null && isBranch(target) && refName(drag.label) !== refName(target);
