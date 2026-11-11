// État du module SSH : carnet d'hôtes, clés, et terminaux ouverts, distants ou locaux.

import { invoke } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import { dialogs } from '../ui/dialogs.svelte';
import { toasts } from '../ui/toasts.svelte';
import { secretsHeld } from './secrets';
import type { Host, KeyInfo } from './types';

export interface Session {
  /** Identifiant de l'onglet (différent de celui de la connexion côté Rust). */
  key: number;
  /** `ssh` : une session sur un hôte du carnet. `local` : le shell de cette machine. */
  kind: 'ssh' | 'local';
  /** L'hôte d'une session SSH ; pour un terminal local, une fiche vide qui ne sert pas. */
  host: Host;
  /** Dossier de départ d'un terminal local ; le dossier personnel s'il est vide. */
  cwd: string;
  title: string;
  status: 'connecting' | 'reconnecting' | 'connected' | 'closed';
  /** La barre de recherche du terminal est ouverte. */
  searching: boolean;
  /** Compte les demandes d'ouverture : chacune rend le focus au champ, même barre ouverte. */
  searchAsk: number;
  /** Ce que le cœur dit d'un terminal local ouvert : son shell et son dossier de départ. */
  opened?: { shell: string; cwd: string };
  /** L'identifiant du terminal local côté cœur, tant que son shell tourne. */
  localId?: number;
}

/** L'état d'une session en mots : le point d'un onglet ou d'un en-tête ne le dit jamais par sa seule couleur. */
export const STATUS: Record<Session['status'], string> = {
  connecting: 'Connexion en cours',
  reconnecting: 'Reconnexion en cours',
  connected: 'Connecté',
  closed: 'Session terminée',
};

/** La fiche d'hôte d'un terminal local : il n'en a pas, celle-ci ne sert qu'à remplir le champ. */
const LOCAL_HOST: Host = {
  id: '',
  alias: '',
  hostname: '',
  port: 0,
  user: '',
  auth: 'auto',
  keyPath: '',
  group: '',
  tags: [],
  color: '',
  startupCommand: '',
  proxyJump: false,
  updatedAt: 0,
};

/** Identifiants des onglets de terminal, communs au module SSH et aux terminaux des dépôts. */
let nextKey = 1;

/**
 * Un terminal local qui démarre dans `cwd` (le dossier personnel s'il est vide), titré
 * « Terminal », puis « Terminal 2 »… le plus petit numéro libre parmi `siblings`.
 */
export function localSession(cwd: string, siblings: Session[]): Session {
  const taken = new Set(siblings.filter((s) => s.kind === 'local').map((s) => s.title));
  let n = 1;
  while (taken.has(n === 1 ? 'Terminal' : `Terminal ${n}`)) n++;
  return {
    key: nextKey++,
    kind: 'local',
    cwd,
    host: LOCAL_HOST,
    title: n === 1 ? 'Terminal' : `Terminal ${n}`,
    status: 'connecting',
    searching: false,
    searchAsk: 0,
  };
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

  async load() {
    try {
      this.hosts = await invoke<Host[]>('hosts_list');
    } catch (e) {
      toasts.err('Le carnet d’hôtes n’a pas pu être lu', message(e));
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

  /** La fiche enregistrée, ou ce qui a empêché de l'enregistrer : la fiche le dit elle-même, une
   *  notification restant sous son voile (charte, 9.11). */
  async save(host: Host): Promise<Host | string> {
    try {
      const saved = await invoke<Host>('host_save', { host });
      this.hosts = await invoke<Host[]>('hosts_list');
      return saved;
    } catch (e) {
      return `L’hôte n’a pas pu être enregistré : ${message(e)}`;
    }
  }

  /** Vrai si la fiche a été supprimée. `failed` : à qui dire l'échec, à la place d'une notification. */
  async remove(host: Host, failed?: (text: string) => void): Promise<boolean> {
    try {
      await invoke('host_delete', { id: host.id });
      this.hosts = this.hosts.filter((h) => h.id !== host.id);
      toasts.ok('Hôte supprimé', host.alias);
      return true;
    } catch (e) {
      if (failed) failed(`L’hôte n’a pas pu être supprimé : ${message(e)}`);
      else toasts.err('L’hôte n’a pas pu être supprimé', message(e));
      return false;
    }
  }

  /** Le compte d'un hôte sur son serveur : c'est à lui qu'appartient un mot de passe retenu. */
  #account(host: Host) {
    return `${host.user || this.defaultUser}@${host.hostname}:${host.port || 22}`;
  }

  /**
   * Demande confirmation, puis supprime la fiche : du clic droit sur sa ligne, ou de la fiche
   * elle-même. Vrai si elle a été supprimée.
   */
  async removeAsked(host: Host, failed?: (text: string) => void): Promise<boolean> {
    // Le mot de passe retenu pour ce compte part avec sa dernière fiche : la confirmation le dit.
    const last = !this.hosts.some((other) => other.id !== host.id && this.#account(other) === this.#account(host));
    const [kept] =
      host.auth === 'password' && last
        ? await secretsHeld([{ kind: 'password', user: host.user || this.defaultUser, host: host.hostname, port: host.port || 22 }])
        : [false];
    const confirmed = await dialogs.confirm({
      title: `Supprimer ${host.alias} du carnet ?`,
      body: `${
        kept
          ? 'La fiche est supprimée, et le mot de passe que le trousseau du système retient pour ce compte est oublié : ni l’un ni l’autre ne pourra être récupéré.'
          : 'La fiche est supprimée et ne pourra pas être récupérée.'
      } Le serveur, ses clés et ~/.ssh/config ne sont pas touchés.`,
      confirmLabel: 'Supprimer',
      danger: true,
    });
    return confirmed && (await this.remove(host, failed));
  }

  /**
   * Après une suppression, la ligne de l'hôte est partie, et avec elle ce qui avait le focus : il va
   * au filtre du carnet, qui est toujours là, même carnet vide (charte, section 9.21). À appeler
   * une fois les boîtes fermées : en se fermant, une boîte rend le focus à ce qui l'avait avant elle.
   */
  async focusBook() {
    await tick();
    document.querySelector<HTMLElement>('.book input[type="search"]')?.focus();
  }

  async importConfig() {
    try {
      const report = await invoke<{ added: number; skipped: number }>('hosts_import');
      this.hosts = await invoke<Host[]>('hosts_list');
      const plural = (n: number) => (n > 1 ? 's' : '');
      const added = report.added ? `${report.added} hôte${plural(report.added)} importé${plural(report.added)}` : 'Aucun nouvel hôte';
      toasts.ok(added, report.skipped ? `${report.skipped} déjà présent${plural(report.skipped)} dans le carnet.` : '');
    } catch (e) {
      toasts.err('L’import de ~/.ssh/config a échoué', message(e));
    }
  }

  /** Ouvre un terminal sur `host` dans un nouvel onglet. */
  open(host: Host) {
    const user = host.user || this.defaultUser;
    const session: Session = {
      key: nextKey++,
      kind: 'ssh',
      cwd: '',
      host: { ...host, user },
      title: host.alias || `${user}@${host.hostname}`,
      status: 'connecting',
      searching: false,
      searchAsk: 0,
    };
    this.sessions.push(session);
    this.activeKey = session.key;
  }

  /** Ouvre un terminal local dans un nouvel onglet : « Terminal », puis « Terminal 2 »… le plus petit numéro libre. */
  openLocal(cwd = '') {
    const session = localSession(cwd, this.sessions);
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
