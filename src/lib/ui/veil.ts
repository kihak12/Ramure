// A click on the veil of a dialog cancels it (guidelines, section 9.10).
//
// The click counts if it was pressed and released on the veil. A drag that started in the dialog,
// a text selection or a row being moved and dropped outside does not close it: the browser then
// gives the dialog as the target of the click, as for a real click on the veil.

/** Svelte action: `<dialog use:veil={cancel}>`. The dialog has no padding: what touches the
 *  dialog itself, and not its content, is the veil. */
export function veil(dialog: HTMLDialogElement, cancel: () => void) {
  let close = cancel;
  let pressedOnVeil = false;
  let releasedOnVeil = false;
  const down = (e: PointerEvent) => (pressedOnVeil = e.target === dialog);
  const up = (e: PointerEvent) => (releasedOnVeil = e.target === dialog);
  const click = () => {
    if (pressedOnVeil && releasedOnVeil) close();
    pressedOnVeil = releasedOnVeil = false;
  };
  dialog.addEventListener('pointerdown', down);
  dialog.addEventListener('pointerup', up);
  dialog.addEventListener('click', click);
  return {
    update(next: () => void) {
      close = next;
    },
    destroy() {
      dialog.removeEventListener('pointerdown', down);
      dialog.removeEventListener('pointerup', up);
      dialog.removeEventListener('click', click);
    },
  };
}
