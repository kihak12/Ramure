import { tick } from 'svelte';
import { focusBack, focusFirstIn, focusLost, usable } from '../ui/focus';
import type { Repo } from './repo.svelte';

/** "Show working changes", "Show files": the work panel takes the place of the button pressed,
 *  and the keyboard goes to its first list (guidelines, 9.6 and 11). */
export async function showFiles(repo: Repo) {
  repo.showWip();
  await tick();
  if (!focusLost()) return;
  // The current row of the first list, else the first control of the panel.
  const row = usable('[data-file-lists] button.open[tabindex="0"]');
  if (row) focusBack(row);
  else focusFirstIn(usable('[data-file-lists]'));
}
