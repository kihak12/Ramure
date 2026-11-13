// Commands of the SSH module offered in the palette (Ctrl + K): actions on the terminal of the
// current tab.

import type { Command } from '../ui/palette.svelte';
import { ssh } from './ssh.svelte';

export function sshCommands(): Command[] {
  const session = ssh.active;
  return [
    {
      id: 'terminal-search',
      group: 'Terminal',
      icon: 'search',
      title: 'Search the terminal',
      keywords: 'find text',
      disabled: !session,
      hint: session ? undefined : 'no terminal open',
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
      title: session?.journal?.live ? 'Stop terminal log' : 'Record terminal log',
      keywords: 'log journal record save file output',
      disabled: !session || session.status !== 'connected',
      hint: !session ? 'no terminal open' : session.status !== 'connected' ? 'the terminal is not connected' : undefined,
      run: () => {
        if (session) session.journalAsk = (session.journalAsk ?? 0) + 1;
      },
    },
    {
      id: 'ssh-export',
      group: 'SSH',
      icon: 'export',
      title: 'Export to ~/.ssh/config…',
      keywords: 'config ssh write book',
      disabled: !ssh.hosts.length,
      hint: ssh.hosts.length ? undefined : 'no hosts',
      run: () => (ssh.exporting = 'all'),
    },
  ];
}
