// État du module SSH : carnet d'hôtes, clés, et sessions de terminal ouvertes.

import { invoke } from '@tauri-apps/api/core';
import { toasts } from '../ui/toasts.svelte';
import type { Host, KeyInfo } from './types';

export interface Session {
  /** Identifiant de l'onglet (différent de celui de la connexion côté Rust). */
  key: number;
  host: Host;
  title: string;
  status: 'connecting' | 'connected' | 'closed';
  /** La barre de recherche du terminal est ouverte. */
  searching: boolean;
}

const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e));

class Ssh {
  hosts = $state.raw<Host[]>([]);
  keys = $state.raw<KeyInfo[]>([]);
  /** Utilisateur de la machine, proposé quand un hôte n'en précise pas. */
  defaultUser = $state('');
  loaded = $state(false);

  sessions = $state<Session[]>([]);
  activeKey = $state(0);

  /** Hôte en cours de modification ; la fiche s'ouvre quand il existe. */
  editing = $state.raw<Host | null>(null);
  showKeys = $state(false);

  active = $derived(this.sessions.find((s) => s.key === this.activeKey) ?? null);
  groups = $derived([...new Set(this.hosts.map((h) => h.group).filter(Boolean))]);

  #next = 1;

  async load() {
    try {
      this.hosts = await invoke<Host[]>('hosts_list');
    } catch (e) {
      toasts.err("Le carnet d'hôtes n'a pas pu être lu", message(e));
    }
    try {
      this.defaultUser = (await invoke<{ user: string }>('ssh_defaults')).user;
    } catch {
      /* l'utilisateur sera saisi dans la fiche */
    }
    await this.loadKeys();
    this.loaded = true;
  }

  async loadKeys() {
    try {
      this.keys = await invoke<KeyInfo[]>('ssh_keys_list');
    } catch {
      this.keys = [];
    }
  }

  async save(host: Host): Promise<Host | null> {
    try {
      const saved = await invoke<Host>('host_save', { host });
      this.hosts = await invoke<Host[]>('hosts_list');
      return saved;
    } catch (e) {
      toasts.err("L'hôte n'a pas pu être enregistré", message(e));
      return null;
    }
  }

  async remove(host: Host) {
    try {
      await invoke('host_delete', { id: host.id });
      this.hosts = this.hosts.filter((h) => h.id !== host.id);
      toasts.ok('Hôte supprimé', host.alias);
    } catch (e) {
      toasts.err("L'hôte n'a pas pu être supprimé", message(e));
    }
  }

  async importConfig() {
    try {
      const report = await invoke<{ added: number; skipped: number }>('hosts_import');
      this.hosts = await invoke<Host[]>('hosts_list');
      const plural = (n: number) => (n > 1 ? 's' : '');
      const added = report.added ? `${report.added} hôte${plural(report.added)} importé${plural(report.added)}` : 'Aucun nouvel hôte';
      toasts.ok(added, report.skipped ? `${report.skipped} déjà présent${plural(report.skipped)} dans le carnet.` : '');
    } catch (e) {
      toasts.err("L'import de ~/.ssh/config a échoué", message(e));
    }
  }

  /** Ouvre un terminal sur `host` dans un nouvel onglet. */
  open(host: Host) {
    const user = host.user || this.defaultUser;
    const session: Session = {
      key: this.#next++,
      host: { ...host, user },
      title: host.alias || `${user}@${host.hostname}`,
      status: 'connecting',
      searching: false,
    };
    this.sessions.push(session);
    this.activeKey = session.key;
  }

  close(key: number) {
    const at = this.sessions.findIndex((s) => s.key === key);
    if (at < 0) return;
    this.sessions.splice(at, 1);
    if (this.activeKey === key) this.activeKey = this.sessions[Math.min(at, this.sessions.length - 1)]?.key ?? 0;
  }
}

export const ssh = new Ssh();
