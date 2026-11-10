// Monte le panneau de l'essai, seul, avec des branches d'exemple, pour en prendre des captures.
// Il lit la copie posée dans src/lib/git/ (voir le README de l'essai).
import { mount } from 'svelte';
import '@fontsource-variable/dm-sans';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import '/src/app.css';
import CleanupPanel from '/src/lib/git/CleanupPanel.svelte';

const branches = [
  { name: 'correctif/focus-du-graphe', sha: '9f3c2ab41d07', merged: true, commits: 3 },
  { name: 'design/jetons', sha: '024045d9aa10', merged: true, commits: 12 },
  { name: 'fonctionnalite/import-des-hotes-depuis-le-fichier-de-configuration-ssh', sha: '7ac2a4a0c3e2', merged: true, commits: 128 },
  { name: 'git/cartes-redimensionnables', sha: '0693489b77f1', merged: true, commits: 5 },
  { name: 'essai/terminal-local', sha: '5d1e09c2b6a4', merged: false, commits: 1 },
  { name: 'ssh/choisir-cle', sha: '444245ae18c9', merged: true, commits: 2 },
  { name: 'docs/anglais', sha: '43de7c8f7368', merged: false, commits: 9 },
  { name: 'design/logo', sha: '6490118d2c55', merged: true, commits: 7 },
  { name: 'outillage/subagent-designer', sha: '67ddfd30e5cf', merged: true, commits: 4 },
  { name: 'git/suivre-branche-distante', sha: 'db95823a91b0', merged: true, commits: 6 },
  { name: 'design/clairiere', sha: '7ac2a4a11f0d', merged: true, commits: 21 },
];

// « Récupérer » ne se termine jamais, pour garder l'indicateur d'attente ; une suppression échoue.
const repo = {
  sync: () => new Promise<void>(() => {}),
  refOp: async () => false,
};

mount(CleanupPanel, {
  target: document.getElementById('app')!,
  props: { repo: repo as never, branches, onclose: () => {} },
});
