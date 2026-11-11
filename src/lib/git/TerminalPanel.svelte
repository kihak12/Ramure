<script lang="ts">
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { display, keys } from '../ui/keys.svelte';
  import Terminal from '../ssh/Terminal.svelte';
  import type { Session } from '../ssh/ssh.svelte';
  import type { Repo } from './repo.svelte';

  /**
   * Terminaux d'un dépôt, sous le graphe (charte, section 9.20). `active` : la vue du dépôt est
   * à l'écran et le panneau affiché.
   */
  let { repo, active }: { repo: Repo; active: boolean } = $props();

  const terminals = $derived(repo.terminals);
  let panelEl = $state<HTMLElement>();

  const combo = $derived(keys.binding('toggle-terminal'));
  const hideTitle = $derived(`Masquer le terminal${combo ? ` (${display(combo)})` : ''}`);
  const NEW = 'Nouveau terminal dans le dépôt';

  /** Ce qu'un onglet dit au survol : le shell et son dossier de départ, que l'en-tête ne montre pas. */
  function about(session: Session) {
    const what = session.status === 'closed' ? 'Terminal local arrêté' : 'Terminal local';
    return session.opened ? `${what} · ${session.opened.shell} · ouvert dans ${session.opened.cwd}` : what;
  }

  // Une demande de clavier (raccourci) : le terminal actif le prend, une fois le panneau affiché.
  $effect(() => {
    if (!terminals.focusAsk) return;
    void tick().then(() =>
      requestAnimationFrame(() => panelEl?.querySelector<HTMLElement>('.view.current .xterm-helper-textarea')?.focus()),
    );
  });

  // --- Le graphe suit (charte, section 9.20) --------------------------------------------------
  // Après une commande, le dépôt est relu : quand la sortie s'est tue 600 ms, au plus toutes les 2 s.
  const QUIET = 600;
  const SPACING = 2000;
  let quietTimer: number | undefined;
  let lastRead = 0;
  let reading = false;

  async function reread() {
    window.clearTimeout(quietTimer);
    if (reading || !repo.result || repo.locked) return;
    reading = true;
    lastRead = Date.now();
    try {
      await Promise.all([repo.refreshStatus(), repo.refreshRefs()]);
    } finally {
      reading = false;
    }
  }

  function output() {
    window.clearTimeout(quietTimer);
    const wait = Math.max(QUIET, lastRead + SPACING - Date.now());
    quietTimer = window.setTimeout(() => void reread(), wait);
  }

  // Le panneau se masque : ce qui s'est passé dans le terminal se voit dans le graphe.
  let wasShown = false;
  $effect(() => {
    const shown = terminals.shown;
    if (wasShown && !shown) void reread();
    wasShown = shown;
  });
  $effect(() => () => window.clearTimeout(quietTimer));
</script>

<section class="panel" bind:this={panelEl} data-terminals aria-label="Terminal du dépôt">
  <header>
    <div class="tabs" role="tablist" aria-label="Terminaux du dépôt">
      {#each terminals.sessions as session (session.key)}
        {@const current = session.key === terminals.activeKey}
        {@const stopped = session.status === 'closed'}
        <div class="tab" class:current>
          <button
            class="open"
            type="button"
            role="tab"
            aria-selected={current}
            title={about(session)}
            onclick={() => terminals.activate(session.key)}
            onauxclick={(e) => {
              // Clic du milieu : ferme l'onglet.
              if (e.button === 1) terminals.close(session.key);
            }}
          >
            <!-- Un terminal local porte l'ordinateur, comme partout où « local » est dit (charte, section 7). -->
            <span class="local" class:stopped role="img" aria-label={stopped ? 'Terminal local arrêté' : 'Terminal local'}>
              <Icon name="laptop" size={14} />
            </span>
            <span class="name">{session.title}</span>
          </button>
          <button class="close" type="button" aria-label="Fermer le terminal {session.title}" title="Fermer le terminal" onclick={() => terminals.close(session.key)}>
            <Icon name="x" size={14} />
          </button>
        </div>
      {/each}
      <button class="add" type="button" aria-label={NEW} title={NEW} onclick={() => terminals.open(repo.path)}>
        <Icon name="plus" size={16} />
      </button>
    </div>
    <button class="hide" type="button" aria-label="Masquer le terminal" title={hideTitle} onclick={() => terminals.hide()}>
      <Icon name="panel-bottom" size={16} />
    </button>
  </header>

  <div class="views">
    {#each terminals.sessions as session (session.key)}
      {@const current = session.key === terminals.activeKey}
      <div class="view" class:current inert={!current} role="tabpanel">
        <Terminal {session} bare autofocus={false} active={active && current} onoutput={output} onclose={() => terminals.close(session.key)} />
      </div>
    {/each}
  </div>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 35px;
    padding: 0 4px 0 6px;
    border-bottom: 1px solid var(--line-soft);
  }
  /* Les 4 px de marge, rendus par une marge négative, laissent la place à l'anneau de focus des
     onglets : sans eux, le défilement horizontal le rognerait. */
  .tabs {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    margin: 0 -4px;
    padding: 4px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    flex: 0 1 180px;
    display: flex;
    align-items: center;
    min-width: 90px;
    height: 26px;
    border-radius: var(--radius);
    color: var(--muted);
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .tab:hover {
    background: var(--hover);
    color: var(--ink);
  }
  /* L'onglet actif est déjà sur une carte : il se lit par le fond de la sélection. */
  .tab.current {
    background: var(--sel);
    color: var(--ink);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 100%;
    padding: 0 4px 0 8px;
    border: 0;
    background: none;
    cursor: pointer;
    color: inherit;
    text-align: left;
  }
  .local {
    flex: none;
    display: grid;
  }
  .local.stopped {
    color: var(--faint);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--fw-regular);
  }
  .current .name {
    font-weight: var(--fw-strong);
  }
  .close {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-right: 2px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  .tab:hover .close,
  .tab.current .close,
  .close:focus-visible {
    opacity: 1;
  }
  .close:hover {
    background: var(--line);
    color: var(--ink);
  }
  /* « + » après le dernier onglet : il reste visible quand les onglets débordent. */
  .add {
    position: sticky;
    right: 0;
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: var(--panel);
    color: var(--muted);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .add:hover:not(:disabled),
  .hide:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .hide {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .views {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .view {
    position: absolute;
    inset: 0;
    visibility: hidden;
    pointer-events: none;
  }
  .view.current {
    visibility: visible;
    pointer-events: auto;
  }
</style>
