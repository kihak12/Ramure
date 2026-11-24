// Commands of the SSH module offered in the palette (Ctrl + K): actions on the terminal of the
// current tab.

import type { Command } from '../ui/palette.svelte';
import { mayCloseTerminal } from './guard';
import { ssh } from './ssh.svelte';

/** The tab `by` places after the active one, in turn: its terminal takes the keyboard. */
function stepTab(by: 1 | -1) {
  const tabs = ssh.sessions;
  if (tabs.length < 2) return;
  const at = Math.max(0, tabs.findIndex((s) => s.key === ssh.activeKey));
  ssh.activeKey = tabs[(at + by + tabs.length) % tabs.length].key;
}

export function sshCommands(): Command[] {
  const session = ssh.active;
  const several = ssh.sessions.length > 1;
  return [
    {
      id: 'terminal-tab-next',
      group: 'Terminal',
      icon: 'terminal',
      title: 'Next terminal tab',
      keywords: 'switch tab terminal session',
      disabled: !several,
      hint: several ? undefined : 'one terminal tab only',
      run: () => stepTab(1),
    },
    {
      id: 'terminal-tab-prev',
      group: 'Terminal',
      icon: 'terminal',
      title: 'Previous terminal tab',
      keywords: 'switch tab terminal session',
      disabled: !several,
      hint: several ? undefined : 'one terminal tab only',
      run: () => stepTab(-1),
    },
    {
      id: 'terminal-tab-close',
      group: 'Terminal',
      icon: 'x',
      title: 'Close terminal tab',
      keywords: 'close tab terminal session disconnect',
      disabled: !session,
      hint: session ? undefined : 'no terminal open',
      // As the cross of the tab: if a program runs in a local terminal, ask first.
      run: () => {
        if (session) void mayCloseTerminal(session).then((sure) => sure && ssh.close(session.key));
      },
    },
    {
      id: 'ssh-book',
      group: 'Terminal',
      icon: 'search',
      title: 'Go to the host book',
      keywords: 'hosts book filter leave terminal',
      run: () => void ssh.focusBook(true),
    },
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
      // The strip of the tunnels, even for a session that has none yet: "Add tunnel" is there
      // (guidelines, 9.15, "Tunnels").
      id: 'terminal-tunnels',
      group: 'Terminal',
      icon: 'tunnel',
      title: 'Show or hide tunnels',
      keywords: 'tunnel forward port socks proxy',
      disabled: !session?.tunnels?.live,
      hint: !session
        ? 'no terminal open'
        : !session.tunnels
          ? 'a local terminal has no tunnels'
          : !session.tunnels.live
            ? 'the terminal is not connected'
            : undefined,
      run: () => {
        if (session?.tunnels) session.tunnels.shown = !session.tunnels.shown;
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
