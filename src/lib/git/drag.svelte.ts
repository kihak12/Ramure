// Branche en cours de glissement : la déposer sur une autre propose de les fusionner ou de
// rebaser l'une sur l'autre.
import type { Label } from './labels';

export const drag = $state<{ label: Label | null }>({ label: null });

/** Seules les branches se glissent et reçoivent : ni les tags, ni un HEAD détaché. */
export const isBranch = (label: Label) => label.local !== null || label.remotes.length > 0;

/** Nom git d'une étiquette de branche : la branche locale si elle existe, sinon la distante. */
export const refName = (label: Label) => label.local ?? label.remotes[0] ?? label.name;

/** Vrai si `target` peut recevoir la branche en cours de glissement. */
export const accepts = (target: Label) =>
  drag.label !== null && isBranch(target) && refName(drag.label) !== refName(target);
