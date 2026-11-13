// Terminals of a repository: local terminals opened in its folder, under the graph
// (guidelines, section 9.20). Each repository has its own; they live as long as its tab is open.

import { localSession, type Session } from '../ssh/ssh.svelte';

export class RepoTerminals {
  sessions = $state<Session[]>([]);
  activeKey = $state(0);
  /** The panel is shown under the graph. */
  shown = $state(false);
  /**
   * The graph is hidden: the terminal takes the whole center column. Only holds while the
   * terminal is shown; the center is never empty (guidelines, section 9.17).
   */
  alone = $state(false);
  /** Counts the requests for the keyboard: each one gives the focus to the active terminal. */
  focusAsk = $state(0);

  active = $derived(this.sessions.find((s) => s.key === this.activeKey) ?? null);
  /** Shells still running: the badge of the “Terminal” button counts them, panel hidden. */
  running = $derived(this.sessions.filter((s) => s.status !== 'closed').length);

  #add(cwd: string) {
    const session = localSession(cwd, this.sessions);
    this.sessions.push(session);
    this.activeKey = session.key;
    this.shown = true;
  }

  /** “+”, or the palette: one more terminal in `cwd`, which takes the keyboard. */
  open(cwd: string) {
    this.#add(cwd);
    this.focusAsk++;
  }

  /** Shows the panel, without giving it the keyboard; if it has no terminal, opens one. */
  show(cwd: string) {
    if (!this.sessions.length) this.#add(cwd);
    this.shown = true;
  }

  /** Hiding the terminal brings the graph back if it was hidden. */
  hide() {
    this.shown = false;
    this.alone = false;
  }

  /** Hides or shows the graph; with no terminal shown, it stays. */
  setGraph(visible: boolean) {
    this.alone = !visible && this.shown;
  }

  toggleGraph() {
    this.setGraph(this.alone);
  }

  /** The button and the palette: show or hide, nothing more. */
  toggle(cwd: string) {
    if (this.shown) this.hide();
    else this.show(cwd);
  }

  /** The shortcut: shows and gives the keyboard, or gives it, or hides if the keyboard is already there. */
  key(cwd: string, focusInside: boolean) {
    if (this.shown && focusInside) return this.hide();
    this.show(cwd);
    this.focusAsk++;
  }

  activate(key: number) {
    this.activeKey = key;
  }

  /** Closes a tab; closing the last one hides the panel, which has no empty state. */
  close(key: number) {
    const at = this.sessions.findIndex((s) => s.key === key);
    if (at < 0) return;
    this.sessions.splice(at, 1);
    if (this.activeKey === key) this.activeKey = this.sessions[Math.min(at, this.sessions.length - 1)]?.key ?? 0;
    if (!this.sessions.length) this.hide();
  }
}
