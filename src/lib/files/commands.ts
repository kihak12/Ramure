// Commands of the Files module offered in the palette (Ctrl + K).

import type { Command } from '../ui/palette.svelte';
import { files, MAX_PANES } from './files.svelte';

export function filesCommands(): Command[] {
  const pane = files.pane(files.focused) ?? files.panes[0];
  const ready = !!pane && !!files.place(pane);
  return [
    {
      id: 'files-add-pane',
      group: 'Files',
      icon: 'plus',
      title: 'Add pane',
      keywords: 'new pane column location host',
      disabled: files.panes.length >= MAX_PANES,
      hint: files.panes.length >= MAX_PANES ? 'four panes at most' : undefined,
      run: () => {
        const added = files.add(pane ?? undefined);
        if (added) void files.focus(added);
      },
    },
    {
      id: 'files-pane-next',
      group: 'Files',
      icon: 'panel-right',
      title: 'Next pane',
      keywords: 'switch pane focus other side',
      disabled: files.panes.length < 2,
      hint: files.panes.length < 2 ? 'one pane only' : undefined,
      run: () => files.cycle(1),
    },
    {
      id: 'files-pane-prev',
      group: 'Files',
      icon: 'panel-left',
      title: 'Previous pane',
      keywords: 'switch pane focus other side',
      disabled: files.panes.length < 2,
      hint: files.panes.length < 2 ? 'one pane only' : undefined,
      run: () => files.cycle(-1),
    },
    {
      id: 'files-hidden',
      group: 'Files',
      icon: 'eye',
      title: pane?.hidden ? 'Hide hidden files' : 'Show hidden files',
      keywords: 'dot files hidden',
      disabled: !pane?.location,
      hint: pane?.location ? undefined : 'no location chosen',
      run: () => {
        if (!pane) return;
        pane.hidden = !pane.hidden;
        files.save();
      },
    },
    {
      id: 'files-mkdir',
      group: 'Files',
      icon: 'folder-plus',
      title: 'New folder…',
      keywords: 'create directory mkdir',
      disabled: !ready,
      hint: ready ? undefined : 'the pane is not connected',
      run: () => pane && void files.mkdir(pane),
    },
    {
      id: 'files-touch',
      group: 'Files',
      icon: 'file-plus',
      title: 'New file…',
      keywords: 'create empty touch',
      disabled: !ready,
      hint: ready ? undefined : 'the pane is not connected',
      run: () => pane && void files.touch(pane),
    },
    {
      id: 'files-clear',
      group: 'Files',
      icon: 'x',
      title: 'Clear finished transfers',
      disabled: !files.transfers.some((t) => t.state !== 'counting' && t.state !== 'copying'),
      hint: files.transfers.some((t) => t.state !== 'counting' && t.state !== 'copying') ? undefined : 'no finished transfer',
      run: () => files.clearFinished(),
    },
  ];
}
