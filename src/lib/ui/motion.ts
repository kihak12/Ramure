/**
 * Durées du mouvement pour les transitions de Svelte, lues dans les jetons de `app.css`.
 * Une seule source : quand le système demande moins d'animations, les jetons tombent à 1 ms
 * et ces transitions suivent.
 */
function ms(token: string): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  const value = parseFloat(raw);
  if (Number.isNaN(value)) return 0;
  return raw.endsWith('ms') ? value : value * 1000;
}

export const motion = {
  /** Survol, pression, disparition. */
  get fast(): number {
    return ms('--t-fast');
  },
  /** Apparition, repli d'une section, réorganisation. */
  get med(): number {
    return ms('--t-med');
  },
  /** Déplacement long. */
  get slow(): number {
    return ms('--t-slow');
  },
};
