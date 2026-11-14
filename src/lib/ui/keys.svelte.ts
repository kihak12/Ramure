// Configurable keyboard shortcuts. Each shortcut triggers a command of the palette, named by its
// id; the user can replace it or remove it.

const STORAGE_KEY = 'ramure.keys';

/** Module where a shortcut acts; without a module, it acts everywhere (guidelines, section 11). */
export type Scope = 'git' | 'ssh';

export interface Shortcut {
  /** Id of the command triggered (the same as in the palette). */
  id: string;
  title: string;
  group: string;
  /** Default shortcut, such as `Ctrl+Shift+F`; empty when there is none. */
  keys: string;
  scope?: Scope;
}

/** Two shortcuts get in each other's way if they can act in the same place. */
const overlap = (a: Shortcut, b: Shortcut) => !a.scope || !b.scope || a.scope === b.scope;

/**
 * Actions a shortcut can be given to, with their default shortcut. Each title says the same as
 * the palette command it triggers; a command whose title follows a state says both.
 */
export const SHORTCUTS: Shortcut[] = [
  { id: 'palette', group: 'General', title: 'Command palette', keys: 'Ctrl+K' },
  { id: 'view-git', group: 'General', title: 'Show repositories', keys: 'Ctrl+Shift+G' },
  { id: 'view-ssh', group: 'General', title: 'Show SSH terminal', keys: 'Ctrl+Shift+T' },
  { id: 'settings', group: 'General', title: 'Settings', keys: 'Ctrl+,' },
  { id: 'shortcuts', group: 'General', title: 'Keyboard shortcuts', keys: '' },
  { id: 'terminal-new', group: 'General', title: 'New local terminal', keys: 'Ctrl+Shift+N' },
  { id: 'toggle-sidebar', group: 'View', title: 'Show or hide branch list', keys: 'Ctrl+Shift+B', scope: 'git' },
  { id: 'toggle-detail', group: 'View', title: 'Show or hide detail panel', keys: 'Ctrl+Shift+E', scope: 'git' },
  { id: 'toggle-graph', group: 'View', title: 'Show or hide commit graph', keys: '', scope: 'git' },
  { id: 'toggle-sides', group: 'View', title: 'Show or hide side panels', keys: '', scope: 'git' },
  { id: 'toggle-terminal', group: 'View', title: 'Show or hide terminal', keys: 'Ctrl+Shift+J', scope: 'git' },
  { id: 'terminal-repo', group: 'View', title: 'New terminal in this repository', keys: '', scope: 'git' },
  { id: 'terminal-search', group: 'Terminal', title: 'Search the terminal', keys: 'Ctrl+Shift+F', scope: 'ssh' },
  { id: 'terminal-journal', group: 'Terminal', title: 'Record or stop terminal log', keys: '', scope: 'ssh' },
  { id: 'terminal-tunnels', group: 'Terminal', title: 'Show or hide tunnels', keys: '', scope: 'ssh' },
  { id: 'tab-new', group: 'Tabs', title: 'New tab', keys: 'Ctrl+T', scope: 'git' },
  { id: 'tab-close', group: 'Tabs', title: 'Close tab', keys: 'Ctrl+W', scope: 'git' },
  { id: 'tab-next', group: 'Tabs', title: 'Next tab', keys: 'Ctrl+Tab', scope: 'git' },
  { id: 'tab-prev', group: 'Tabs', title: 'Previous tab', keys: 'Ctrl+Shift+Tab', scope: 'git' },
  { id: 'open', group: 'Repository', title: 'Open repository…', keys: 'Ctrl+O', scope: 'git' },
  { id: 'clone', group: 'Repository', title: 'Clone repository…', keys: '', scope: 'git' },
  { id: 'init', group: 'Repository', title: 'Initialize repository…', keys: '', scope: 'git' },
  { id: 'reload', group: 'Repository', title: 'Reload repository', keys: 'F5', scope: 'git' },
  { id: 'fetch', group: 'Sync', title: 'Fetch', keys: 'Ctrl+Shift+F', scope: 'git' },
  { id: 'pull', group: 'Sync', title: 'Pull', keys: 'Ctrl+Shift+D', scope: 'git' },
  { id: 'push', group: 'Sync', title: 'Push', keys: 'Ctrl+Shift+U', scope: 'git' },
  { id: 'push-force', group: 'Sync', title: 'Force push…', keys: '', scope: 'git' },
  { id: 'branch', group: 'Work in progress', title: 'Create branch…', keys: 'Ctrl+B', scope: 'git' },
  { id: 'wip', group: 'Work in progress', title: 'Show working changes', keys: 'Ctrl+Shift+M', scope: 'git' },
  { id: 'stage-all', group: 'Work in progress', title: 'Stage all', keys: 'Ctrl+Shift+A', scope: 'git' },
  { id: 'stash', group: 'Work in progress', title: 'Stash changes…', keys: 'Ctrl+Shift+S', scope: 'git' },
  { id: 'pop', group: 'Work in progress', title: 'Pop latest stash', keys: '', scope: 'git' },
  { id: 'op-continue', group: 'Work in progress', title: 'Continue operation in progress', keys: '', scope: 'git' },
  { id: 'op-abort', group: 'Work in progress', title: 'Abort operation in progress…', keys: '', scope: 'git' },
];

const DEFAULTS = new Map(SHORTCUTS.map((s) => [s.id, s.keys]));

/** Text editing combinations: they cannot be given to an action. */
const RESERVED = new Set(['Ctrl+C', 'Ctrl+V', 'Ctrl+X', 'Ctrl+A', 'Ctrl+Z', 'Ctrl+Y', 'Ctrl+Shift+Z']);

/** Combination of a keyboard event, such as `Ctrl+Shift+F`; empty for a modifier key alone. */
export function comboOf(event: KeyboardEvent): string {
  let key = event.key;
  if (['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'Dead', 'Unidentified'].includes(key)) return '';
  if (key === ' ') key = 'Space';
  if (key.length === 1) key = key.toUpperCase();
  const parts = [];
  if (event.ctrlKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  parts.push(key);
  return parts.join('+');
}

/** Why a combination cannot be a shortcut; empty when it can. */
export function refusal(combo: string): string {
  if (RESERVED.has(combo)) return 'This combination is used to edit text.';
  const functionKey = /^(Ctrl\+|Alt\+|Shift\+)*F\d{1,2}$/.test(combo);
  if (!functionKey && !combo.includes('Ctrl+') && !combo.includes('Alt+')) {
    return 'A shortcut must include Ctrl or Alt, or be a function key.';
  }
  return '';
}

/** Readable form: `Ctrl+Shift+F` → `Ctrl + Shift + F`. A final `+` is the key itself: `Ctrl++` → `Ctrl + +`. */
export const display = (combo: string) => combo.split(/\+(?!$)/).join(' + ');

/**
 * In a terminal, the Ctrl + letter combinations belong to the shell (Ctrl + K deletes the end of
 * the line, Ctrl + W the previous word). Only those a shell does not use go through: with Shift
 * or Alt, or a function key.
 */
export const allowedInTerminal = (combo: string) => /Shift\+|Alt\+|(^|\+)F\d{1,2}$/.test(combo);

function read(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}');
  } catch {
    return {};
  }
}

class Keys {
  /** Shortcuts changed by the user; an empty value removes the shortcut. */
  overrides = $state<Record<string, string>>(read());
  /** True while a new combination is being typed in the settings: no shortcut fires. */
  recording = $state(false);

  binding(id: string): string {
    return this.overrides[id] ?? DEFAULTS.get(id) ?? '';
  }

  isCustom(id: string): boolean {
    return id in this.overrides;
  }

  /** Command bound to a combination in the module `scope`, if there is one. */
  find(combo: string, scope: Scope): string | null {
    return SHORTCUTS.find((s) => this.binding(s.id) === combo && (!s.scope || s.scope === scope))?.id ?? null;
  }

  /** The action that would lose `combo` if it were given to `id`, if there is one. */
  rival(id: string, combo: string): Shortcut | null {
    const own = SHORTCUTS.find((s) => s.id === id);
    if (!own || !combo) return null;
    return SHORTCUTS.find((s) => s.id !== id && this.binding(s.id) === combo && overlap(own, s)) ?? null;
  }

  /** Gives `combo` to `id`. A combination has one role per module: it is taken from the action
   *  that had it wherever both act. */
  set(id: string, combo: string) {
    const next = { ...this.overrides };
    const own = SHORTCUTS.find((s) => s.id === id);
    if (combo && own) {
      for (const other of SHORTCUTS) {
        if (other.id !== id && this.binding(other.id) === combo && overlap(own, other)) next[other.id] = '';
      }
    }
    next[id] = combo;
    this.#save(next);
  }

  reset(id: string) {
    const next = { ...this.overrides };
    delete next[id];
    // The default shortcut coming back may have been given to another action in the meantime.
    const restored = DEFAULTS.get(id) ?? '';
    const own = SHORTCUTS.find((s) => s.id === id);
    if (restored && own) {
      for (const other of SHORTCUTS) {
        if (other.id !== id && (next[other.id] ?? DEFAULTS.get(other.id)) === restored && overlap(own, other)) next[other.id] = '';
      }
    }
    this.#save(next);
  }

  resetAll() {
    this.#save({});
  }

  /** Puts custom shortcuts back as they were, after a “Reset all” that was undone. */
  restore(overrides: Record<string, string>) {
    this.#save({ ...overrides });
  }

  #save(next: Record<string, string>) {
    // A value equal to the default is not a customization.
    for (const [id, combo] of Object.entries(next)) if (combo === DEFAULTS.get(id)) delete next[id];
    this.overrides = next;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      /* storage unavailable: the shortcuts hold for this session */
    }
  }
}

export const keys = new Keys();
