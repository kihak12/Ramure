// The settings page: whether it is on screen, and on which section (guidelines, section 9.18).

export const SECTIONS = [
  { id: 'general', title: 'General' },
  { id: 'git', title: 'Git' },
  { id: 'ssh', title: 'Terminal and SSH' },
  { id: 'keys', title: 'Keyboard shortcuts' },
] as const;

export type SectionId = (typeof SECTIONS)[number]['id'];

class SettingsPage {
  open = $state(false);
  section = $state<SectionId>('general');

  /** Opens the page, on `section` if one is asked for, otherwise where it was left. */
  show(section?: SectionId) {
    if (section) this.section = section;
    this.open = true;
  }

  hide() {
    this.open = false;
  }

  /** Opens the page on `section`, or closes it if it is open. */
  toggle(section?: SectionId) {
    if (this.open) this.hide();
    else this.show(section);
  }
}

export const settingsPage = new SettingsPage();
