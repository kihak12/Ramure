// Mounts the panel of the test on its own, with sample branches, to take screenshots of it.
// It reads the copy placed in src/lib/git/ (see the README of the test).
import { mount } from 'svelte';
import '@fontsource-variable/dm-sans';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import '/src/app.css';
import CleanupPanel from '/src/lib/git/CleanupPanel.svelte';

const branches = [
  { name: 'fix/graph-focus', sha: '9f3c2ab41d07', merged: true, commits: 3 },
  { name: 'design/tokens', sha: '024045d9aa10', merged: true, commits: 12 },
  { name: 'feature/import-hosts-from-the-ssh-configuration-file-on-first-launch', sha: '7ac2a4a0c3e2', merged: true, commits: 128 },
  { name: 'git/resizable-cards', sha: '0693489b77f1', merged: true, commits: 5 },
  { name: 'experiment/local-terminal', sha: '5d1e09c2b6a4', merged: false, commits: 1 },
  { name: 'ssh/choose-key', sha: '444245ae18c9', merged: true, commits: 2 },
  { name: 'docs/english', sha: '43de7c8f7368', merged: false, commits: 9 },
  { name: 'design/logo', sha: '6490118d2c55', merged: true, commits: 7 },
  { name: 'tooling/designer-subagent', sha: '67ddfd30e5cf', merged: true, commits: 4 },
  { name: 'git/track-remote-branch', sha: 'db95823a91b0', merged: true, commits: 6 },
  { name: 'design/clairiere', sha: '7ac2a4a11f0d', merged: true, commits: 21 },
];

// The fetch never ends, to keep the waiting indicator on screen; a deletion fails.
const repo = {
  sync: () => new Promise<void>(() => {}),
  refOp: async () => false,
};

mount(CleanupPanel, {
  target: document.getElementById('app')!,
  props: { repo: repo as never, branches, onclose: () => {} },
});
