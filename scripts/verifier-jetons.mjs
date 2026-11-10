// Vérifie qu'aucun composant n'écrit en dur une valeur qui a un jeton dans src/app.css :
// couleur, taille ou graisse de texte, rayon, durée (docs/design-guidelines.md).
// Lancé par `npm run check`. Sort avec le code 1 et la liste des écarts s'il en trouve.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const source = join(root, 'src');
const tokens = join(source, 'app.css'); // le seul fichier qui a le droit d'écrire ces valeurs

const TIME = /(?<![\w.-])\d*\.?\d+m?s\b/;

/** Chaque règle : ce qu'on cherche sur une ligne, et le jeton à employer à la place. */
const RULES = [
  { test: (l) => /(?<![&\w])#[0-9a-fA-F]{3,8}\b(?!['"]?\))/.test(l) && /(color|background|border|fill|stroke|shadow|outline)/i.test(l), why: 'couleur en dur : employer un jeton de couleur' },
  { test: (l) => /\b(rgba?|hsla?|oklch|oklab)\(/.test(l), why: 'couleur en dur : employer un jeton, ou color-mix() entre jetons' },
  { test: (l) => /font-size:\s*\d/.test(l) || /\bfont:\s*\d/.test(l) || /fontSize:\s*['"]?\d/.test(l), why: 'taille de texte en dur : employer --fs-*' },
  { test: (l) => /font-weight:\s*\d/.test(l) || /fontWeight:\s*['"]?\d/.test(l), why: 'graisse en dur : employer --fw-regular ou --fw-strong' },
  { test: (l) => /border-radius:[^;]*\d+(\.\d+)?px/.test(l) || /borderRadius:\s*['"]?\d/.test(l), why: 'rayon en dur : employer --radius-*' },
  { test: (l) => /\b(transition|animation)(-duration|-delay)?:/.test(l) && TIME.test(l.replace(/var\([^)]*\)/g, '')), why: 'durée en dur : employer --t-*' },
  { test: (l) => /\bduration:\s*\d/.test(l), why: 'durée en dur : employer motion.fast, motion.med ou motion.slow' },
];

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (/\.(svelte|css|ts)$/.test(name)) yield path;
  }
}

const findings = [];
for (const file of walk(source)) {
  if (file === tokens) continue;
  const lines = readFileSync(file, 'utf8').split('\n');
  lines.forEach((line, index) => {
    const text = line.trim();
    if (text.startsWith('//') || text.startsWith('/*') || text.startsWith('*') || text.startsWith('<!--')) return;
    for (const rule of RULES) {
      if (rule.test(line)) findings.push(`${relative(root, file)}:${index + 1}  ${rule.why}\n    ${text}`);
    }
  });
}

// Le thème sombre est écrit deux fois dans src/app.css : pour qui suit le système, et pour qui a
// choisi « Sombre » dans les réglages. Les deux blocs doivent donner les mêmes valeurs.
{
  const css = readFileSync(tokens, 'utf8');
  const declarations = (selector) => {
    const start = css.indexOf(selector);
    if (start < 0) return null;
    const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
    return body
      .split(';')
      .map((d) => d.trim())
      .filter(Boolean)
      .join(';\n');
  };
  const system = declarations(":root:not([data-theme='light'])");
  const chosen = declarations(":root[data-theme='dark']");
  if (system === null || chosen === null) findings.push('src/app.css  un des deux blocs du thème sombre est introuvable');
  else if (system !== chosen) findings.push('src/app.css  les deux blocs du thème sombre ne donnent pas les mêmes valeurs');
}

if (findings.length > 0) {
  console.error(`Jetons : ${findings.length} valeur(s) écrite(s) en dur.\n`);
  console.error(findings.join('\n'));
  process.exit(1);
}
console.log('Jetons : aucune valeur écrite en dur dans les composants.');
