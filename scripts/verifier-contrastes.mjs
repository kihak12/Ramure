// Vérifie que chaque thème de couleurs de src/app.css tient les contrastes et les écarts de couleur
// de la charte (docs/design-guidelines.md, section 2.4), en clair et en sombre, étiquettes de
// branche et capsules comprises. Vérifie aussi que le sombre de chaque thème, écrit deux fois
// (pour qui suit le système, pour qui a choisi « Sombre »), donne les mêmes valeurs.
// Lancé par `npm run check`. Sort avec le code 1 et la liste des problèmes s'il en trouve.
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
/** La couleur en CIELAB, blanc D65. */
const lab = (rgb) => {
  const [r, g, b] = rgb.map(channel);
  const f = (v) => (v > 0.008856 ? Math.cbrt(v) : 7.787 * v + 16 / 116);
  const x = f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047);
  const y = f(0.2126 * r + 0.7152 * g + 0.0722 * b);
  const z = f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883);
  return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
};
/** L'écart de couleur ΔE CIE76 : sous 2, il ne se voit pas ; vers 10, il se voit franchement. */
const distance = (a, b) => Math.hypot(...lab(a).map((v, i) => v - lab(b)[i]));

// Teintes des composants qui posent du texte sur une couleur de branche (Graph.svelte).
const LABEL_REST = 0.24; // fond d'une étiquette
const LABEL_HOVER = 0.28; // fond d'une étiquette survolée
const TOWARD_INK = 0.9; // survol d'un fond plein : 10 % vers --ink
const CAPSULE = 0.15; // capsule d'un message : un voile de la couleur, sur la ligne nue, survolée ou choisie
const MATCH = 0.3; // résultat d'une recherche dans le terminal : l'accent sur --term-bg (Terminal.svelte)

// Les écarts de couleur minimaux, en clair puis en sombre : un survol se voit sur son fond, et la
// sélection reste à part de la carte et du survol. Chacun est la plus basse valeur des quatre
// thèmes, vérifiés à l'écran, arrondie à l'entier inférieur.
const DISTANCES = [
  ['--hover', '--panel', 6, 10], // une ligne survolée dans une carte
  ['--hover', '--raised', 8, 6], // un élément survolé dans ce qui flotte
  ['--hover-side', '--side', 5, 9], // un élément survolé sur le fond de la fenêtre
  ['--accent-soft', '--raised', 10, 18], // un élément de menu survolé
  ['--sel', '--panel', 18, 24], // une ligne sélectionnée dans une carte
  ['--sel', '--hover', 9, 15], // la sélection à côté d'une ligne survolée
];

const findings = [];

/** `dark` : le thème est sombre. Les couleurs ANSI s'y lisent toutes, sauf le noir en sombre et le
 *  blanc en clair : ce sont des fonds (charte, section 2.6). */
function check(name, t, dark) {
  const need = (what, value, minimum) => {
    if (value < minimum) findings.push(`${name} : ${what} à ${value.toFixed(2)}:1, il faut ${minimum}:1`);
  };
  const lanes = Array.from({ length: 8 }, (_, i) => [`--l${i}`, t[`--l${i}`]]);
  for (const bg of ['--panel', '--bg', '--sel', '--hover', '--hover-side']) need(`--ink sur ${bg}`, contrast(t['--ink'], t[bg]), 7);
  // Le texte secondaire : dans une carte, sur la fenêtre, sur ce qui flotte, sur une ligne survolée,
  // et dans le bandeau d'une opération arrêtée.
  for (const bg of ['--panel', '--side', '--sel', '--raised', '--hover', '--hover-side', '--warn-soft']) need(`--muted sur ${bg}`, contrast(t['--muted'], t[bg]), 4.5);
  // Les fonds teintés : le texte courant s'y lit (élément de menu survolé, bandeau, ligne ajoutée
  // ou retirée), et l'icône en --accent d'un module actif ou d'une action enfoncée s'y voit.
  for (const bg of ['--accent-soft', '--ok-soft', '--warn-soft', '--bad-soft']) need(`--ink sur ${bg}`, contrast(t['--ink'], t[bg]), 7);
  need('--accent sur --accent-soft', contrast(t['--accent'], t['--accent-soft']), 3);
  // Les états : dans une carte, dans un champ, et dans une boîte (le résultat d'un test).
  for (const state of ['--ok', '--warn', '--bad']) {
    for (const bg of ['--panel', '--bg', '--raised']) need(`${state} sur ${bg}`, contrast(t[state], t[bg]), 4.5);
  }
  // Un bouton fantôme destructeur et l'élément destructeur d'un menu, survolés, et dans le bandeau
  // d'une opération arrêtée.
  for (const bg of ['--bad-soft', '--warn-soft']) need(`--bad sur ${bg}`, contrast(t['--bad'], t[bg]), 4.5);
  need('--accent sur --panel', contrast(t['--accent'], t['--panel']), 3);
  for (const [a, b, day, night] of DISTANCES) {
    const minimum = dark ? night : day;
    const value = distance(t[a], t[b]);
    if (value < minimum) findings.push(`${name} : écart entre ${a} et ${b} à ${value.toFixed(1)}, il faut ${minimum}`);
  }
  need('--term-ink sur --term-bg', contrast(t['--term-ink'], t['--term-bg']), 7);
  for (let i = 0; i < 16; i++) {
    const token = `--ansi-${i}`;
    if (!t[token]) {
      findings.push(`${name} : ${token} manque`);
      continue;
    }
    if (i === (dark ? 0 : 7)) continue;
    need(`${token} sur --term-bg`, contrast(t[token], t['--term-bg']), 4.5);
  }
  need('--term-ink sur un résultat de recherche', contrast(t['--term-ink'], mix(t['--accent'], MATCH, t['--term-bg'])), 7);
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
    for (const bg of ['--panel', '--hover', '--sel']) {
      need(`--ink sur une capsule ${lane}, ligne en ${bg}`, contrast(t['--ink'], mix(c, CAPSULE, t[bg])), 7);
    }
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
  check(`${palette}, clair`, day, false);
  check(`${palette}, sombre`, { ...day, ...colours(chosen) }, true);
}

if (findings.length > 0) {
  console.error(`Contrastes et écarts : ${findings.length} problème(s).\n`);
  console.error(findings.join('\n'));
  process.exit(1);
}
console.log(`Contrastes et écarts : les ${PALETTES.length} thèmes tiennent ceux de la charte, en clair et en sombre.`);
