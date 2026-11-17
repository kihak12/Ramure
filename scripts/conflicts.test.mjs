// Tests of the order of the lines kept from a conflict (src/lib/git/conflicts.ts). Run by
// `npm test`: Node reads the TypeScript file as it is.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { choose, emptyPick, isDecided, parseConflicts, pickLines, pickSide, resolved, sideState } from '../src/lib/git/conflicts.ts';

const settings = {
  ours: ['port = 8443', 'workers = 8', 'timeout = 45', 'log_level = "info"', 'max_body = "10MB"'],
  theirs: ['port = 9090', 'workers = 16', 'timeout = 60', 'log_level = "debug"', 'max_body = "1MB"'],
};
const lines = (block, pick) => resolved(block, pick).map((r) => r.line);

test('lines picked one by one from both sides go row by row', () => {
  let pick = emptyPick(settings);
  pick = pickLines(pick, 'ours', 0, 0, true);
  pick = pickLines(pick, 'theirs', 1, 3, true);
  pick = pickLines(pick, 'ours', 4, 4, true);
  assert.deepEqual(lines(settings, pick), ['port = 8443', 'workers = 16', 'timeout = 60', 'log_level = "debug"', 'max_body = "10MB"']);
  assert.deepEqual(resolved(settings, pick).map((r) => r.side), ['ours', 'theirs', 'theirs', 'theirs', 'ours']);
});

test('the order does not depend on the order of the clicks', () => {
  const a = pickLines(pickLines(emptyPick(settings), 'theirs', 1, 1, true), 'ours', 0, 0, true);
  const b = pickLines(pickLines(emptyPick(settings), 'ours', 0, 0, true), 'theirs', 1, 1, true);
  assert.deepEqual(lines(settings, a), lines(settings, b));
});

test('keeping both gives one side then the other, or the reverse', () => {
  assert.deepEqual(lines(settings, choose(settings, 'both')), [...settings.ours, ...settings.theirs]);
  assert.deepEqual(lines(settings, choose(settings, 'both-reversed')), [...settings.theirs, ...settings.ours]);
  // A row that keeps both sides breaks the row order: side by side.
  const pick = pickLines(pickLines(emptyPick(settings), 'ours', 0, 1, true), 'theirs', 1, 1, true);
  assert.deepEqual(lines(settings, pick), ['port = 8443', 'workers = 8', 'workers = 16']);
});

test('sides of different lengths keep the order of the sides', () => {
  const block = { ours: ['a', 'b'], theirs: ['x', 'y', 'z'] };
  const pick = pickLines(pickLines(emptyPick(block), 'theirs', 2, 2, true), 'ours', 1, 1, true);
  assert.deepEqual(lines(block, pick), ['b', 'z']);
});

test('a conflict is decided once a line is kept, or neither is', () => {
  const pick = emptyPick(settings);
  assert.equal(isDecided(pick), false);
  assert.equal(isDecided(choose(settings, 'neither')), true);
  assert.deepEqual(lines(settings, choose(settings, 'neither')), []);
  const one = pickLines(pick, 'theirs', 2, 2, true);
  assert.equal(isDecided(one), true);
  // Letting go of the last line leaves it to decide again.
  assert.equal(isDecided(pickLines(one, 'theirs', 2, 2, false)), false);
});

test('the checkbox of a side says all, some or none', () => {
  const pick = pickLines(emptyPick(settings), 'ours', 1, 2, true);
  assert.equal(sideState(pick, 'ours'), 'mixed');
  assert.equal(sideState(pick, 'theirs'), 'off');
  assert.equal(sideState(pickSide(pick, 'ours', true), 'ours'), 'on');
  assert.equal(sideState(pickSide(pick, 'ours', false), 'ours'), 'off');
});

test('the markers of git are read, with or without the common ancestor', () => {
  const text = ['a', '<<<<<<< HEAD', 'one', '||||||| base', 'zero', '=======', 'two', '>>>>>>> topic', 'b'];
  const [block] = parseConflicts(text);
  assert.deepEqual([block.ours, block.base, block.theirs, block.start, block.end], [['one'], ['zero'], ['two'], 1, 7]);
  const [plain] = parseConflicts(['<<<<<<< HEAD', 'one', '=======', '>>>>>>> topic']);
  assert.deepEqual([plain.ours, plain.base, plain.theirs], [['one'], null, []]);
});
