// Commandes du module SSH proposées dans la palette (Ctrl + K) : actions sur le terminal de
// l'onglet courant.

import type { Command } from '../ui/palette.svelte';
import { ssh } from './ssh.svelte';

export function sshCommands(): Command[] {
  const session = ssh.active;
  return [
    {
      id: 'terminal-search',
      group: 'Terminal',
      icon: 'search',
      title: 'Chercher dans le terminal',
      keywords: 'rechercher trouver texte',
      disabled: !session,
      hint: session ? undefined : 'aucun terminal ouvert',
      run: () => {
        if (!session) return;
        session.searching = true;
        session.searchAsk++;
      },
    },
    {
      id: 'ssh-export',
      group: 'SSH',
      icon: 'export',
      title: 'Exporter le carnet vers ~/.ssh/config…',
      keywords: 'config ssh ecrire hotes',
      disabled: !ssh.hosts.length,
      hint: ssh.hosts.length ? undefined : 'le carnet est vide',
      run: () => (ssh.exporting = 'all'),
    },
  ];
}
