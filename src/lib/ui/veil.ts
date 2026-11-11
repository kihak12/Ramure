// Un clic sur le voile d'une boîte de dialogue l'annule (charte, section 9.10).
//
// Le clic compte s'il a été pressé et relâché sur le voile. Un glissé parti de la boîte, une
// sélection de texte ou une ligne qu'on déplace et qu'on lâche dehors, ne la ferme pas : le
// navigateur donne alors la boîte pour cible au clic, comme pour un vrai clic sur le voile.

/** Action Svelte : `<dialog use:veil={annuler}>`. La boîte n'a pas de marge intérieure : ce qui
 *  touche la boîte elle-même, et non son contenu, est le voile. */
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
