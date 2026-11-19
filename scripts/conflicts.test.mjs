// Tests of the order of the lines kept from a conflict (src/lib/git/conflicts.ts). Run by
// `npm test`: Node reads the TypeScript file as it is.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { choose, emptyPick, isDecided, keepSide, parseConflicts, resolved, setAside, sideStatus } from '../src/lib/git/conflicts.ts';

const settings = {
  ours: ['port = 8443', 'workers = 8'],
  theirs: ['port = 9090', 'workers = 16', 'timeout = 60'],
};
const lines = (block, pick) => resolved(block, pick).map((r) => r.line);

test('a side kept with its arrow gives all its lines', () => {
  const pick = keepSide(emptyPick(), 'theirs');
  assert.deepEqual(lines(settings, pick), settings.theirs);
  assert.deepEqual(resolved(settings, pick).map((r) => r.side), ['theirs', 'theirs', 'theirs']);
  assert.equal(sideStatus(pick, 'theirs'), 'kept');
  assert.equal(sideStatus(pick, 'ours'), 'open');
});

test('both sides kept come in the order of the arrows', () => {
  assert.deepEqual(lines(settings, keepSide(keepSide(emptyPick(), 'ours'), 'theirs')), [...settings.ours, ...settings.theirs]);
  assert.deepEqual(lines(settings, keepSide(keepSide(emptyPick(), 'theirs'), 'ours')), [...settings.theirs, ...settings.ours]);
  // A second arrow on a side already kept changes nothing.
  const once = keepSide(emptyPick(), 'ours');
  assert.equal(keepSide(once, 'ours'), once);
});

test('the cross lets a kept side go, or sets an open one aside', () => {
  const kept = keepSide(emptyPick(), 'ours');
  const gone = setAside(kept, 'ours');
  assert.deepEqual(lines(settings, gone), []);
  assert.equal(sideStatus(gone, 'ours'), 'aside');
  assert.equal(isDecided(gone), false, 'one side aside, the other open: still to decide');
  // The arrow takes a side back from aside.
  assert.equal(sideStatus(keepSide(gone, 'ours'), 'ours'), 'kept');
});

test('a conflict is decided once a side is kept, or both are set aside', () => {
  assert.equal(isDecided(emptyPick()), false);
  assert.equal(isDecided(keepSide(emptyPick(), 'theirs')), true);
  const neither = setAside(setAside(emptyPick(), 'ours'), 'theirs');
  assert.equal(isDecided(neither), true);
  assert.deepEqual(lines(settings, neither), []);
});

test('the buttons of the bar: side 1, the one on the left, first in "Keep both"', () => {
  assert.deepEqual(lines(settings, choose('both')), [...settings.ours, ...settings.theirs]);
  assert.deepEqual(lines(settings, choose('both', 'theirs')), [...settings.theirs, ...settings.ours], 'in a rebase, side 1 is theirs');
  assert.deepEqual(lines(settings, choose('both-reversed', 'theirs')), [...settings.ours, ...settings.theirs]);
  assert.deepEqual(lines(settings, choose('ours')), settings.ours);
  assert.equal(isDecided(choose('neither')), true);
  assert.deepEqual(lines(settings, choose('neither')), []);
});

test('the markers of git are read, with or without the common ancestor', () => {
  const text = ['a', '<<<<<<< HEAD', 'one', '||||||| base', 'zero', '=======', 'two', '>>>>>>> topic', 'b'];
  const [block] = parseConflicts(text);
  assert.deepEqual([block.ours, block.base, block.theirs, block.start, block.end], [['one'], ['zero'], ['two'], 1, 7]);
  const [plain] = parseConflicts(['<<<<<<< HEAD', 'one', '=======', '>>>>>>> topic']);
  assert.deepEqual([plain.ours, plain.base, plain.theirs], [['one'], null, []]);
});
