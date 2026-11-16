// The module on screen: Git, SSH or Files. The settings page takes its place without changing it
// (guidelines, 9.18): leaving the settings brings it back.
import { settingsPage } from '../settings/settings.svelte';

export type Module = 'git' | 'ssh' | 'files';

class Modules {
  view = $state<Module>('git');

  show(module: Module) {
    this.view = module;
    settingsPage.hide();
  }
}

export const modules = new Modules();
