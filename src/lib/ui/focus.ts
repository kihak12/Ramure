// A focus given by the script, after a keyboard action or an event that the user did not trigger
// with the mouse, does not always get the ring of the browser: WebKit only shows it if the element
// left behind showed it. It is set here, for as long as the focus lasts (class `.focus-ring`,
// app.css): a focus can always be seen (guidelines, section 11).
export function focusWithRing(element: HTMLElement | null | undefined) {
  if (!element) return;
  element.focus();
  element.classList.add('focus-ring');
  // The ring goes with the focus, or at the first click: the mouse takes over again, and an element
  // that keeps the focus (the list of the graph, one of whose rows is clicked) no longer shows it.
  const off = () => {
    element.classList.remove('focus-ring');
    element.removeEventListener('blur', off);
    window.removeEventListener('pointerdown', off, true);
  };
  element.addEventListener('blur', off);
  window.addEventListener('pointerdown', off, true);
}
