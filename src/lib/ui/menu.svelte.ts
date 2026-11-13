// Floating menu: one at a time. It opens where the right-click was, or under the button that
// drops it down (guidelines, section 9.9).
import { focusWithRing } from './focus';
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
  // keeps the menu of the system (cut, copy, paste).
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
      // A `contextmenu` that nobody handled has the menu of the engine as its default action: the
      // window prevents it last, if it bubbles up to the window without a menu having stopped it.
      const resent = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 2 });
      const quiet = (e: Event) => {
        if (e === resent) e.preventDefault();
      };
      resending = true;
      window.addEventListener('contextmenu', quiet);
      try {
        target.dispatchEvent(resent);
      } finally {
        window.removeEventListener('contextmenu', quiet);
        resending = false;
      }
    },
    true,
  );
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
    this.current = { x: event.clientX, y: event.clientY, items };
  }

  /** Drops the menu down under `anchor`, aligned on its right edge. */
  openUnder(anchor: HTMLElement, items: () => MenuItem[], keyboard = false) {
    const box = anchor.getBoundingClientRect();
    this.current = { x: box.right, y: box.bottom + 4, items, anchor, keyboard };
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
    const origin = this.current?.origin;
    this.current = null;
    if (origin && !outside) focusWithRing(origin);
    else if (origin) origin.focus();
    else if (keyboard) focusWithRing(anchor);
    else anchor?.focus();
  }
}

export const menu = new Menu();
