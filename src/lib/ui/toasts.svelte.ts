// Notifications éphémères, affichées en bas, au centre de la fenêtre.

export type ToastKind = 'ok' | 'warn' | 'err';

/** L'action qu'une notification peut proposer : défaire ce qui vient d'être fait. */
export interface ToastAction {
  label: string;
  run: () => void;
  /** Ce qu'elle défait vient d'être lancé au clavier : le focus va sur son bouton. */
  focus?: boolean;
}

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  body: string;
  action?: ToastAction;
}

/** Les sorties bavardes de git (push, fetch) sont réduites à leurs dernières lignes. */
const MAX_LINES = 8;

class Toasts {
  items = $state<Toast[]>([]);
  #next = 1;
  /** Minuteries des notifications qui s'effacent seules. */
  #timers = new Map<number, number>();

  push(kind: ToastKind, title: string, body = '', action?: ToastAction) {
    const id = this.#next++;
    const lines = body.trim().split('\n');
    const text = kind === 'ok' ? lines.slice(-MAX_LINES).join('\n') : lines.join('\n');
    this.items = [...this.items, { id, kind, title, body: text, action }].slice(-5);
    this.#arm(id);
  }

  /** Une réussite s'efface seule ; un avertissement plus lentement ; une erreur reste jusqu'au clic.
   *  Une notification qui propose une action laisse le temps de la lire et de cliquer. */
  #arm(id: number) {
    const toast = this.items.find((t) => t.id === id);
    if (!toast || toast.kind === 'err') return;
    const delay = toast.kind === 'ok' && !toast.action ? 4500 : 9000;
    this.#timers.set(
      id,
      window.setTimeout(() => this.dismiss(id), delay),
    );
  }

  /** La souris ou le focus est sur la notification : elle reste tant qu'on y est. */
  hold(id: number) {
    window.clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
  }

  /** On l'a quittée : son délai repart de zéro. */
  release(id: number) {
    if (!this.#timers.has(id)) this.#arm(id);
  }

  ok(title: string, body = '', action?: ToastAction) {
    this.push('ok', title, body, action);
  }
  warn(title: string, body = '') {
    this.push('warn', title, body);
  }
  err(title: string, body = '') {
    this.push('err', title, body);
  }

  dismiss(id: number) {
    this.hold(id);
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new Toasts();
