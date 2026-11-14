// Checks that no two files of src/ are mistaken for each other on a disk that ignores case
// (Windows, macOS). An import is written without `.ts`: `./toasts.svelte` means `toasts.svelte.ts`
// on Linux, but `Toasts.svelte` on Windows if it exists, and the page stays blank.
// Run by `npm run check`. Exits with code 1 and the list of the pairs it finds, if any.
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

/** The name under which an import can point to the file: without `.ts` or `.js`, without case. */
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
  console.error('Files mistaken for each other on a disk that ignores case:');
  for (const [a, b] of clashes) console.error(`  ${a}\n  ${b}\n`);
  console.error('Rename one of the two: on Windows, an import of one loads the other.');
  process.exit(1);
}
console.log(`File names: ${seen.size} files, none mistaken for another.`);
