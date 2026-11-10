// Notifications éphémères, affichées en bas, au centre de la fenêtre.

export type ToastKind = 'ok' | 'warn' | 'err';

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  body: string;
}

/** Les sorties bavardes de git (push, fetch) sont réduites à leurs dernières lignes. */
const MAX_LINES = 8;

class Toasts {
  items = $state<Toast[]>([]);
  #next = 1;

  push(kind: ToastKind, title: string, body = '') {
    const id = this.#next++;
    const lines = body.trim().split('\n');
    const text = kind === 'ok' ? lines.slice(-MAX_LINES).join('\n') : lines.join('\n');
    this.items = [...this.items, { id, kind, title, body: text }].slice(-5);
    // Une réussite s'efface seule ; un avertissement plus lentement ; une erreur reste jusqu'au clic.
    if (kind !== 'err') window.setTimeout(() => this.dismiss(id), kind === 'ok' ? 4500 : 9000);
  }

  ok(title: string, body = '') {
    this.push('ok', title, body);
  }
  warn(title: string, body = '') {
    this.push('warn', title, body);
  }
  err(title: string, body = '') {
    this.push('err', title, body);
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new Toasts();
