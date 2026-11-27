// A focus given by the script, after a keyboard action or an event that the user did not trigger
// with the mouse, does not always get the ring of the browser: WebKit only shows it if the element
// left behind showed it. It is set here, for as long as the focus lasts (class `.focus-ring`,
// app.css): a focus can always be seen (guidelines, section 11).
// The last gesture, the mouse or a key: a focus the application gives back after the mouse
// shows no ring (guidelines, 9.9 and 11).
let byPointer = false;
if (typeof window !== 'undefined') {
  window.addEventListener('pointerdown', () => (byPointer = true), true);
  window.addEventListener('keydown', () => (byPointer = false), true);
}

/** The last gesture was a key, not the mouse: a focus that comes now shows its ring. */
export const byKeyboard = () => !byPointer;

/** Gives the focus back to an element: with its ring after a key, without it after the mouse. */
export function focusBack(element: HTMLElement | null | undefined) {
  if (!element) return;
  if (byPointer) element.focus();
  else focusWithRing(element);
}

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

/** The controls of `root` that Tab reaches, in order: of a group of radio buttons, the one checked. */
function tabbables(root: HTMLElement): HTMLElement[] {
  const all = [...root.querySelectorAll<HTMLElement>('a[href], button, input, select, textarea, [contenteditable="true"], [tabindex]')];
  return all.filter((el) => {
    if (el.tabIndex < 0 || (el as HTMLButtonElement).disabled || el.closest('[inert]') || !el.getClientRects().length) return false;
    if (el instanceof HTMLInputElement && el.type === 'radio' && !el.checked && el.name) {
      return !all.some((other) => other instanceof HTMLInputElement && other.type === 'radio' && other.name === el.name && other.checked);
    }
    return true;
  });
}

/**
 * Tab and Shift + Tab go round the controls of the modal dialog that has the keyboard, and never
 * leave it for a stop that cannot be seen (guidelines, 9.10 and 11). Out of a dialog, they go round
 * the controls of the window the same way: past the last one, or before the first, WebKitGTK gives
 * the keyboard to the window itself, a stop that shows nothing (11). Installed once, by the window.
 */
export function keepFocusInDialogs(): () => void {
  const onKey = (event: KeyboardEvent) => {
    // With Shift, WebKitGTK may not name Tab (ISO_Left_Tab): it is known by its place.
    const tab = event.key === 'Tab' || event.code === 'Tab';
    if (!tab || event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey) return;
    const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const open = [...document.querySelectorAll<HTMLDialogElement>('dialog[open]')];
    const dialog = focused?.closest<HTMLDialogElement>('dialog[open]') ?? open.at(-1);
    if (!dialog) return;
    const items = tabbables(dialog);
    if (!items.length) {
      event.preventDefault();
      return;
    }
    const at = focused ? items.indexOf(focused) : -1;
    const last = items.length - 1;
    if (at < 0 || (!event.shiftKey && at === last) || (event.shiftKey && at === 0)) {
      event.preventDefault();
      // Out of the dialog, or past one end of it: round to the other end.
      focusWithRing(items[event.shiftKey ? last : 0]);
    }
  };
  // After the components: a key they took (the shell of a terminal, a field that keeps Tab) is
  // theirs.
  const onPageKey = (event: KeyboardEvent) => {
    const tab = event.key === 'Tab' || event.code === 'Tab';
    if (!tab || event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey) return;
    if (document.querySelector('dialog[open]')) return;
    const focused = document.activeElement;
    if (!(focused instanceof HTMLElement) || focused === document.body) return;
    const items = tabbables(document.body);
    const first = items[0];
    const last = items.at(-1);
    if (!first || !last) return;
    // Nothing that Tab reaches after the focus, or before it with Shift: round to the other end.
    const beyond = event.shiftKey
      ? !(first.compareDocumentPosition(focused) & Node.DOCUMENT_POSITION_FOLLOWING)
      : !(focused.compareDocumentPosition(last) & Node.DOCUMENT_POSITION_FOLLOWING);
    if (!beyond) return;
    event.preventDefault();
    focusWithRing(event.shiftKey ? last : first);
  };
  window.addEventListener('keydown', onKey, true);
  window.addEventListener('keydown', onPageKey);
  return () => {
    window.removeEventListener('keydown', onKey, true);
    window.removeEventListener('keydown', onPageKey);
  };
}
