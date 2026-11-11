// Vérifie que chaque thème de couleurs de src/app.css tient les contrastes de la charte
// (docs/design-guidelines.md, section 2.4), en clair et en sombre, étiquettes de branche et
// capsules comprises. Vérifie aussi que le sombre de chaque thème, écrit deux fois (pour qui suit
// le système, pour qui a choisi « Sombre »), donne les mêmes valeurs.
// Lancé par `npm run check`. Sort avec le code 1 et la liste des écarts s'il en trouve.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('../src/app.css', import.meta.url)), 'utf8');
const PALETTES = ['ardoise', 'encre', 'sous-bois', 'braise'];

/** Le corps du bloc qui commence par `selector`, ou null. */
function block(selector) {
  const start = css.indexOf(selector);
  if (start < 0) return null;
  const open = css.indexOf('{', start);
  return css.slice(open + 1, css.indexOf('}', open));
}

/** Les couleurs d'un bloc : `--nom` → [r, g, b]. */
function colours(body) {
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

// Teintes des composants qui posent du texte sur une couleur de branche (Graph.svelte).
const LABEL_REST = 0.24; // fond d'une étiquette
const LABEL_HOVER = 0.28; // fond d'une étiquette survolée
const TOWARD_INK = 0.9; // survol d'un fond plein : 10 % vers --ink

const findings = [];

function check(name, t) {
  const need = (what, value, minimum) => {
    if (value < minimum) findings.push(`${name} : ${what} à ${value.toFixed(2)}:1, il faut ${minimum}:1`);
  };
  const lanes = Array.from({ length: 8 }, (_, i) => [`--l${i}`, t[`--l${i}`]]);
  for (const bg of ['--panel', '--bg', '--sel', '--hover']) need(`--ink sur ${bg}`, contrast(t['--ink'], t[bg]), 7);
  for (const bg of ['--panel', '--side', '--sel']) need(`--muted sur ${bg}`, contrast(t['--muted'], t[bg]), 4.5);
  for (const state of ['--ok', '--warn', '--bad']) {
    for (const bg of ['--panel', '--bg']) need(`${state} sur ${bg}`, contrast(t[state], t[bg]), 4.5);
  }
  need('--accent sur --panel', contrast(t['--accent'], t['--panel']), 3);
  for (const fill of ['--accent', '--bad']) {
    need(`--on-accent sur ${fill}`, contrast(t['--on-accent'], t[fill]), 4.5);
    need(`--on-accent sur ${fill} survolé`, contrast(t['--on-accent'], mix(t[fill], TOWARD_INK, t['--ink'])), 4.5);
  }
  for (const [lane, c] of lanes) {
    need(`${lane} sur --panel`, contrast(c, t['--panel']), 3);
    need(`--on-accent sur ${lane}`, contrast(t['--on-accent'], c), 4.5);
    need(`--on-accent sur ${lane} survolé`, contrast(t['--on-accent'], mix(c, TOWARD_INK, t['--ink'])), 4.5);
    need(`--ink sur une étiquette ${lane}`, contrast(t['--ink'], mix(c, LABEL_REST, t['--panel'])), 7);
    need(`--ink sur une étiquette ${lane} survolée`, contrast(t['--ink'], mix(c, LABEL_HOVER, t['--panel'])), 7);
  }
}

for (const palette of PALETTES) {
  const light = block(`[data-palette='${palette}'] {`);
  const system = block(`[data-palette='${palette}']:not([data-theme='light']) {`);
  const chosen = block(`[data-palette='${palette}'][data-theme='dark'] {`);
  if (light === null || system === null || chosen === null) {
    findings.push(`${palette} : un de ses trois blocs (clair, sombre du système, sombre choisi) est introuvable`);
    continue;
  }
  const normal = (body) => body.split(';').map((d) => d.trim()).filter(Boolean).join(';');
  if (normal(system) !== normal(chosen)) findings.push(`${palette} : les deux blocs du sombre ne donnent pas les mêmes valeurs`);
  const day = colours(light);
  check(`${palette}, clair`, day);
  check(`${palette}, sombre`, { ...day, ...colours(chosen) });
}

if (findings.length > 0) {
  console.error(`Contrastes : ${findings.length} écart(s).\n`);
  console.error(findings.join('\n'));
  process.exit(1);
}
console.log(`Contrastes : les ${PALETTES.length} thèmes tiennent ceux de la charte, en clair et en sombre.`);
