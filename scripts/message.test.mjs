// Tests of how the description of a commit is laid out in the detail panel
// (src/lib/git/message.ts). Run by `npm test`: Node reads the TypeScript file as it is.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readMessage } from '../src/lib/git/message.ts';

test('the lines of a paragraph cut at 72 columns are joined', () => {
  const view = readMessage('Fixing a typo took a detour: the amend\ncheckbox of the changes, or the\ninteractive rebase.\n\nSecond paragraph.');
  assert.deepEqual(view.paragraphs, [
    { text: 'Fixing a typo took a detour: the amend checkbox of the changes, or the interactive rebase.', pre: false },
    { text: 'Second paragraph.', pre: false },
  ]);
  assert.deepEqual(view.trailers, []);
});

test('a list, a quote or an indented block is kept as written', () => {
  for (const block of ['- one\n- two', '* one\n* two', '1. one\n2. two', '> quoted\n> text', 'Code:\n    let x = 1;', '# Heading\ntext']) {
    assert.deepEqual(readMessage(block).paragraphs, [{ text: block, pre: true }], block);
  }
});

test('a number at the start of a sentence does not make a list', () => {
  assert.equal(readMessage('1.5 times faster\nthan before.').paragraphs[0].pre, false);
});

test('the last paragraph of trailers is set apart', () => {
  const view = readMessage('Text.\n\nCloses #182.\n\nCo-Authored-By: A <a@b.c>\nClaude-Session: https://claude.ai/code/x');
  assert.deepEqual(
    view.paragraphs.map((p) => p.text),
    ['Text.', 'Closes #182.'],
  );
  assert.deepEqual(view.trailers, ['Co-Authored-By: A <a@b.c>', 'Claude-Session: https://claude.ai/code/x']);
});

test('a paragraph with one line that is not a trailer stays in the description', () => {
  const view = readMessage('Text.\n\nSigned-off-by: A <a@b.c>\nsee above');
  assert.deepEqual(view.trailers, []);
  assert.equal(view.paragraphs.length, 2);
});

test('a paragraph alone is a description, even when it reads "Area: what changes"', () => {
  // The body of a merge of GitHub: the title of the pull request.
  const title = 'Conflicts: pick the lines to keep, one by one, from both sides (#186)';
  assert.deepEqual(readMessage(title), { paragraphs: [{ text: title, pre: false }], trailers: [] });
  assert.deepEqual(readMessage('Refs: #200').trailers, []);
});

test('nothing', () => {
  assert.deepEqual(readMessage(''), { paragraphs: [], trailers: [] });
  assert.deepEqual(readMessage('\n\n  \n'), { paragraphs: [], trailers: [] });
});

test('Windows line ends and blank lines with spaces separate paragraphs', () => {
  assert.deepEqual(
    readMessage('One\r\ntwo\r\n  \r\nThree').paragraphs.map((p) => p.text),
    ['One two', 'Three'],
  );
});
