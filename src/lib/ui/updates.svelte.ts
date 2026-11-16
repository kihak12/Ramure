// In-app updates (guidelines, section 9.23): what the core knows of a newer version, and the two
// steps of its install. The settings show this state; the rail shows that a version waits.

import { Channel, invoke } from '@tauri-apps/api/core';
import { prefs } from '../git/prefs.svelte';

/** How this copy was installed. `null`: built from the sources. */
export type Installer = 'deb' | 'appimage' | 'nsis';

interface Info {
  version: string;
  canUpdate: boolean;
  installer: Installer | null;
}

/** A newer version, as its release announces it. */
export interface Available {
  version: string;
  /** In milliseconds since the epoch, when the release says it. */
  published: number | null;
}

/** Why a step failed: what happened and what to do, then the reason of the system as it is. */
export interface Failure {
  message: string;
  reason: string | null;
}

/** The last check: when, whether it was asked for, and why it failed. */
interface Checked {
  at: number;
  manual: boolean;
  failure: Failure | null;
}

/** Where the version found stands. `idle`: none was found. */
export type Phase = 'idle' | 'available' | 'downloading' | 'verifying' | 'ready' | 'installing';

const DAY = 24 * 60 * 60 * 1000;
/** After the start, once the window is up: the check never slows the opening of a repository. */
const FIRST_CHECK = 10_000;

/** What the core answers when a command fails: a failure, or the text of an unexpected error. */
const failure = (e: unknown, fallback: string): Failure =>
  typeof e === 'object' && e !== null && 'message' in e ? (e as Failure) : { message: fallback, reason: typeof e === 'string' ? e : String(e) };

class Updates {
  info = $state<Info | null>(null);
  phase = $state<Phase>('idle');
  /** A check is running. */
  checking = $state(false);
  found = $state<Available | null>(null);
  checked = $state<Checked | null>(null);
  /** The download: bytes received, and the size when the server gives it. */
  progress = $state<{ done: number; total: number | null }>({ done: 0, total: null });
  /** What kept the download or the install from ending, said in the box. */
  failed = $state<Failure | null>(null);

  /** A version waits, downloaded or not: the dot of the rail and of "General". */
  waiting = $derived(this.found !== null);

  #timer: number | undefined;

  /** At startup: what this copy is, then the automatic check, at once and once a day. */
  async start() {
    try {
      this.info = await invoke<Info>('update_info');
    } catch {
      // The core does not answer: no version to show, and nothing to update.
      this.info = null;
    }
    window.setTimeout(() => void this.#auto(), FIRST_CHECK);
    this.#timer = window.setInterval(() => void this.#auto(), DAY);
  }

  stop() {
    window.clearInterval(this.#timer);
  }

  /** The check that runs on its own: only when the setting is on, and never over a step in progress. */
  async #auto() {
    if (!prefs.updateCheck || !this.info?.canUpdate) return;
    if (this.phase !== 'idle' && this.phase !== 'available') return;
    await this.check(false);
  }

  /** Asks GitHub whether a newer version exists. `manual`: "Check for updates" was clicked. */
  async check(manual: boolean) {
    if (!this.info?.canUpdate || this.checking) return;
    // A version that is downloading, ready or installing is not looked for again.
    if (this.phase !== 'idle' && this.phase !== 'available') return;
    this.checking = true;
    try {
      const found = await invoke<Available | null>('update_check');
      // A newer version than the one that waited takes its place, and its failure goes with it.
      if (found?.version !== this.found?.version) this.failed = null;
      this.found = found;
      this.checked = { at: Date.now(), manual, failure: null };
      this.phase = found ? 'available' : 'idle';
    } catch (e) {
      this.checked = { at: Date.now(), manual, failure: failure(e, 'Ramure could not check for updates. Try again.') };
    } finally {
      this.checking = false;
    }
  }

  /** "Download update": downloads and verifies, while the user works. */
  async download() {
    if (this.phase !== 'available') return;
    this.phase = 'downloading';
    this.failed = null;
    this.progress = { done: 0, total: null };
    const onProgress = new Channel<{ done: number; total: number | null; verifying: boolean }>();
    onProgress.onmessage = ({ done, total, verifying }) => {
      this.progress = { done, total };
      if (verifying && this.phase === 'downloading') this.phase = 'verifying';
    };
    try {
      const done = await invoke<boolean>('update_download', { onProgress });
      this.phase = done ? 'ready' : 'available';
    } catch (e) {
      this.failed = failure(e, 'The download failed. Check your connection, then try again.');
      this.phase = 'available';
    }
  }

  /** "Cancel": the download stops; the version is still available. */
  async cancel() {
    if (this.phase !== 'downloading') return;
    await invoke('update_cancel').catch(() => {});
  }

  /** Installs and restarts. Comes back only on a failure: the download is kept. */
  async install() {
    if (this.phase !== 'ready') return;
    this.phase = 'installing';
    this.failed = null;
    try {
      await invoke('update_install');
    } catch (e) {
      this.failed = failure(e, 'The update was not installed. Ramure is unchanged.');
      this.phase = 'ready';
    }
  }
}

export const updates = new Updates();
