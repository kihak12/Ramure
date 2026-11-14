// Vérifie que deux fichiers de src/ ne se confondent jamais sur un disque qui ignore la casse
// (Windows, macOS). Un import s'écrit sans `.ts` : `./toasts.svelte` désigne `toasts.svelte.ts`
// sous Linux, mais `Toasts.svelte` sous Windows s'il existe, et la page reste blanche.
// Lancé par `npm run check`. Sort avec le code 1 et la liste des paires s'il en trouve.
import { readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const source = join(root, 'src');

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else yield path;
  }
}

/** Le nom sous lequel un import peut désigner le fichier : sans `.ts` ni `.js`, sans casse. */
const key = (path) => path.replace(/\.(ts|js)$/, '').toLowerCase();

const seen = new Map();
const clashes = [];
for (const file of walk(source)) {
  const path = relative(root, file).replaceAll('\\', '/');
  const other = seen.get(key(path));
  if (other) clashes.push([other, path]);
  else seen.set(key(path), path);
}

if (clashes.length > 0) {
  console.error('Fichiers qui se confondent sur un disque qui ignore la casse :');
  for (const [a, b] of clashes) console.error(`  ${a}\n  ${b}\n`);
  console.error('Renomme l’un des deux : sous Windows, un import de l’un charge l’autre.');
  process.exit(1);
}
console.log(`Noms de fichiers : ${seen.size} fichiers, aucun ne se confond avec un autre.`);
