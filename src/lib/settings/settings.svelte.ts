// La page de réglages : si elle est à l'écran, et sur quelle section (charte, section 9.18).

export const SECTIONS = [
  { id: 'general', title: 'Général' },
  { id: 'git', title: 'Git' },
  { id: 'ssh', title: 'Terminal et SSH' },
  { id: 'keys', title: 'Raccourcis clavier' },
] as const;

export type SectionId = (typeof SECTIONS)[number]['id'];

class SettingsPage {
  open = $state(false);
  section = $state<SectionId>('general');

  /** Ouvre la page, sur `section` si on en demande une, sinon là où on l'avait laissée. */
  show(section?: SectionId) {
    if (section) this.section = section;
    this.open = true;
  }

  hide() {
    this.open = false;
  }

  toggle() {
    this.open = !this.open;
  }
}

export const settingsPage = new SettingsPage();
