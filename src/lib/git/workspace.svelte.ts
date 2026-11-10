// Espace de travail Git : les onglets ouverts (un dépôt chacun) et les dépôts récents.
// Les onglets sont rouverts au démarrage.

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { toasts } from '../ui/toasts.svelte';
import { Repo } from './repo.svelte';
import type { CommandLog, Done } from './types';

const STORAGE_TABS = 'ramure.tabs';
const STORAGE_RECENTS = 'ramure.recents';
/** Ancienne clé, du temps où Ramure n'ouvrait qu'un dépôt. */
const STORAGE_LEGACY = 'ramure.repo';
const MAX_RECENTS = 12;

export interface Recent {
  path: string;
  name: string;
  /** Dernière ouverture, en millisecondes Unix. */
  at: number;
}

export interface Tab {
  id: number;
  repo: Repo;
}

interface SavedTabs {
  paths: string[];
  active: number;
}

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* stockage indisponible : rien n'est mémorisé pour la prochaine fois */
  }
}

/** Dernier élément d'un chemin, avec l'un ou l'autre séparateur. */
export const basename = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? path;

class Workspace {
  tabs = $state.raw<Tab[]>([]);
  activeId = $state(0);
  recents = $state.raw<Recent[]>(read<Recent[]>(STORAGE_RECENTS, []));
  /** Vrai tant que la boîte de clonage est ouverte. */
  cloning = $state(false);

  active = $derived(this.tabs.find((tab) => tab.id === this.activeId) ?? null);

  #next = 1;
  #restoring = true;

  constructor() {
    let legacy = '';
    try {
      legacy = localStorage.getItem(STORAGE_LEGACY) ?? '';
    } catch {
      /* stockage indisponible */
    }
    const saved = read<SavedTabs>(STORAGE_TABS, { paths: legacy ? [legacy] : [], active: 0 });
    const tabs = saved.paths.map((path) => this.#create(path));
    if (!tabs.length) tabs.push(this.#create());
    this.tabs = tabs;
    this.activeId = (tabs[saved.active] ?? tabs[0]).id;
    this.#restoring = false;
    this.#persist();
  }

  #create(path = ''): Tab {
    const repo = new Repo();
    repo.onopened = (opened) => this.#opened(opened);
    if (path) void repo.open(path);
    return { id: this.#next++, repo };
  }

  #persist() {
    if (this.#restoring) return;
    const open = this.tabs.filter((tab) => tab.repo.path);
    const active = this.active;
    write(STORAGE_TABS, {
      paths: open.map((tab) => tab.repo.path),
      active: Math.max(0, active ? open.indexOf(active) : 0),
    } satisfies SavedTabs);
  }

  #opened(repo: Repo) {
    const path = repo.result?.repo;
    if (!path) return;
    const entry: Recent = { path, name: repo.result?.name ?? basename(path), at: Date.now() };
    this.recents = [entry, ...this.recents.filter((r) => r.path !== path)].slice(0, MAX_RECENTS);
    write(STORAGE_RECENTS, this.recents);
    this.#persist();
  }

  /** Titre d'un onglet : le nom du dépôt, ou celui du dossier pendant le chargement. */
  title(tab: Tab) {
    return tab.repo.result?.name ?? (tab.repo.path ? basename(tab.repo.path) : 'Nouvel onglet');
  }

  newTab() {
    const tab = this.#create();
    this.tabs = [...this.tabs, tab];
    this.activeId = tab.id;
    this.#persist();
    return tab;
  }

  activate(id: number) {
    this.activeId = id;
    this.#persist();
  }

  /** Passe à l'onglet suivant (`step = 1`) ou précédent (`step = -1`). */
  cycle(step: 1 | -1) {
    const at = this.tabs.findIndex((tab) => tab.id === this.activeId);
    if (at < 0 || this.tabs.length < 2) return;
    this.activate(this.tabs[(at + step + this.tabs.length) % this.tabs.length].id);
  }

  closeTab(id: number) {
    const at = this.tabs.findIndex((tab) => tab.id === id);
    if (at < 0) return;
    const rest = this.tabs.filter((tab) => tab.id !== id);
    // Il reste toujours un onglet : le dernier fermé laisse place à la page d'accueil.
    if (!rest.length) rest.push(this.#create());
    this.tabs = rest;
    if (this.activeId === id) this.activeId = rest[Math.min(at, rest.length - 1)].id;
    this.#persist();
  }

  /**
   * Ouvre `path` : dans l'onglet qui montre déjà ce dépôt, sinon dans l'onglet courant
   * s'il est vide, sinon dans un nouvel onglet. Renvoie l'onglet utilisé.
   */
  async openPath(path: string): Promise<Tab> {
    const existing = this.tabs.find((tab) => tab.repo.path === path);
    if (existing) {
      this.activate(existing.id);
      return existing;
    }
    const current = this.active;
    const tab = current && !current.repo.path && !current.repo.loading ? current : this.newTab();
    this.activeId = tab.id;
    await tab.repo.open(path);
    this.#persist();
    return tab;
  }

  /** Demande un dossier, puis l'ouvre. */
  async pick() {
    const dir = await open({ directory: true, multiple: false, title: 'Ouvrir un dépôt Git' });
    if (typeof dir === 'string') await this.openPath(dir);
  }

  /** Demande un dossier, y initialise un dépôt, puis l'ouvre. */
  async init() {
    const dir = await open({ directory: true, multiple: false, title: 'Choisir le dossier du nouveau dépôt' });
    if (typeof dir !== 'string') return;
    try {
      const reply = await invoke<Done<string>>('git_init', { path: dir });
      const tab = await this.openPath(reply.data);
      tab.repo.record(reply.commands);
      toasts.ok('Dépôt initialisé', dir);
    } catch (e) {
      const failed = e as { message?: string };
      toasts.err("L'initialisation a échoué", failed.message ?? String(e));
    }
  }

  /** Ouvre le dépôt qui vient d'être cloné et verse la commande à son journal. */
  async openCloned(path: string, commands: CommandLog[]) {
    const tab = await this.openPath(path);
    tab.repo.record(commands);
  }

  forget(path: string) {
    this.recents = this.recents.filter((r) => r.path !== path);
    write(STORAGE_RECENTS, this.recents);
  }
}

export const workspace = new Workspace();
