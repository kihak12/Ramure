// Git workspace: the open tabs (one repository each) and the recent repositories.
// The tabs are reopened at startup.

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { mayCloseRepo } from '../ssh/guard';
import { toasts } from '../ui/toasts.svelte';
import { Repo } from './repo.svelte';
import type { CommandLog, Done } from './types';

const STORAGE_TABS = 'ramure.tabs';
const STORAGE_RECENTS = 'ramure.recents';
/** Former key, from the time when Ramure opened only one repository. */
const STORAGE_LEGACY = 'ramure.repo';
const MAX_RECENTS = 12;

export interface Recent {
  path: string;
  name: string;
  /** Last opening, in Unix milliseconds. */
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
    /* storage unavailable: nothing is remembered for next time */
  }
}

/** Last element of a path, with either separator. */
export const basename = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? path;

class Workspace {
  tabs = $state.raw<Tab[]>([]);
  activeId = $state(0);
  recents = $state.raw<Recent[]>(read<Recent[]>(STORAGE_RECENTS, []));
  /** True while the clone dialog is open. */
  cloning = $state(false);

  active = $derived(this.tabs.find((tab) => tab.id === this.activeId) ?? null);

  #next = 1;
  #restoring = true;

  constructor() {
    let legacy = '';
    try {
      legacy = localStorage.getItem(STORAGE_LEGACY) ?? '';
    } catch {
      /* storage unavailable */
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

  /** Title of a tab: the name of the repository, or that of the folder while it loads. */
  title(tab: Tab) {
    return tab.repo.result?.name ?? (tab.repo.path ? basename(tab.repo.path) : 'New tab');
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

  /** Goes to the next tab (`step = 1`) or the previous one (`step = -1`). */
  cycle(step: 1 | -1) {
    const at = this.tabs.findIndex((tab) => tab.id === this.activeId);
    if (at < 0 || this.tabs.length < 2) return;
    this.activate(this.tabs[(at + step + this.tabs.length) % this.tabs.length].id);
  }

  /** Closes a tab. If a program runs in the terminal of the repository, asks first. */
  async closeTab(id: number) {
    const closing = this.tabs.find((tab) => tab.id === id);
    if (!closing || !(await mayCloseRepo(this.title(closing), closing.repo.terminals.sessions))) return;
    // The question may have taken a while: the tab is looked up again.
    const at = this.tabs.findIndex((tab) => tab.id === id);
    if (at < 0) return;
    const rest = this.tabs.filter((tab) => tab.id !== id);
    // There is always a tab left: closing the last one leaves room for the start page.
    if (!rest.length) rest.push(this.#create());
    this.tabs = rest;
    if (this.activeId === id) this.activeId = rest[Math.min(at, rest.length - 1)].id;
    this.#persist();
  }

  /**
   * Opens `path`: in the tab that already shows this repository, else in the current tab if it
   * is empty, else in a new tab. Returns the tab used.
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

  /** Asks for a folder, then opens it. */
  async pick() {
    const dir = await open({ directory: true, multiple: false, title: 'Open Git repository' });
    if (typeof dir === 'string') await this.openPath(dir);
  }

  /** Asks for a folder, initializes a repository in it, then opens it. */
  async init() {
    const dir = await open({ directory: true, multiple: false, title: 'Choose the folder of the new repository' });
    if (typeof dir !== 'string') return;
    try {
      const reply = await invoke<Done<string>>('git_init', { path: dir });
      const tab = await this.openPath(reply.data);
      tab.repo.record(reply.commands);
      toasts.ok('Repository initialized', dir);
    } catch (e) {
      const failed = e as { message?: string };
      toasts.err('Could not initialize repository', failed.message ?? String(e));
    }
  }

  /** Opens the repository just cloned and adds the command to its command log. */
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
