// Mirror of the structures of the Files module (src-tauri/src/files).

import { dateTime, fullDateTime } from '../ui/dates';

/** Where a command acts: this machine, or an SFTP connection. */
export type Place = { kind: 'local' } | { kind: 'remote'; id: number };

export interface Entry {
  name: string;
  kind: 'dir' | 'file' | 'link' | 'other';
  /** A folder, or a link that leads to one: it opens like a folder. */
  dir: boolean;
  size: number;
  /** Seconds since the epoch. */
  modified: number | null;
  mode: number | null;
  hidden: boolean;
}

export interface Listing {
  path: string;
  parent: string | null;
  entries: Entry[];
}

export type FsError = { kind: 'lost' } | { kind: 'canceled' } | { kind: 'other'; message: string };

export interface SftpOpened {
  id: number;
  home: string;
  knownHost: boolean;
  learnFailed?: string | null;
  authMethod: string;
  connectMs: number;
  keepFailed?: string | null;
}

export interface Progress {
  counting: boolean;
  filesTotal: number;
  filesDone: number;
  bytesTotal: number;
  bytesDone: number;
  current: string;
}

export interface Summary {
  files: number;
  folders: number;
  bytes: number;
  skipped: string[];
  links: string[];
  failed: { path: string; reason: string }[];
}

/** The sentence of an error of the core. */
export function fsMessage(e: unknown): string {
  if (e && typeof e === 'object' && 'kind' in e) {
    const error = e as FsError;
    if (error.kind === 'other') return error.message;
    if (error.kind === 'lost') return 'The connection to the host was lost.';
    return 'Canceled.';
  }
  return typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
}

export const isLost = (e: unknown) => !!e && typeof e === 'object' && 'kind' in e && (e as FsError).kind === 'lost';

const numbers = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });

/** A size in bytes, in kB, MB and GB of 1,000 bytes (guidelines, 9.24): "12.4 MB". */
export function size(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`;
  const units = ['kB', 'MB', 'GB', 'TB'];
  let value = bytes / 1000;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  return `${numbers.format(value)} ${units[unit]}`;
}

/** "18.4 of 60 MB": both in the unit of the total, so that they read together. */
export function sizeOf(done: number, total: number): string {
  if (total < 1000) return `${done} of ${total} B`;
  const units = ['kB', 'MB', 'GB', 'TB'];
  let divisor = 1000;
  let unit = 0;
  while (total / divisor >= 1000 && unit < units.length - 1) {
    divisor *= 1000;
    unit++;
  }
  return `${numbers.format(done / divisor)} of ${numbers.format(total / divisor)} ${units[unit]}`;
}

/**
 * "Nov 9, 2026, 11:35 PM", and its tooltip "Monday, November 9, 2026 at 11:35:12 PM": two versions
 * of the same day are told apart (guidelines, 9.24 and 12).
 */
export const shortDate = (seconds: number | null) => (seconds === null ? '' : dateTime(seconds * 1000));
export const fullDate = (seconds: number | null) => (seconds === null ? '' : fullDateTime(seconds * 1000, true));

/** The separator of a path: "/" on a server and on Linux, "\" on Windows. */
const separator = (path: string) => (path.includes('\\') && !path.includes('/') ? '\\' : '/');

export function joinPath(dir: string, name: string): string {
  const sep = separator(dir);
  return dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;
}

/** The folder above `path`; null at the root ("/", "C:\"). */
export function parentPath(path: string): string | null {
  const sep = separator(path);
  const trimmed = path.length > 1 ? path.replace(/[\\/]+$/, '') : path;
  if (trimmed === '/' || trimmed === '' || /^[A-Za-z]:$/.test(trimmed)) return null;
  const at = trimmed.lastIndexOf(sep);
  if (at < 0) return null;
  if (at === 0) return '/';
  const parent = trimmed.slice(0, at);
  return /^[A-Za-z]:$/.test(parent) ? `${parent}\\` : parent;
}

/** The segments of a path, each with the folder it opens: "/", "var", "www". */
export function segments(path: string): { name: string; path: string }[] {
  const out: { name: string; path: string }[] = [];
  let current: string | null = path;
  while (current) {
    const parent = parentPath(current);
    const name = parent === null ? current : current.slice(parent.length).replace(/^[\\/]/, '');
    out.unshift({ name, path: current });
    current = parent;
  }
  return out;
}

/** The last part of a path. */
export function nameOf(path: string): string {
  const parent = parentPath(path);
  return parent === null ? path : path.slice(parent.length).replace(/^[\\/]/, '');
}
