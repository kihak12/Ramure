// Thème clair ou sombre, celui du système ou celui qu'on a choisi, et thème de couleurs
// (charte, section 2.5). Les jetons de src/app.css suivent les attributs `data-theme` et
// `data-palette` de la racine.

import { getCurrentWindow } from '@tauri-apps/api/window';
import { prefs } from '../git/prefs.svelte';

const system = window.matchMedia('(prefers-color-scheme: dark)');

class Theme {
  #systemDark = $state(system.matches);

  /** Vrai quand l'interface est sombre. Ce qui dessine ses propres couleurs (graphe, terminal) le lit. */
  dark = $derived(prefs.theme === 'system' ? this.#systemDark : prefs.theme === 'dark');
  /** Change dès qu'une couleur change : clair ou sombre, ou thème de couleurs. Le graphe et le
   *  terminal, qui lisent les jetons pour dessiner, se redessinent quand elle change. */
  key = $derived(`${prefs.palette}:${this.dark ? 'dark' : 'light'}`);

  constructor() {
    system.addEventListener('change', () => (this.#systemDark = system.matches));
  }

  /** Applique le choix à la page, et à la barre de titre de la fenêtre quand le système le permet. */
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
          /* système sans thème de fenêtre : la page, elle, a déjà changé */
        });
    } catch {
      /* hors de Tauri (aperçu dans un navigateur) : il n'y a pas de fenêtre à régler */
    }
  }
}

export const theme = new Theme();
