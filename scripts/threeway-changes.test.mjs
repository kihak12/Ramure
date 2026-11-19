// Tests of the changes that do not conflict, in the conflict editor (src/lib/git/threeway.ts).
// Run by `npm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mergedChanges } from '../src/lib/git/threeway.ts';

// The ancestor: ten settings. Side 1 changes `b` (no conflict) and `e` (conflict); side 2 changes
// `h` (no conflict) and `e` (conflict). The result: git's merge, with its markers around `e`.
const base = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
const ours = ['a', 'B1', 'c', 'd', 'E1', 'f', 'g', 'h', 'i', 'j'];
const theirs = ['a', 'b', 'c', 'd', 'E2', 'f', 'g', 'H2', 'i', 'j'];
const result = ['a', 'B1', 'c', 'd', '<<<<<<< HEAD', 'E1', '=======', 'E2', '>>>>>>> other', 'f', 'g', 'H2', 'i', 'j'];
const conflicts = { ours: [{ from: 4, to: 5 }], theirs: [{ from: 4, to: 5 }], result: [{ from: 4, to: 9 }] };

test('each side marks the lines it changed outside the conflicts', () => {
  const changes = mergedChanges(base, ours, theirs, result, conflicts.ours, conflicts.theirs, conflicts.result);
  assert.deepEqual(changes.ours, [1]);
  assert.deepEqual(changes.theirs, [7]);
});

test('the result says which side each merged line comes from', () => {
  const changes = mergedChanges(base, ours, theirs, result, conflicts.ours, conflicts.theirs, conflicts.result);
  assert.deepEqual(changes.result, [
    { line: 1, side: 'ours' },
    { line: 11, side: 'theirs' },
  ]);
});

test('lines added by a side are marked, on the side and in the result', () => {
  const plus = ['a', 'b', 'new 1', 'new 2', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
  const merged = [...plus];
  const changes = mergedChanges(base, plus, base, merged, [], [], []);
  assert.deepEqual(changes.ours, [2, 3]);
  assert.deepEqual(changes.theirs, []);
  assert.deepEqual(changes.result, [
    { line: 2, side: 'ours' },
    { line: 3, side: 'ours' },
  ]);
});

test('a line changed alike on both sides has no single side in the result', () => {
  const same = ['a', 'X', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
  const changes = mergedChanges(base, same, same, same, [], [], []);
  assert.deepEqual(changes.ours, [1]);
  assert.deepEqual(changes.theirs, [1]);
  assert.deepEqual(changes.result, []);
});

test('a side whose conflicts were not all found marks nothing', () => {
  const changes = mergedChanges(base, ours, theirs, result, null, conflicts.theirs, conflicts.result);
  assert.deepEqual(changes.ours, []);
  assert.deepEqual(changes.theirs, [7]);
  assert.deepEqual(changes.result, [{ line: 11, side: 'theirs' }]);
});

test('without a common ancestor, nothing is marked outside the conflicts', () => {
  assert.deepEqual(mergedChanges(null, ours, theirs, result, [], [], []), { ours: [], theirs: [], result: [] });
});
