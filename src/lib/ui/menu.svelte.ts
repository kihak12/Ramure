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
  /** L'élément qui avait le focus quand le menu contextuel a été demandé au clavier : il le
   *  retrouve à la fermeture. C'est la ligne, ou la liste qui la désigne (le graphe). */
  origin?: HTMLElement;
}

// La touche Menu, ou Maj + F10 : le navigateur envoie l'événement `contextmenu` juste après. On
// retient l'heure de la touche pour reconnaître un menu contextuel demandé au clavier.
let contextKeyAt = -Infinity;
const fromKeyboard = (event: Event) => event.timeStamp - contextKeyAt < 500;
/** Vrai pendant que l'on renvoie un `contextmenu` demandé au clavier : il ne doit pas l'être deux fois. */
let resending = false;
/** Vrai si `element` se voit, au moins en partie, dans la zone visible de `list`. */
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
  // WebKitGTK envoie ce `contextmenu` à l'élément qui se trouve sous le coin bas gauche de celui qui
  // a le focus, pas toujours à lui : son propre menu ne s'ouvrait pas, et le moteur montrait le sien.
  // On le renvoie à l'élément qui a le focus, ou, pour une liste qui garde le focus et désigne sa
  // ligne courante (`aria-activedescendant`, le graphe), à cette ligne. Un champ de texte garde le
  // menu du système (couper, copier, coller).
  window.addEventListener(
    'contextmenu',
    (event) => {
      const focused = document.activeElement;
      if (resending || !fromKeyboard(event) || !(focused instanceof HTMLElement) || focused === document.body) return;
      if (focused.isContentEditable || focused.matches('input, textarea, select')) return;
      const current = focused.getAttribute('aria-activedescendant');
      // Cherchée dans la liste : un onglet masqué garde ses lignes, sous les mêmes identifiants.
      const row = current ? focused.querySelector<HTMLElement>(`#${CSS.escape(current)}`) : null;
      // La ligne courante n'est pas en vue (la liste a défilé loin d'elle ; le graphe garde aussi des
      // lignes rendues hors de la vue) : rien ne s'ouvre, ni le menu du moteur, ni celui de la ligne
      // qui se trouverait sous le coin de la liste.
      if (current && (!row || !inView(row, focused))) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      // Toujours renvoyé, même quand le moteur visait déjà le bon élément : un élément sans menu
      // (un titre de section) n'ouvre alors rien, et jamais le menu du moteur.
      const target = row || focused;
      event.preventDefault();
      event.stopPropagation();
      // Un `contextmenu` que personne n'a pris en charge a pour action par défaut le menu du moteur :
      // la fenêtre l'annule en dernier, s'il remonte jusqu'à elle sans qu'un menu l'ait arrêté.
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
    // Demandé au clavier : sous l'élément qui a le focus, sur son bord gauche, et le premier élément
    // a le focus ; à la fermeture, l'élément le retrouve (charte, 9.9).
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
   *  d'où un menu contextuel a été demandé au clavier. `outside` : fermé à la souris (un clic à
   *  côté, la molette, un élément choisi au clic) ; cet élément retrouve le focus sans l'anneau,
   *  comme le bouton (charte, 9.9). */
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
