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
      id: 'terminal-journal',
      group: 'Terminal',
      icon: 'log',
      title: session?.journal?.live ? 'Arrêter le journal du terminal' : 'Enregistrer le journal du terminal',
      keywords: 'journal log enregistrer fichier sortie',
      disabled: !session || session.status !== 'connected',
      hint: !session ? 'aucun terminal ouvert' : session.status !== 'connected' ? 'le terminal n’est pas connecté' : undefined,
      run: () => {
        if (session) session.journalAsk = (session.journalAsk ?? 0) + 1;
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
