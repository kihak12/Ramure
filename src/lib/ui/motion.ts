/**
 * Motion durations for the transitions of Svelte, read from the tokens of `app.css`.
 * One source only: when the system asks for less animation, the tokens drop to 1 ms
 * and these transitions follow.
 */
function ms(token: string): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  const value = parseFloat(raw);
  if (Number.isNaN(value)) return 0;
  return raw.endsWith('ms') ? value : value * 1000;
}

export const motion = {
  /** Hover, press, disappearance. */
  get fast(): number {
    return ms('--t-fast');
  },
  /** Appearance, folding of a section, reordering. */
  get med(): number {
    return ms('--t-med');
  },
  /** Long move. */
  get slow(): number {
    return ms('--t-slow');
  },
};
