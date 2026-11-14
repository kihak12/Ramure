// Checks that no component hard-codes a value that has a token in src/app.css: a color, a text
// size or weight, a radius, a duration (docs/design-guidelines.md).
// Run by `npm run check`. Exits with code 1 and the list of the values it finds, if any.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const source = join(root, 'src');
const tokens = join(source, 'app.css'); // the only file allowed to write these values

const TIME = /(?<![\w.-])\d*\.?\d+m?s\b/;

/** Each rule: what is looked for on a line, and the token to use instead. */
const RULES = [
  { test: (l) => /(?<![&\w])#[0-9a-fA-F]{3,8}\b(?!['"]?\))/.test(l) && /(color|background|border|fill|stroke|shadow|outline)/i.test(l), why: 'hard-coded color: use a color token' },
  { test: (l) => /\b(rgba?|hsla?|oklch|oklab)\(/.test(l), why: 'hard-coded color: use a token, or color-mix() between tokens' },
  { test: (l) => /font-size:\s*\d/.test(l) || /\bfont:\s*\d/.test(l) || /fontSize:\s*['"]?\d/.test(l), why: 'hard-coded text size: use --fs-*' },
  { test: (l) => /font-weight:\s*\d/.test(l) || /fontWeight:\s*['"]?\d/.test(l), why: 'hard-coded weight: use --fw-regular or --fw-strong' },
  { test: (l) => /border-radius:[^;]*\d+(\.\d+)?px/.test(l) || /borderRadius:\s*['"]?\d/.test(l), why: 'hard-coded radius: use --radius-*' },
  { test: (l) => /\b(transition|animation)(-duration|-delay)?:/.test(l) && TIME.test(l.replace(/var\([^)]*\)/g, '')), why: 'hard-coded duration: use --t-*' },
  { test: (l) => /\bduration:\s*\d/.test(l), why: 'hard-coded duration: use motion.fast, motion.med or motion.slow' },
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

if (findings.length > 0) {
  console.error(`Tokens: ${findings.length} hard-coded value(s).\n`);
  console.error(findings.join('\n'));
  process.exit(1);
}
console.log('Tokens: no value hard-coded in the components.');
