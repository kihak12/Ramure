// Text of the "Clean up branches" panel.

/** Summary of the selection, beside the list. */
export function summary(count: number): string {
  if (count === 0) return 'No branches selected.';
  return `${count} branches selected.`;
}

/** Title of the confirmation, before the deletion. */
export function confirmTitle(count: number): string {
  return `Delete ${count} branch${count === 1 ? '' : 'es'}?`;
}

/** Message shown when a deletion fails. */
export function failure(): string {
  return 'Oops, the deletion failed!';
}
