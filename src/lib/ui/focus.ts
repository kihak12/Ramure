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

const last = new WeakMap<HTMLElement, HTMLElement>();

/** Remembers the last element of `root` that had the keyboard (a Svelte action). */
export function remember(root: HTMLElement) {
  const keep = (event: FocusEvent) => {
    if (event.target instanceof HTMLElement) last.set(root, event.target);
  };
  root.addEventListener('focusin', keep);
  return { destroy: () => root.removeEventListener('focusin', keep) };
}

/**
 * A module or a tab was just shown: when the keyboard is nowhere, or in the part that was hidden,
 * it goes to `root`, where it was last, else to its main control (`data-main`), else to its first
 * one, with its ring after a key (guidelines, 11). A focus that is still somewhere is left alone:
 * the rail clicked keeps it.
 */
export function rescue(root: HTMLElement | null | undefined) {
  if (!root) return;
  const active = document.activeElement;
  if (active && active !== document.body && !active.closest('[inert]')) return;
  const usable = (el: HTMLElement | undefined): el is HTMLElement =>
    !!el && el.isConnected && !el.closest('[inert]') && el.getClientRects().length > 0;
  const before = last.get(root);
  const main = [...root.querySelectorAll<HTMLElement>('[data-main]')].find(usable);
  focusBack(usable(before) ? before : (main ?? tabbables(root)[0]));
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
 * leave it for a stop that cannot be seen (guidelines, 9.10 and 11). Installed once, by the window.
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
  window.addEventListener('keydown', onKey, true);
  return () => window.removeEventListener('keydown', onKey, true);
}

/**
 * A control disabled while it has the keyboard, a toolbar button while git works for instance,
 * loses it: WebKit drops the focus to the page. It is given back to the control once enabled
 * again, if nothing else took it meanwhile (guidelines, 11: the focus is never lost). Installed
 * once, by the window.
 */
export function keepFocusThroughDisabling(): () => void {
  let watched: MutationObserver | null = null;
  const stop = () => {
    watched?.disconnect();
    watched = null;
  };
  const onOut = (event: FocusEvent) => {
    const el = event.target;
    if (!(el instanceof HTMLButtonElement || el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement)) return;
    if (event.relatedTarget) return;
    // The attribute is set before the blur it causes: an element blurred for another reason is
    // still enabled here.
    if (!el.disabled) return;
    stop();
    const keyboard = byKeyboard();
    watched = new MutationObserver(() => {
      if (!el.isConnected) return stop();
      if (el.disabled) return;
      stop();
      const active = document.activeElement;
      if (active && active !== document.body) return;
      if (keyboard) focusWithRing(el);
      else el.focus();
    });
    watched.observe(el, { attributes: true, attributeFilter: ['disabled'] });
  };
  // Another element takes the keyboard: the one disabled no longer waits for it.
  const onIn = () => stop();
  document.addEventListener('focusout', onOut, true);
  document.addEventListener('focusin', onIn, true);
  return () => {
    stop();
    document.removeEventListener('focusout', onOut, true);
    document.removeEventListener('focusin', onIn, true);
  };
}

/** The keyboard is nowhere: on the page, or in a part that cannot be reached. */
export function focusLost(): boolean {
  const active = document.activeElement;
  return !active || active === document.body || !active.isConnected || !!active.closest('[inert]');
}

/** Gives the keyboard to the first control of `root` that Tab reaches, with its ring after a key. */
export function focusFirstIn(root: Element | null | undefined): boolean {
  if (!(root instanceof HTMLElement)) return false;
  const first = tabbables(root)[0];
  if (!first) return false;
  focusBack(first);
  return true;
}

/** The first element matching `selector` that can be seen and reached: of a hidden tab, none. */
export function usable<T extends HTMLElement>(selector: string): T | undefined {
  return [...document.querySelectorAll<T>(selector)].find((el) => !el.closest('[inert]') && el.getClientRects().length > 0);
}

/**
 * Gives the keyboard to `el`, with its ring after a key; a control disabled for now (the message
 * field while git works) gets it once enabled, if the keyboard is still nowhere then (11).
 */
export function focusSoon(el: HTMLElement | null | undefined) {
  if (!el) return;
  const disabled = (el as HTMLButtonElement).disabled;
  if (!disabled) return focusBack(el);
  const keyboard = byKeyboard();
  const watch = new MutationObserver(() => {
    if (!el.isConnected) return watch.disconnect();
    if ((el as HTMLButtonElement).disabled) return;
    watch.disconnect();
    if (!focusLost()) return;
    if (keyboard) focusWithRing(el);
    else el.focus();
  });
  watch.observe(el, { attributes: true, attributeFilter: ['disabled'] });
}
