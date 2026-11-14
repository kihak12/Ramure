// Checks that every color theme of src/app.css meets the contrasts and the color distances of the
// guidelines (docs/design-guidelines.md, section 2.4), in light and in dark, branch labels and
// capsules included. Also checks that the dark of each theme, written twice (for whoever follows
// the system, for whoever chose "Dark"), gives the same values.
// Run by `npm run check`. Exits with code 1 and the list of problems if it finds any.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('../src/app.css', import.meta.url)), 'utf8');
const PALETTES = ['slate', 'ink', 'undergrowth', 'ember'];

/** The body of the block that starts with `selector`, or null. */
function block(selector) {
  const start = css.indexOf(selector);
  if (start < 0) return null;
  const open = css.indexOf('{', start);
  return css.slice(open + 1, css.indexOf('}', open));
}

/** The colors of a block: `--name` → [r, g, b]. */
function colors(body) {
  const out = {};
  for (const [, name, hex] of body.matchAll(/(--[\w-]+):\s*#([0-9a-fA-F]{6});/g)) {
    out[name] = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
  }
  return out;
}

const channel = (c) => {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
};
const luminance = ([r, g, b]) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
const contrast = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};
/** `color-mix(in srgb, a p, b)`. */
const mix = (a, p, b) => a.map((v, i) => v * p + b[i] * (1 - p));
/** The color in CIELAB, D65 white. */
const lab = (rgb) => {
  const [r, g, b] = rgb.map(channel);
  const f = (v) => (v > 0.008856 ? Math.cbrt(v) : 7.787 * v + 16 / 116);
  const x = f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047);
  const y = f(0.2126 * r + 0.7152 * g + 0.0722 * b);
  const z = f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883);
  return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
};
/** The color distance ΔE CIE76: under 2, it cannot be seen; around 10, it is plain to see. */
const distance = (a, b) => Math.hypot(...lab(a).map((v, i) => v - lab(b)[i]));

// Tints of the components that put text on a branch color (Graph.svelte).
const LABEL_REST = 0.24; // background of a label
const LABEL_HOVER = 0.28; // background of a hovered label
const TOWARD_INK = 0.9; // hover on a solid fill: 10% toward --ink
const CAPSULE = 0.15; // capsule of a message: a veil of the color, on the bare, hovered or selected row
const MATCH = 0.3; // search match in the terminal: the accent on --term-bg (Terminal.svelte)

// The minimum color distances, in light then in dark: a hover shows on its background, and the
// selection stands apart from its background and from the hover. Each is the lowest value of the
// four themes, rounded down.
const DISTANCES = [
  ['--hover', '--panel', 6, 10], // a hovered row in a card
  ['--hover', '--raised', 8, 6], // a hovered element in what floats
  ['--hover-side', '--side', 5, 9], // a hovered element on the window background
  ['--accent-soft', '--raised', 10, 18], // a hovered menu item
  ['--sel', '--panel', 18, 24], // a selected row in a card
  ['--sel', '--raised', 18, 21], // a selected row in what floats (the command palette)
  ['--sel', '--hover', 9, 15], // the selection next to a hovered row
];

const findings = [];

/** `dark`: the theme is dark. Every ANSI color reads on it, except black in dark and white in
 *  light: those are backgrounds (guidelines, section 2.6). */
function check(name, t, dark) {
  const need = (what, value, minimum) => {
    if (value < minimum) findings.push(`${name}: ${what} at ${value.toFixed(2)}:1, needs ${minimum}:1`);
  };
  const lanes = Array.from({ length: 8 }, (_, i) => [`--l${i}`, t[`--l${i}`]]);
  for (const bg of ['--panel', '--bg', '--sel', '--hover', '--hover-side']) need(`--ink on ${bg}`, contrast(t['--ink'], t[bg]), 7);
  // Secondary text: in a card, on the window, on what floats, on a hovered row, and in the banner
  // of a stopped operation.
  for (const bg of ['--panel', '--side', '--sel', '--raised', '--hover', '--hover-side', '--warn-soft']) need(`--muted on ${bg}`, contrast(t['--muted'], t[bg]), 4.5);
  // Tinted backgrounds: body text reads on them (a hovered menu item, a banner, an added or removed
  // line), and the --accent icon of an active module or of a pressed action shows on them.
  for (const bg of ['--accent-soft', '--ok-soft', '--warn-soft', '--bad-soft']) need(`--ink on ${bg}`, contrast(t['--ink'], t[bg]), 7);
  need('--accent on --accent-soft', contrast(t['--accent'], t['--accent-soft']), 3);
  // States: in a card, in a field, and in a dialog (the result of a test).
  for (const state of ['--ok', '--warn', '--bad']) {
    for (const bg of ['--panel', '--bg', '--raised']) need(`${state} on ${bg}`, contrast(t[state], t[bg]), 4.5);
  }
  // A ghost danger button and the destructive item of a menu, hovered, and in the banner of a
  // stopped operation.
  for (const bg of ['--bad-soft', '--warn-soft']) need(`--bad on ${bg}`, contrast(t['--bad'], t[bg]), 4.5);
  need('--accent on --panel', contrast(t['--accent'], t['--panel']), 3);
  for (const [a, b, day, night] of DISTANCES) {
    const minimum = dark ? night : day;
    const value = distance(t[a], t[b]);
    if (value < minimum) findings.push(`${name}: distance between ${a} and ${b} at ${value.toFixed(1)}, needs ${minimum}`);
  }
  need('--term-ink on --term-bg', contrast(t['--term-ink'], t['--term-bg']), 7);
  for (let i = 0; i < 16; i++) {
    const token = `--ansi-${i}`;
    if (!t[token]) {
      findings.push(`${name}: ${token} is missing`);
      continue;
    }
    if (i === (dark ? 0 : 7)) continue;
    need(`${token} on --term-bg`, contrast(t[token], t['--term-bg']), 4.5);
  }
  need('--term-ink on a search match', contrast(t['--term-ink'], mix(t['--accent'], MATCH, t['--term-bg'])), 7);
  for (const fill of ['--accent', '--bad']) {
    need(`--on-accent on ${fill}`, contrast(t['--on-accent'], t[fill]), 4.5);
    need(`--on-accent on hovered ${fill}`, contrast(t['--on-accent'], mix(t[fill], TOWARD_INK, t['--ink'])), 4.5);
  }
  for (const [lane, c] of lanes) {
    need(`${lane} on --panel`, contrast(c, t['--panel']), 3);
    need(`--on-accent on ${lane}`, contrast(t['--on-accent'], c), 4.5);
    need(`--on-accent on hovered ${lane}`, contrast(t['--on-accent'], mix(c, TOWARD_INK, t['--ink'])), 4.5);
    need(`--ink on a ${lane} label`, contrast(t['--ink'], mix(c, LABEL_REST, t['--panel'])), 7);
    need(`--ink on a hovered ${lane} label`, contrast(t['--ink'], mix(c, LABEL_HOVER, t['--panel'])), 7);
    for (const bg of ['--panel', '--hover', '--sel']) {
      need(`--ink on a ${lane} capsule, row in ${bg}`, contrast(t['--ink'], mix(c, CAPSULE, t[bg])), 7);
    }
  }
}

for (const palette of PALETTES) {
  const light = block(`[data-palette='${palette}'] {`);
  const system = block(`[data-palette='${palette}']:not([data-theme='light']) {`);
  const chosen = block(`[data-palette='${palette}'][data-theme='dark'] {`);
  if (light === null || system === null || chosen === null) {
    findings.push(`${palette}: one of its three blocks (light, system dark, chosen dark) cannot be found`);
    continue;
  }
  const normal = (body) => body.split(';').map((d) => d.trim()).filter(Boolean).join(';');
  if (normal(system) !== normal(chosen)) findings.push(`${palette}: its two dark blocks do not give the same values`);
  const day = colors(light);
  check(`${palette}, light`, day, false);
  check(`${palette}, dark`, { ...day, ...colors(chosen) }, true);
}

if (findings.length > 0) {
  console.error(`Contrasts and distances: ${findings.length} problem(s).\n`);
  console.error(findings.join('\n'));
  process.exit(1);
}
console.log(`Contrasts and distances: the ${PALETTES.length} themes meet those of the guidelines, in light and in dark.`);
