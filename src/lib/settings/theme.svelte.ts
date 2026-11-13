// Light or dark theme, the system's or the one chosen, and color theme (guidelines, section 2.5).
// The tokens of src/app.css follow the `data-theme` and `data-palette` attributes of the root.

import { getCurrentWindow } from '@tauri-apps/api/window';
import { prefs } from '../git/prefs.svelte';

const system = window.matchMedia('(prefers-color-scheme: dark)');

class Theme {
  #systemDark = $state(system.matches);

  /** True when the interface is dark. What draws its own colors (graph, terminal) reads it. */
  dark = $derived(prefs.theme === 'system' ? this.#systemDark : prefs.theme === 'dark');
  /** Changes as soon as a color changes: light or dark, or color theme. The graph and the
   *  terminal, which read the tokens to draw, redraw when it changes. */
  key = $derived(`${prefs.palette}:${this.dark ? 'dark' : 'light'}`);

  constructor() {
    system.addEventListener('change', () => (this.#systemDark = system.matches));
  }

  /** Applies the choice to the page, and to the title bar of the window when the system allows it. */
  apply() {
    const choice = prefs.theme;
    const root = document.documentElement;
    if (choice === 'system') delete root.dataset.theme;
    else root.dataset.theme = choice;
    root.dataset.palette = prefs.palette;
    try {
      void getCurrentWindow()
        .setTheme(choice === 'system' ? null : choice)
        .catch(() => {
          /* a system without a window theme: the page itself has already changed */
        });
    } catch {
      /* outside Tauri (a preview in a browser): there is no window to set */
    }
  }
}

export const theme = new Theme();
