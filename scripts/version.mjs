// The version of Ramure is written in four files, which must agree: package.json (and its
// lock), src-tauri/Cargo.toml (and its lock) and src-tauri/tauri.conf.json, the one the installers
// and the updater read.
//
//   node scripts/version.mjs               checks that they agree
//   node scripts/version.mjs 0.2.0         writes 0.2.0 in all of them
//   node scripts/version.mjs --tag v0.2.0  checks that they agree and match the tag of a release,
//                                          and that the releases can be verified by the updater
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('..', import.meta.url);
const path = (name) => fileURLToPath(new URL(name, root));
const read = (name) => readFileSync(path(name), 'utf8');
const write = (name, text) => writeFileSync(path(name), text);

// The version of the package `ramure`, in Cargo.toml and in Cargo.lock: the first `version` line
// that follows `name = "ramure"`.
const CARGO = /(name = "ramure"\r?\nversion = ")([^"]+)(")/;
const JSON_VERSION = /("version": ")([^"]+)(")/;

const files = [
  { name: 'package.json', pattern: JSON_VERSION },
  // The lock repeats it twice: for the project, and for its root package.
  { name: 'package-lock.json', pattern: JSON_VERSION, twice: /("packages": \{\s*"": \{\s*"name": "ramure",\s*"version": ")([^"]+)(")/ },
  { name: 'src-tauri/Cargo.toml', pattern: CARGO },
  { name: 'src-tauri/Cargo.lock', pattern: CARGO },
  { name: 'src-tauri/tauri.conf.json', pattern: JSON_VERSION },
];

function found(file) {
  const text = read(file.name);
  const versions = [file.pattern, file.twice].filter(Boolean).map((pattern) => text.match(pattern)?.[2]);
  if (versions.some((version) => !version)) throw new Error(`${file.name}: no version found.`);
  return versions;
}

function check(expected) {
  const all = files.map((file) => ({ name: file.name, versions: found(file) }));
  const wanted = expected ?? all[0].versions[0];
  const wrong = all.filter((file) => file.versions.some((version) => version !== wanted));
  if (wrong.length) {
    for (const file of wrong) console.error(`${file.name}: ${file.versions.join(', ')}, expected ${wanted}.`);
    console.error('Run `npm run version:set -- <version>` to write the same version everywhere.');
    process.exit(1);
  }
  console.log(`Version: ${wanted}, the same in ${files.length} files.`);
}

const [arg, value] = process.argv.slice(2);
// Three numbers, and an optional pre-release: what the installers and the updater can compare.
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

if (!arg) {
  check();
} else if (arg === '--tag') {
  if (!value?.startsWith('v') || !SEMVER.test(value.slice(1))) {
    console.error(`"${value ?? ''}" is not the tag of a release: expected "v" and a version, as in v0.2.0.`);
    process.exit(1);
  }
  check(value.slice(1));
  // A release without the public key of the updater would install copies that can never update.
  const updater = JSON.parse(read('src-tauri/tauri.conf.json')).plugins?.updater;
  if (!updater?.pubkey?.trim()) {
    console.error('src-tauri/tauri.conf.json: plugins.updater.pubkey is empty. See "Once: the key of the updates" in docs/releasing.md.');
    process.exit(1);
  }
  console.log('Updates: the public key is set.');
} else if (SEMVER.test(arg)) {
  for (const file of files) {
    let text = read(file.name);
    for (const pattern of [file.pattern, file.twice].filter(Boolean)) text = text.replace(pattern, `$1${arg}$3`);
    write(file.name, text);
  }
  check(arg);
} else {
  console.error(`"${arg}" is not a version: expected three numbers, as in 0.2.0.`);
  process.exit(1);
}
