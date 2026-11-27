// Floating menu: one at a time. It opens where the right-click was, or under the button that
// drops it down (guidelines, section 9.9).
import { focusBack, focusWithRing } from './focus';
import type { IconName } from './Icon.svelte';

export type MenuItem =
  | {
      label: string;
      icon?: IconName;
      run: () => void;
      danger?: boolean;
      disabled?: boolean;
      hint?: string;
      /** A checkable item: choosing it does not close the menu. */
      checked?: boolean;
      /** The shortcut of the action, such as `Ctrl+Shift+B`, recalled on the right. */
      keys?: string;
    }
  | { separator: true };

interface Open {
  x: number;
  y: number;
  /** A function when the items change while the menu is open: checkable items. */
  items: MenuItem[] | (() => MenuItem[]);
  /** The button that dropped the menu down: it gets the focus back when the menu closes. */
  anchor?: HTMLElement;
  /** Opened from the keyboard: the first item shows at once that it has the focus. */
  keyboard?: boolean;
  /** The element that had the focus when the context menu was asked for from the keyboard: it
   *  gets it back when the menu closes. It is the row, or the list that points to it (the graph). */
  origin?: HTMLElement;
  /** Opened with a right click: the list that had the focus, holding the row clicked. It gets the
   *  focus back as `origin` does, without being the row the menu was asked for from the keyboard. */
  back?: HTMLElement;
}

// The Menu key, or Shift + F10: the browser sends the `contextmenu` event right after. The time of
// the key is kept, to recognize a context menu asked for from the keyboard.
let contextKeyAt = -Infinity;
const fromKeyboard = (event: Event) => event.timeStamp - contextKeyAt < 500;
/** True while a `contextmenu` asked for from the keyboard is being sent again: it must not be sent twice. */
let resending = false;
/** True if `element` can be seen, at least in part, in the visible area of `list`. */
function inView(element: HTMLElement, list: HTMLElement) {
  const box = element.getBoundingClientRect();
  const frame = list.getBoundingClientRect();
  return box.bottom > frame.top && box.top < frame.bottom;
}
if (typeof window !== 'undefined') {
  window.addEventListener(
    'keydown',
    (event) => {
      if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) contextKeyAt = event.timeStamp;
    },
    true,
  );
  // WebKitGTK sends this `contextmenu` to the element under the bottom left corner of the one that
  // has the focus, not always to that one: its own menu did not open, and the engine showed its
  // own. It is sent again to the element that has the focus, or, for a list that keeps the focus
  // and points to its current row (`aria-activedescendant`, the graph), to that row. A text field
  // keeps the editing menu of the engine (cut, copy, paste): see the listener below.
  window.addEventListener(
    'contextmenu',
    (event) => {
      const focused = document.activeElement;
      if (resending || !fromKeyboard(event) || !(focused instanceof HTMLElement) || focused === document.body) return;
      if (focused.isContentEditable || focused.matches('input, textarea, select')) return;
      const current = focused.getAttribute('aria-activedescendant');
      // Looked for inside the list: a hidden tab keeps its rows, under the same ids.
      const row = current ? focused.querySelector<HTMLElement>(`#${CSS.escape(current)}`) : null;
      // The current row is not in view (the list has scrolled far from it; the graph also keeps
      // rows rendered out of view): nothing opens, neither the menu of the engine nor that of the
      // row that would be under the corner of the list.
      if (current && (!row || !inView(row, focused))) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      // Always sent again, even when the engine already aimed at the right element: an element with
      // no menu (the title of a section) then opens nothing, and never the menu of the engine.
      const target = row || focused;
      event.preventDefault();
      event.stopPropagation();
      // Sent by the script, it has no default action of the engine: one that no menu takes opens
      // nothing.
      resending = true;
      try {
        target.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 2 }));
      } finally {
        resending = false;
      }
    },
    true,
  );
  // A right-click that no menu of the application took opens nothing: the navigation menu of the
  // engine ("Back", "Reload") has no place in an application, and "Reload" would lose the state of
  // the window. Its editing menu stays where it serves: a text field (cut, copy, paste), a
  // terminal, whose field xterm puts under the pointer as the click comes, and a text selected
  // with the mouse (copy) (guidelines, 9.9). From the keyboard, the engine may aim at the element
  // under the corner of the one that has the focus: the focused field counts too.
  window.addEventListener('contextmenu', (event) => {
    if (event.defaultPrevented) return;
    const editing = (element: unknown) => element instanceof Element && (textField(element) || !!element.closest('.xterm'));
    if (editing(event.target) || (fromKeyboard(event) && editing(document.activeElement))) return;
    if (onSelection(event.clientX, event.clientY)) return;
    event.preventDefault();
  });
}

/** True when the point is on the text selected in the page. */
function onSelection(x: number, y: number) {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !selection.rangeCount) return false;
  for (let i = 0; i < selection.rangeCount; i++) {
    for (const box of selection.getRangeAt(i).getClientRects()) {
      if (x >= box.left && x <= box.right && y >= box.top && y <= box.bottom) return true;
    }
  }
  return false;
}

/** The inputs that take no text: a right-click on them gets no menu of the system. */
const NOT_TEXT = new Set(['radio', 'checkbox', 'button', 'submit', 'reset', 'range', 'color', 'file', 'image', 'hidden']);

/** True for a field where text is typed, which keeps the menu of the system (guidelines, 9.9). */
function textField(element: Element) {
  if (element instanceof HTMLInputElement) return !NOT_TEXT.has(element.type);
  if (element instanceof HTMLTextAreaElement) return true;
  return element instanceof HTMLElement && element.isContentEditable;
}

class Menu {
  current = $state.raw<Open | null>(null);

  open(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    event.stopPropagation();
    if (!items.length) {
      this.current = null;
      return;
    }
    // Asked for from the keyboard: under the element that has the focus, on its left edge, and the
    // first item has the focus; when the menu closes, the element gets it back (guidelines, 9.9).
    const element = event.currentTarget instanceof HTMLElement ? event.currentTarget : undefined;
    if (element && fromKeyboard(event)) {
      const box = element.getBoundingClientRect();
      const active = document.activeElement;
      const origin = active instanceof HTMLElement && active !== document.body ? active : element;
      this.current = { x: box.left, y: box.bottom + 4, items, keyboard: true, origin };
      return;
    }
    // With a right click: the list that has the focus and holds the row clicked gets it back.
    const active = document.activeElement;
    const holds = active instanceof HTMLElement && active !== document.body && event.target instanceof Node && active.contains(event.target);
    this.current = { x: event.clientX, y: event.clientY, items, back: holds ? active : undefined };
  }

  /** Drops the menu down under `anchor`, aligned on its right edge. */
  openUnder(anchor: HTMLElement, items: () => MenuItem[], keyboard = false) {
    const box = anchor.getBoundingClientRect();
    this.current = { x: box.right, y: box.bottom + 4, items, anchor, keyboard };
    // A button drawn outside the components, which says it has a menu but not whether it is open
    // ("More" of a conflict): it says so while its menu is open (guidelines, 9.9 and 11).
    if (anchor.dataset.expands !== undefined || (anchor.hasAttribute('aria-haspopup') && !anchor.hasAttribute('aria-expanded'))) {
      anchor.dataset.expands = '';
      anchor.setAttribute('aria-expanded', 'true');
    }
  }

  /** True while the menu dropped down by `anchor` is open. */
  isUnder(anchor: HTMLElement | undefined) {
    return !!anchor && this.current?.anchor === anchor;
  }

  /** `keyboard`: closed by Escape. The button that gets the focus back shows it, as does the element
   *  from which a context menu was asked for from the keyboard. `outside`: closed with the mouse (a
   *  click outside, the wheel, an item chosen with a click); that element gets the focus back
   *  without the ring, as the button does (guidelines, 9.9). */
  close(keyboard = false, outside = false) {
    const anchor = this.current?.anchor;
    if (anchor?.dataset.expands !== undefined) anchor.setAttribute('aria-expanded', 'false');
    const origin = this.current?.origin ?? this.current?.back;
    this.current = null;
    // Closed by the window (its focus lost, its size changed): the ring follows the last gesture.
    if (origin && keyboard) focusWithRing(origin);
    else if (origin && outside) origin.focus();
    else if (origin) focusBack(origin);
    else if (keyboard) focusWithRing(anchor);
    else anchor?.focus();
  }
}

export const menu = new Menu();
