<script lang="ts">
  import { gitCommands } from './lib/git/commands';
  import { sshCommands } from './lib/ssh/commands';
  import { ssh } from './lib/ssh/ssh.svelte';
  import GitWorkspace from './lib/git/GitWorkspace.svelte';
  import SshWorkspace from './lib/ssh/SshWorkspace.svelte';
  import ContextMenu from './lib/ui/ContextMenu.svelte';
  import Dialog from './lib/ui/Dialog.svelte';
  import Icon from './lib/ui/Icon.svelte';
  import { allowedInTerminal, comboOf, display, keys } from './lib/ui/keys.svelte';
  import CommandPalette from './lib/ui/CommandPalette.svelte';
  import { palette, type Command } from './lib/ui/palette.svelte';
  import Notifications from './lib/ui/Notifications.svelte';
  import SettingsPage from './lib/settings/SettingsPage.svelte';
  import { settingsPage } from './lib/settings/settings.svelte';

  type View = 'git' | 'ssh';
  let view = $state<View>('git');
  // La page de réglages prend la place du module ; en la quittant, on retrouve celui qu'on regardait.
  const shown = $derived(settingsPage.open ? 'settings' : view);

  let settingsButton = $state<HTMLButtonElement>();

  /** Les réglages s'ouvrent sur la section du module d'où on vient (charte, section 9.18). */
  const toggleSettings = () => settingsPage.toggle(view === 'git' ? 'git' : 'ssh');

  function show(module: View) {
    view = module;
    settingsPage.hide();
  }

  /** Toutes les commandes disponibles maintenant, avec leur raccourci. */
  function commands(): Command[] {
    const general: Command[] = [
      { id: 'view-git', group: 'Modules', icon: 'branch', title: 'Afficher les dépôts', disabled: shown === 'git', run: () => show('git') },
      { id: 'view-ssh', group: 'Modules', icon: 'terminal', title: 'Afficher le terminal SSH', disabled: shown === 'ssh', run: () => show('ssh') },
      { id: 'settings', group: 'Modules', icon: 'settings', title: settingsPage.open ? 'Fermer les réglages' : 'Réglages', keywords: 'preferences options theme sombre clair style graphe', run: toggleSettings },
      {
        id: 'terminal-new',
        group: 'Modules',
        icon: 'laptop',
        title: 'Nouveau terminal local',
        keywords: 'shell console ligne de commande powershell bash',
        run: () => {
          show('ssh');
          ssh.openLocal();
        },
      },
      { id: 'shortcuts', group: 'Modules', icon: 'settings', title: 'Raccourcis clavier', keywords: 'touches clavier reglages', run: () => settingsPage.show('keys') },
    ];
    // Les actions Git n'ont de sens que devant un dépôt.
    const all = shown === 'git' ? [...gitCommands(), ...general] : shown === 'ssh' ? [...sshCommands(), ...general] : general;
    return all.map((command) => {
      const combo = keys.binding(command.id);
      return combo ? { ...command, shortcut: display(combo) } : command;
    });
  }

  const openPalette = () => palette.show(commands());

  function onKey(e: KeyboardEvent) {
    // On tape une nouvelle combinaison dans les réglages : elle ne déclenche rien.
    if (keys.recording) return;
    const modal = document.querySelector('dialog[open]') !== null;
    // Échap quitte la page de réglages. Un champ de recherche rempli se vide d'abord.
    if (e.key === 'Escape' && settingsPage.open && !modal) {
      if (e.target instanceof HTMLInputElement && e.target.type === 'search' && e.target.value) return;
      e.preventDefault();
      settingsPage.hide();
      // La page devient inerte : le focus revient sur le bouton qui l'ouvre, il ne se perd pas.
      settingsButton?.focus();
      return;
    }
    const combo = comboOf(e);
    if (!combo) return;
    // Ctrl + Maj + P ouvre toujours la palette, quel que soit le raccourci choisi pour elle.
    const id = combo === 'Ctrl+Shift+P' ? 'palette' : keys.find(combo, view);
    if (!id) return;
    const inTerminal = e.target instanceof Element && e.target.closest('.xterm') !== null;
    if (inTerminal && !allowedInTerminal(combo)) return;
    // Une boîte de dialogue ouverte garde le clavier ; seule la palette se referme par son raccourci.
    if (modal && !(id === 'palette' && palette.commands)) return;
    e.preventDefault();
    e.stopPropagation();
    if (id === 'palette') {
      if (palette.commands) palette.hide();
      else openPalette();
      return;
    }
    const command = commands().find((c) => c.id === id);
    if (command && !command.disabled) (command.key ?? command.run)();
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<div class="shell">
  <nav class="rail" aria-label="Modules">
    <button type="button" aria-pressed={shown === 'git'} title="Dépôts" aria-label="Dépôts" onclick={() => show('git')}>
      <Icon name="branch" size={18} />
    </button>
    <button type="button" aria-pressed={shown === 'ssh'} title="Terminal SSH" aria-label="Terminal SSH" onclick={() => show('ssh')}>
      <Icon name="terminal" size={18} />
    </button>
    <button class="last" type="button" title="Palette de commandes{keys.binding('palette') ? ` (${display(keys.binding('palette'))})` : ''}" aria-label="Palette de commandes" onclick={openPalette}>
      <Icon name="search" size={18} />
    </button>
    <button
      bind:this={settingsButton}
      type="button"
      aria-pressed={settingsPage.open}
      title="Réglages{keys.binding('settings') ? ` (${display(keys.binding('settings'))})` : ''}"
      aria-label="Réglages"
      onclick={toggleSettings}
    >
      <Icon name="settings" size={18} />
    </button>
  </nav>

  <!-- Les vues restent montées : une session SSH survit au passage par le graphe ou par les réglages. -->
  <main>
    <section class="pane" class:hidden={shown !== 'git'} inert={shown !== 'git'}><GitWorkspace active={shown === 'git'} /></section>
    <section class="pane" class:hidden={shown !== 'ssh'} inert={shown !== 'ssh'}><SshWorkspace active={shown === 'ssh'} /></section>
    <section class="pane" class:hidden={shown !== 'settings'} inert={shown !== 'settings'} aria-label="Réglages"><SettingsPage /></section>
  </main>
</div>

<!-- Couches communes à toute l'application. -->
<CommandPalette />
<Dialog />
<ContextMenu />
<Notifications />

<style>
  .shell {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr);
    height: 100%;
  }
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    background: var(--side);
  }
  .rail button {
    position: relative;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    display: grid;
    place-items: center;
    color: var(--muted);
    background: none;
    border: 0;
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .rail button:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .rail .last {
    margin-top: auto;
  }
  .rail button:active {
    transform: scale(0.97);
  }
  .rail button[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--accent);
  }
  /* Repère du module actif, qui glisse le long du rail. */
  .rail button::before {
    content: '';
    position: absolute;
    left: -6px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 var(--radius-pill) var(--radius-pill) 0;
    background: var(--accent);
    transform: scaleY(0);
    transition: transform var(--t-slow) var(--ease);
  }
  .rail button[aria-pressed='true']::before {
    transform: scaleY(1);
  }
  main {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  /* Les vues sont superposées et se fondent l'une dans l'autre. */
  .pane {
    position: absolute;
    inset: 0;
    min-height: 0;
    transition:
      opacity var(--t-med) var(--ease),
      transform var(--t-med) var(--ease),
      visibility 0s;
  }
  .pane.hidden {
    opacity: 0;
    visibility: hidden;
    transform: translateY(6px);
    pointer-events: none;
    transition:
      opacity var(--t-med) var(--ease),
      transform var(--t-med) var(--ease),
      visibility 0s var(--t-med);
  }
</style>
