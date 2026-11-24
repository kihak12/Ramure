// The lists of files of the changes panel, in conflict, unstaged and staged, are numbered in the
// order of the screen (`data-order` on each `ul.files`, inside the `[data-file-lists]` panel).

/**
 * Where the keyboard goes when a list of files is left empty (guidelines, 9.6): the current row of
 * the next list of the changes, otherwise of the previous one, otherwise the commit message. The
 * lists are read again, after the change: a list the change filled is one of them.
 */
export function besideList(scope: Element | null | undefined, order: number): HTMLElement | null {
  if (!scope) return null;
  const at = (list: HTMLElement) => Number(list.dataset.order);
  const lists = [...scope.querySelectorAll<HTMLElement>('ul.files[data-order]')].filter((list) => list.querySelector('button.open'));
  const next = lists.filter((list) => at(list) > order).sort((a, b) => at(a) - at(b))[0];
  const previous = lists.filter((list) => at(list) < order).sort((a, b) => at(b) - at(a))[0];
  const list = next ?? previous;
  return list?.querySelector<HTMLElement>('button.open[tabindex="0"]') ?? scope.querySelector<HTMLElement>('textarea');
}
