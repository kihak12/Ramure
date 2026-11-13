// Short-lived notifications, shown at the bottom, in the center of the window.

export type ToastKind = 'ok' | 'warn' | 'err';

/** The action a notification can offer: undoing what was just done. */
export interface ToastAction {
  label: string;
  run: () => void;
  /** What it undoes was just run from the keyboard: the focus goes to its button. */
  focus?: boolean;
}

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  body: string;
  action?: ToastAction;
}

/** The wordy outputs of git (push, fetch) are cut down to their last lines. */
const MAX_LINES = 8;

class Toasts {
  items = $state<Toast[]>([]);
  #next = 1;
  /** Timers of the notifications that go away on their own. */
  #timers = new Map<number, number>();

  push(kind: ToastKind, title: string, body = '', action?: ToastAction) {
    const id = this.#next++;
    const lines = body.trim().split('\n');
    const text = kind === 'ok' ? lines.slice(-MAX_LINES).join('\n') : lines.join('\n');
    this.items = [...this.items, { id, kind, title, body: text, action }].slice(-5);
    this.#arm(id);
  }

  /** A success goes away on its own; a warning more slowly; an error stays until it is clicked.
   *  A notification that offers an action leaves the time to read it and to click. */
  #arm(id: number) {
    const toast = this.items.find((t) => t.id === id);
    if (!toast || toast.kind === 'err') return;
    const delay = toast.kind === 'ok' && !toast.action ? 4500 : 9000;
    this.#timers.set(
      id,
      window.setTimeout(() => this.dismiss(id), delay),
    );
  }

  /** The mouse or the focus is on the notification: it stays for as long as they are there. */
  hold(id: number) {
    window.clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
  }

  /** They have left it: its delay starts again from zero. */
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
