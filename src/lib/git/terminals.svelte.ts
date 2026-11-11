// Terminaux d'un dépôt : des terminaux locaux ouverts dans son dossier, sous le graphe
// (charte, section 9.20). Chaque dépôt a les siens ; ils vivent tant que son onglet est ouvert.

import { localSession, type Session } from '../ssh/ssh.svelte';

export class RepoTerminals {
  sessions = $state<Session[]>([]);
  activeKey = $state(0);
  /** Le panneau est affiché sous le graphe. */
  shown = $state(false);
  /** Compte les demandes de clavier : chacune donne le focus au terminal actif. */
  focusAsk = $state(0);

  active = $derived(this.sessions.find((s) => s.key === this.activeKey) ?? null);
  /** Shells qui tournent encore : la pastille du bouton « Terminal » les compte, panneau masqué. */
  running = $derived(this.sessions.filter((s) => s.status !== 'closed').length);

  #add(cwd: string) {
    const session = localSession(cwd, this.sessions);
    this.sessions.push(session);
    this.activeKey = session.key;
    this.shown = true;
  }

  /** « + », ou la palette : un terminal de plus dans `cwd`, qui prend le clavier. */
  open(cwd: string) {
    this.#add(cwd);
    this.focusAsk++;
  }

  /** Affiche le panneau, sans lui donner le clavier ; s'il n'a pas de terminal, en ouvre un. */
  show(cwd: string) {
    if (!this.sessions.length) this.#add(cwd);
    this.shown = true;
  }

  hide() {
    this.shown = false;
  }

  /** Le bouton et la palette : afficher ou masquer, sans plus. */
  toggle(cwd: string) {
    if (this.shown) this.hide();
    else this.show(cwd);
  }

  /** Le raccourci : affiche et donne le clavier, ou le donne, ou masque si le clavier y est déjà. */
  key(cwd: string, focusInside: boolean) {
    if (this.shown && focusInside) return this.hide();
    this.show(cwd);
    this.focusAsk++;
  }

  activate(key: number) {
    this.activeKey = key;
  }

  /** Ferme un onglet ; fermer le dernier masque le panneau, qui n'a pas d'état vide. */
  close(key: number) {
    const at = this.sessions.findIndex((s) => s.key === key);
    if (at < 0) return;
    this.sessions.splice(at, 1);
    if (this.activeKey === key) this.activeKey = this.sessions[Math.min(at, this.sessions.length - 1)]?.key ?? 0;
    if (!this.sessions.length) this.shown = false;
  }
}
