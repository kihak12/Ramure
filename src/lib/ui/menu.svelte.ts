// Menu flottant : un seul à la fois. Il s'ouvre à l'endroit du clic droit, ou sous le bouton qui
// le déroule (charte, section 9.9).
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
      /** Une case à cocher : la choisir ne ferme pas le menu. */
      checked?: boolean;
      /** Le raccourci de l'action, comme `Ctrl+Shift+B`, rappelé à droite. */
      keys?: string;
    }
  | { separator: true };

interface Open {
  x: number;
  y: number;
  /** Une fonction quand les éléments changent pendant que le menu est ouvert : des cases à cocher. */
  items: MenuItem[] | (() => MenuItem[]);
  /** Le bouton qui a déroulé le menu : il retrouve le focus à la fermeture. */
  anchor?: HTMLElement;
  /** Ouvert au clavier : le premier élément montre tout de suite qu'il a le focus. */
  keyboard?: boolean;
  /** L'élément d'où un menu contextuel a été demandé au clavier : il retrouve le focus à la fermeture. */
  origin?: HTMLElement;
}

// La touche Menu, ou Maj + F10 : le navigateur envoie l'événement `contextmenu` juste après. On
// retient l'heure de la touche pour reconnaître un menu contextuel demandé au clavier.
let contextKeyAt = -Infinity;
if (typeof window !== 'undefined') {
  window.addEventListener(
    'keydown',
    (event) => {
      if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) contextKeyAt = event.timeStamp;
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
    // Demandé au clavier : sous l'élément qui a le focus, sur son bord gauche, et le premier élément
    // a le focus ; à la fermeture, l'élément le retrouve (charte, 9.9).
    const origin = event.currentTarget instanceof HTMLElement ? event.currentTarget : undefined;
    if (origin && event.timeStamp - contextKeyAt < 500) {
      const box = origin.getBoundingClientRect();
      this.current = { x: box.left, y: box.bottom + 4, items, keyboard: true, origin };
      return;
    }
    this.current = { x: event.clientX, y: event.clientY, items };
  }

  /** Déroule le menu sous `anchor`, aligné sur son bord droit. */
  openUnder(anchor: HTMLElement, items: () => MenuItem[], keyboard = false) {
    const box = anchor.getBoundingClientRect();
    this.current = { x: box.right, y: box.bottom + 4, items, anchor, keyboard };
  }

  /** Vrai tant que le menu déroulé par `anchor` est ouvert. */
  isUnder(anchor: HTMLElement | undefined) {
    return !!anchor && this.current?.anchor === anchor;
  }

  /** `keyboard` : fermé par Échap. Le bouton qui retrouve le focus le montre, comme l'élément
   *  d'où un menu contextuel a été demandé au clavier. */
  close(keyboard = false) {
    const anchor = this.current?.anchor;
    const origin = this.current?.origin;
    this.current = null;
    if (origin) focusWithRing(origin);
    else if (keyboard) focusWithRing(anchor);
    else anchor?.focus();
  }
}

export const menu = new Menu();
