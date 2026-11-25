// Ramure's settings, shared by every open repository. They are changed on the settings page
// (src/lib/settings/) and apply at once.

const STORAGE_KEY = 'ramure.settings';

export type PullMode = 'ff-only' | 'rebase' | 'merge';
/** `ribbon`: author dots and colored ribbons. `classic`: points and curves, more compact. */
export type GraphStyle = 'ribbon' | 'classic';
export type DiffMode = 'unified' | 'split';
/** `system`: light or dark, as the system says. */
export type ThemeChoice = 'system' | 'light' | 'dark';
/** Color theme (guidelines, section 2.5). */
export type Palette = 'slate' | 'ink' | 'undergrowth' | 'ember';
/** The order of a date, and the clock (guidelines, 9.18): see `src/lib/ui/dates.ts`. */
export type DateFormat = 'mdy' | 'dmy' | 'ymd' | 'text';
export type Clock = '24' | '12';
/** A file whose extension or name is `match` opens in `program` (guidelines, 9.18, "Files"). */
export interface OpenWith {
  match: string;
  program: string;
}
/** The names of the color themes before the interface moved to English (#147): a setting saved
 *  under one of them is read under its new name. */
const FORMER_PALETTES: Readonly<Record<string, Palette>> = { ardoise: 'slate', encre: 'ink', 'sous-bois': 'undergrowth', braise: 'ember' };

export interface Settings {
  /** Commits loaded; 0 for the whole history. */
  limit: number;
  /** Automatic fetch, in minutes; 0 to turn it off. */
  autoFetch: number;
  pullMode: PullMode;
  graphStyle: GraphStyle;
  /** Diff in one column, or old and new side by side. */
  diffMode: DiffMode;
  theme: ThemeChoice;
  palette: Palette;
  /** A lost SSH connection opens again on its own (guidelines, section 9.15). */
  sshReconnect: boolean;
  /** The folder of session logs; empty for the default one (guidelines, 9.15, 9.18). */
  journalDir: string;
  /** Ramure asks GitHub at startup, then once a day, whether a newer version exists (guidelines, 9.23). */
  updateCheck: boolean;
  /** How every date of the application is written, and its time (guidelines, 9.18 and 12). */
  dateFormat: DateFormat;
  clock: Clock;
  /** The program of the text files, empty for that of the system; the exceptions, in their order
   *  (guidelines, 9.18 and 9.24). */
  textEditor: string;
  openWith: OpenWith[];
}

export const DEFAULTS: Readonly<Settings> = { limit: 10000, autoFetch: 5, pullMode: 'ff-only', graphStyle: 'ribbon', diffMode: 'unified', theme: 'system', palette: 'slate', sshReconnect: true, journalDir: '', updateCheck: true, dateFormat: 'mdy', clock: '24', textEditor: '', openWith: [] };

/** Bounds of the automatic fetch, in minutes. */
export const AUTO_FETCH = { min: 0, max: 60 } as const;

function read(): Settings {
  try {
    const saved: Settings = { ...DEFAULTS, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') };
    saved.palette = FORMER_PALETTES[saved.palette] ?? saved.palette;
    return saved;
  } catch {
    return { ...DEFAULTS };
  }
}

export const prefs = $state<Settings>(read());

/** Saves one or more settings. */
export function savePrefs(next: Partial<Settings>) {
  Object.assign(prefs, next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    /* storage unavailable: the settings hold for this session */
  }
}
