// Tests of the choice of the program that opens a file (src/lib/files/opening.ts). Run by `npm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { exceptionFor, extensionOf, matchFor, matches } from '../src/lib/files/opening.ts';

test('the extension of a name, in lower case, none for a name with no dot or a dotfile', () => {
  assert.equal(extensionOf('nginx.CONF'), '.conf');
  assert.equal(extensionOf('archive.tar.gz'), '.gz');
  assert.equal(extensionOf('Dockerfile'), '');
  assert.equal(extensionOf('.bashrc'), '');
});

test('an exception names an extension, with or without its dot, or a whole name', () => {
  assert.ok(matches('.conf', 'nginx.conf'));
  assert.ok(matches('conf', 'nginx.CONF'));
  assert.ok(matches('Dockerfile', 'dockerfile'));
  assert.ok(matches('.bashrc', '.bashrc'));
  assert.ok(!matches('conf', 'nginx.config'));
  assert.ok(!matches('', 'nginx.conf'));
});

test('the first exception that names the file wins; one with no program is skipped', () => {
  const rules = [
    { match: '.md', program: '' },
    { match: 'md', program: 'typora' },
    { match: '.md', program: 'code' },
  ];
  assert.equal(exceptionFor('README.md', rules), 'typora');
  assert.equal(exceptionFor('notes.txt', rules), null);
});

test('"Always open" keeps the extension, or the name when there is none', () => {
  assert.deepEqual(matchFor('nginx.conf'), { match: '.conf', label: 'Always open .conf files with it' });
  assert.deepEqual(matchFor('Dockerfile'), { match: 'Dockerfile', label: 'Always open files named Dockerfile with it' });
});
