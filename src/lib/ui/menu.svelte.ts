// Menu contextuel : un seul à la fois, ouvert à l'endroit du clic droit.
import type { IconName } from './Icon.svelte';

export type MenuItem =
  | { label: string; icon?: IconName; run: () => void; danger?: boolean; disabled?: boolean; hint?: string }
  | { separator: true };

class Menu {
  current = $state.raw<{ x: number; y: number; items: MenuItem[] } | null>(null);

  open(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    event.stopPropagation();
    this.current = items.length ? { x: event.clientX, y: event.clientY, items } : null;
  }

  close() {
    this.current = null;
  }
}

export const menu = new Menu();
