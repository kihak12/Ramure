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

const TOWARD_INK = 0.9; // hover on a solid fill: 10% toward --ink
const MATCH = 0.3; // search match in the terminal: the accent on --term-bg (Terminal.svelte)
// Tints of the two sides of the conflict editor (ConflictSide, ConflictResult, ConflictEditor).
const SIDE_LINE = 0.1; // a line of a conflict, and a line of the result kept from a side
const SIDE_KEPT = 0.24; // a line kept
const SIDE_CHIP = 0.3; // the chip "1" or "2"
const SIDE_BUTTON = 0.12; // "Keep upstream", "Keep your commit"
const SIDE_BUTTON_HOVER = 0.24;

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

/** The strengths of the tints of a block (section 2.7): `--tint-veil` → 0.24. */
function tints(body) {
  const out = {};
  for (const [, name, value] of body.matchAll(/(--tint-[\w-]+):\s*([\d.]+)%;/g)) out[name] = Number(value) / 100;
  return out;
}
// The strengths are those of Slate, in light and in dark, for every theme: a theme changes colors only (2.5).
const STRENGTHS = {
  light: tints(block(`[data-palette='slate'] {`) ?? ''),
  dark: tints(block(`[data-palette='slate'][data-theme='dark'] {`) ?? ''),
};
// A theme changes colors only: the strengths are written in the blocks of Slate and nowhere else.
for (const palette of PALETTES.filter((p) => p !== 'slate')) {
  for (const selector of [`[data-palette='${palette}'] {`, `[data-palette='${palette}']:not([data-theme='light']) {`, `[data-palette='${palette}'][data-theme='dark'] {`]) {
    if (/--tint-/.test(block(selector) ?? '')) findings.push(`${palette}: a strength of tint is written in its block, where only Slate writes them`);
  }
}
for (const [mode, values] of Object.entries(STRENGTHS)) {
  for (const name of ['--tint-veil', '--tint-lane', '--tint-lane-veil', '--tint-label', '--tint-label-soft', '--tint-label-hover', '--tint-letter', '--tint-added', '--tint-removed', '--tint-line-hover', '--tint-line-pick']) {
    if (!(name in values)) findings.push(`slate, ${mode}: ${name} is missing`);
  }
}

/** `dark`: the theme is dark. Every ANSI color reads on it, except black in dark and white in
 *  light: those are backgrounds (guidelines, section 2.6). */
function check(name, t, dark) {
  const need = (what, value, minimum) => {
    if (value < minimum) findings.push(`${name}: ${what} at ${value.toFixed(2)}:1, needs ${minimum}:1`);
  };
  const s = STRENGTHS[dark ? 'dark' : 'light'];
  const LABEL_REST = Math.max(s['--tint-label'], s['--tint-label-soft']); // background of a label
  const LABEL_HOVER = s['--tint-label-hover']; // background of a hovered label
  const CAPSULE = s['--tint-veil']; // capsule of a message, on the bare, hovered or selected row
  const lanes = Array.from({ length: 8 }, (_, i) => [`--l${i}`, t[`--l${i}`]]);
  for (const bg of ['--panel', '--bg', '--sel', '--hover', '--hover-side']) need(`--ink on ${bg}`, contrast(t['--ink'], t[bg]), 7);
  // Secondary text: in a card, on the window, on what floats, on a hovered row, and in the banner
  // of a stopped operation.
  for (const bg of ['--panel', '--side', '--sel', '--raised', '--hover', '--hover-side', '--warn-soft']) need(`--muted on ${bg}`, contrast(t['--muted'], t[bg]), 4.5);
  // Tinted backgrounds: body text reads on them (a hovered menu item, a banner, an added or removed
  // line), and the --accent icon of an active module or of a pressed action shows on them.
  for (const bg of ['--accent-soft', '--ok-soft', '--warn-soft', '--bad-soft']) need(`--ink on ${bg}`, contrast(t['--ink'], t[bg]), 7);
  need('--accent on --accent-soft', contrast(t['--accent'], t['--accent-soft']), 3);
  need('--muted on --bg (the header of a hunk)', contrast(t['--muted'], t['--bg']), 4.5);
  // The dot that says an update waits: on the rail, at rest and hovered, and on the entry
  // "General" of the settings, current or hovered (9.23).
  for (const bg of ['--side', '--hover-side', '--sel', '--hover']) need(`--accent on ${bg}`, contrast(t['--accent'], t[bg]), 3);
  // States: in a card, in a field, and in a dialog (the result of a test).
  for (const state of ['--ok', '--warn', '--bad']) {
    for (const bg of ['--panel', '--bg', '--raised']) need(`${state} on ${bg}`, contrast(t[state], t[bg]), 4.5);
  }
  // A ghost danger button and the destructive item of a menu, hovered, and in the banner of a
  // stopped operation.
  for (const bg of ['--bad-soft', '--warn-soft']) need(`--bad on ${bg}`, contrast(t['--bad'], t[bg]), 4.5);
  // A transfer that ended badly, on its tinted box (9.24): the icon of an incomplete one, the way
  // of a failed one.
  need('--warn on --warn-soft', contrast(t['--warn'], t['--warn-soft']), 3);
  need('--muted on --bad-soft', contrast(t['--muted'], t['--bad-soft']), 4.5);
  // The date of a folder row that receives a drop (9.24).
  need('--muted on --accent-soft', contrast(t['--muted'], t['--accent-soft']), 4.5);
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
      // The capsule of a commit of the lane under the pointer (10.2).
      need(`--ink on a ${lane} capsule of the lane under the pointer, row in ${bg}`, contrast(t['--ink'], mix(c, s['--tint-lane-veil'], t[bg])), 7);
    }
    // The 3 px line of the lane under the pointer, on its own sleeve (10.2).
    need(`${lane} line on its sleeve`, contrast(c, mix(c, s['--tint-lane'], t['--panel'])), 2);
    // The header of the pane of a host, tinted with its color (9.24): --ink is the capsule's.
    need(`${lane} dot on the header of a host pane`, contrast(c, mix(c, CAPSULE, t['--panel'])), 3);
    need(`--muted icons on the header of a ${lane} host pane`, contrast(t['--muted'], mix(c, CAPSULE, t['--panel'])), 3);
    need(`${lane} author initials on their tint`, contrast(c, mix(c, s['--tint-letter'], t['--panel'])), 3);
  }
  // A line of a diff, at rest, hovered to be picked, and picked (9.14): the code reads on it.
  for (const [state, strength] of [['--ok', s['--tint-added']], ['--bad', s['--tint-removed']]]) {
    for (const [what, p] of [['line', strength], ['hovered line', strength + s['--tint-line-hover']], ['picked line', strength + s['--tint-line-pick']]]) {
      need(`--ink on a ${state} ${what} of a diff`, contrast(t['--ink'], mix(t[state], p, t['--panel'])), 7);
    }
    // The line shows on the card, and the hovered line and the picked line stand apart from it.
    const [rest, hover, picked] = [0, s['--tint-line-hover'], s['--tint-line-pick']].map((d) => mix(t[state], strength + d, t['--panel']));
    for (const [what, a, b, day, night] of [['line and --panel', rest, t['--panel'], 14, 18], ['line and hovered line', rest, hover, 7, 6], ['hovered line and picked line', hover, picked, 5, 5]]) {
      const minimum = dark ? night : day;
      const value = distance(a, b);
      if (value < minimum) findings.push(`${name}: distance between a ${state} ${what} of a diff at ${value.toFixed(1)}, needs ${minimum}`);
    }
  }
  // The letter of a file state, halfway to --ink, on its tint, on a row at rest, hovered or
  // selected (9.6): the tint lets the row show through. The icon of a notification, in its own
  // color, on the same tint over what floats (9.11).
  for (const state of ['--ok', '--warn', '--bad', '--accent']) {
    for (const bg of ['--panel', '--hover', '--sel']) {
      need(`a ${state} letter on its tint, row in ${bg}`, contrast(mix(t[state], 0.5, t['--ink']), mix(t[state], s['--tint-letter'], t[bg])), 4.5);
    }
    need(`a ${state} icon on its tint`, contrast(t[state], mix(t[state], s['--tint-letter'], t['--raised'])), 3);
  }
  // The two sides of the conflict editor take --l0 and --l1 (guidelines, 2.2 and 9.26): the text on
  // their tints, and the ring of the keyboard on a line kept.
  for (const side of ['--l0', '--l1']) {
    const c = t[side];
    const tints = [
      ['a line', SIDE_LINE, '--bg'],
      ['a kept line', SIDE_KEPT, '--bg'],
      ['a chip in a bar', SIDE_CHIP, '--panel'],
      ['a chip in the result', SIDE_CHIP, '--bg'],
      ['a Keep button', SIDE_BUTTON, '--raised'],
      ['a hovered Keep button', SIDE_BUTTON_HOVER, '--raised'],
    ];
    for (const [what, p, bg] of tints) need(`--ink on ${what} of ${side}`, contrast(t['--ink'], mix(c, p, t[bg])), 7);
    need(`--accent ring on a kept line of ${side}`, contrast(t['--accent'], mix(c, SIDE_KEPT, t['--bg'])), 3);
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
