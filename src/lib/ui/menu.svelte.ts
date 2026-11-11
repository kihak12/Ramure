// Menu flottant : un seul à la fois. Il s'ouvre à l'endroit du clic droit, ou sous le bouton qui
// le déroule (charte, section 9.9).
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
}

class Menu {
  current = $state.raw<Open | null>(null);

  open(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    event.stopPropagation();
    this.current = items.length ? { x: event.clientX, y: event.clientY, items } : null;
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

  /** `keyboard` : fermé par Échap. Le bouton qui retrouve le focus le montre (classe `.ring`, app.css). */
  close(keyboard = false) {
    const anchor = this.current?.anchor;
    this.current = null;
    if (!anchor) return;
    anchor.focus();
    if (!keyboard) return;
    // Un focus donné par le script n'a pas toujours l'anneau du navigateur : on le pose, le temps du focus.
    anchor.classList.add('ring');
    anchor.addEventListener('blur', () => anchor.classList.remove('ring'), { once: true });
  }
}

export const menu = new Menu();
