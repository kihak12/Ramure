<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { flip } from 'svelte/animate';
  import { fade } from 'svelte/transition';
  import { dialogs } from '../ui/dialogs.svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu } from '../ui/menu.svelte';
  import HostDialog from './HostDialog.svelte';
  import KeysDialog from './KeysDialog.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import { display, keys } from '../ui/keys.svelte';
  import { mayCloseTerminal } from './guard';
  import { ssh, STATUS, type Session } from './ssh.svelte';
  import Terminal from './Terminal.svelte';
  import { addressOf, emptyHost, type Host } from './types';
  import { motion } from '../ui/motion';

  /** Vrai quand le module SSH est à l'écran. */
  let { active = false }: { active?: boolean } = $props();

  // L'onglet actif reste à l'écran quand les onglets débordent : la barre défile jusqu'à lui,
  // et la marge de défilement le garde à gauche du « + ».
  let tabsEl = $state<HTMLElement>();
  $effect(() => {
    void ssh.activeKey;
    void tick().then(() => tabsEl?.querySelector('.tab.current')?.scrollIntoView({ block: 'nearest', inline: 'nearest' }));
  });

  let filter = $state('');

  const needle = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    ssh.hosts.filter(
      (h) =>
        !needle ||
        [h.alias, h.hostname, h.user, h.group, ...h.tags].some((text) => text.toLowerCase().includes(needle)),
    ),
  );
  // Les hôtes arrivent triés par groupe : on les découpe en sections.
  const sections = $derived.by(() => {
    const out: { group: string; hosts: Host[] }[] = [];
    for (const host of shown) {
      const last = out.at(-1);
      if (last && last.group === host.group) last.hosts.push(host);
      else out.push({ group: host.group, hosts: [host] });
    }
    return out;
  });

  // Les hôtes dont une session s'ouvre : leur ligne le dit aussitôt, là où l'on vient d'agir
  // (charte, section 9.15). L'état vient de la session, pas du terminal : il n'attend pas son montage.
  const waiting = $derived(
    new Map(
      ssh.sessions
        .filter((s) => s.kind === 'ssh' && (s.status === 'connecting' || s.status === 'reconnecting'))
        .map((s) => [s.host.id, s.status]),
    ),
  );

  const address = (host: Host) => addressOf(host.user || ssh.defaultUser, host.hostname, host.port);


  // « + » des onglets : un terminal local, d'un clic. L'infobulle dit le raccourci.
  const newLocal = $derived(`Nouveau terminal local${keys.binding('terminal-new') ? ` (${display(keys.binding('terminal-new'))})` : ''}`);

  /** La croix ou le clic du milieu : si un programme tourne dans un terminal local, on demande d'abord. */
  async function closeTab(session: Session) {
    if (await mayCloseTerminal(session)) ssh.close(session.key);
  }

  /** Le menu de la ligne : la suppression demandée, puis le focus au filtre du carnet. */
  async function removeHost(host: Host) {
    if (await ssh.removeAsked(host)) await ssh.focusBook();
  }

  function hostMenu(event: MouseEvent, host: Host) {
    menu.open(event, [
      { label: 'Ouvrir un terminal', icon: 'terminal', disabled: host.proxyJump, run: () => ssh.open(host) },
      { separator: true },
      { label: 'Modifier…', icon: 'pencil', run: () => (ssh.editing = host) },
      { label: 'Dupliquer…', icon: 'copy', run: () => (ssh.editing = { ...host, id: '', alias: `${host.alias} (copie)` }) },
      { label: 'Exporter vers ~/.ssh/config…', icon: 'export', run: () => (ssh.exporting = host.id) },
      { separator: true },
      { label: 'Supprimer…', icon: 'trash', danger: true, run: () => void removeHost(host) },
    ]);
  }

  onMount(() => void ssh.load());
</script>

<div class="ssh">
  <aside class="book" aria-label="Carnet d’hôtes">
    <header>
      <span class="label">Hôtes · {ssh.hosts.length}</span>
      <button class="btn small" type="button" onclick={() => (ssh.editing = emptyHost())}><Icon name="plus" size={14} /> Ajouter un hôte…</button>
    </header>
    <label class="search">
      <Icon name="search" size={14} />
      <input type="search" placeholder="Filtrer par nom, adresse, groupe, tag" aria-label="Filtrer les hôtes" bind:value={filter} />
    </label>

    <div class="scroll">
      {#each sections as section (section.group)}
        {#if section.group}<div class="group">{section.group}</div>{/if}
        <ul>
          {#each section.hosts as host (host.id)}
            {@const wait = waiting.get(host.id)}
            <li>
              <button
                class="host"
                type="button"
                title="{host.alias} — double-clic pour ouvrir un terminal, clic droit pour les actions"
                ondblclick={() => !host.proxyJump && ssh.open(host)}
                onkeydown={(e) => {
                  if (e.key === 'Enter' && !host.proxyJump) ssh.open(host);
                }}
                oncontextmenu={(e) => hostMenu(e, host)}
              >
                <!-- La couleur choisie nomme l'hôte ; sans couleur, la place reste vide (charte, 9.21). -->
                <span class="tint" aria-hidden="true" style:--c={host.color ? `var(--l${host.color})` : 'transparent'}></span>
                <span class="text">
                  <b>{host.alias}</b>
                  <!-- Pendant la connexion, l'attente prend la place de l'adresse : la ligne garde sa hauteur. -->
                  {#if wait}
                    <small class="wait"><span class="spinner" aria-hidden="true"></span><span>{STATUS[wait]}…</span></small>
                  {:else}
                    <small class="mono">{address(host)}</small>
                  {/if}
                  {#if host.tags.length || host.proxyJump}
                    <span class="tags">
                      {#if host.proxyJump}<span class="chip via" title="Rebond par bastion : pas encore pris en charge">bastion</span>{/if}
                      {#each host.tags as tag (tag)}<span class="chip">{tag}</span>{/each}
                    </span>
                  {/if}
                </span>
              </button>
              <span class="acts">
                <button type="button" aria-label="Ouvrir un terminal sur {host.alias}" title="Ouvrir un terminal" disabled={host.proxyJump} onclick={() => ssh.open(host)}><Icon name="play" size={14} /></button>
                <button type="button" aria-label="Modifier {host.alias}" title="Modifier" onclick={() => (ssh.editing = host)}><Icon name="pencil" size={14} /></button>
              </span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="none">
          {#if !ssh.loaded}
            Lecture du carnet…
          {:else if ssh.hosts.length}
            Aucun hôte ne correspond à « {filter} ».
          {:else}
            Le carnet est vide. Ajoute un hôte, ou importe ceux de ~/.ssh/config.
          {/if}
        </p>
      {/each}
    </div>

    <footer>
      <button class="btn small" type="button" onclick={() => void ssh.importConfig()} title="Ajoute au carnet les hôtes de ~/.ssh/config qui n’y sont pas encore">
        <Icon name="import" size={14} /> Importer ~/.ssh/config
      </button>
      <button class="btn small" type="button" onclick={() => (ssh.exporting = 'all')} title="Écrit les hôtes du carnet dans ~/.ssh/config, après avoir montré ce qui change">
        <Icon name="export" size={14} /> Exporter vers ~/.ssh/config…
      </button>
      <button class="btn small" type="button" onclick={() => (ssh.showKeys = true)}><Icon name="key" size={14} /> Gérer les clés…</button>
    </footer>
  </aside>

  <div class="work">
    {#if ssh.sessions.length}
      <div class="tabs" role="tablist" aria-label="Terminaux ouverts" bind:this={tabsEl}>
        {#each ssh.sessions as session (session.key)}
          {@const current = session.key === ssh.activeKey}
          <div class="tab" class:current animate:flip={{ duration: motion.med }} in:fade={{ duration: motion.med }}>
            <button
              class="open"
              type="button"
              role="tab"
              aria-selected={current}
              title={session.kind === 'local' ? (session.status === 'closed' ? 'Terminal local arrêté' : 'Terminal local') : undefined}
              onclick={() => (ssh.activeKey = session.key)}
              onauxclick={(e) => {
                if (e.button === 1) void closeTab(session);
              }}
            >
              {#if session.kind === 'local'}
                <!-- Un terminal local porte l'ordinateur, comme partout où « local » est dit. -->
                <span
                  class="local"
                  class:stopped={session.status === 'closed'}
                  role="img"
                  aria-label={session.status === 'closed' ? 'Terminal local arrêté' : 'Terminal local'}
                  title={session.status === 'closed' ? 'Terminal local arrêté' : 'Terminal local'}
                >
                  <Icon name="laptop" size={14} />
                </span>
              {:else if session.status === 'connecting'}
                <!-- La connexion s'ouvre : l'anneau d'attente tient la place du point. -->
                <span class="spinner" role="img" aria-label={STATUS.connecting} title={STATUS.connecting}></span>
              {:else}
                <!-- L'état de la session est dit en mots, pas seulement par la couleur du point. -->
                <span
                  class="dot s-{session.status}"
                  role="img"
                  aria-label={STATUS[session.status]}
                  title={STATUS[session.status]}
                  style:--c={session.host.color ? `var(--l${session.host.color})` : 'var(--ok)'}
                ></span>
              {/if}
              <span class="name">{session.title}</span>
              {#if session.journal?.live}
                <!-- Le journal en cours : une icône, dite en mots, sans couleur ni animation (9.7). -->
                <span class="journal" role="img" aria-label="Journal en cours d’enregistrement" title="Journal en cours d’enregistrement">
                  <Icon name="journal" size={14} />
                </span>
              {/if}
            </button>
            <button class="close" type="button" aria-label="Fermer le terminal {session.title}" title="Fermer le terminal" onclick={() => void closeTab(session)}>
              <Icon name="x" size={14} />
            </button>
          </div>
        {/each}
        <button class="add" type="button" aria-label={newLocal} title={newLocal} onclick={() => ssh.openLocal()}>
          <Icon name="plus" size={16} />
        </button>
      </div>
      <div class="views">
        {#each ssh.sessions as session (session.key)}
          {@const current = session.key === ssh.activeKey}
          <section class="view" class:hidden={!current} inert={!current} role="tabpanel">
            <Terminal {session} active={active && current} onclose={() => ssh.close(session.key)} />
          </section>
        {/each}
      </div>
    {:else}
      <div class="empty">
        <span class="art"><Icon name="terminal" size={30} /></span>
        <p>Double-clique un hôte pour ouvrir un terminal distant, ou ouvre un terminal sur cette machine.</p>
        <div class="row">
          <button class="btn primary" type="button" onclick={() => (ssh.editing = emptyHost())}><Icon name="plus" size={14} /> Ajouter un hôte…</button>
          <button class="btn" type="button" onclick={() => ssh.openLocal()}><Icon name="laptop" size={14} /> Ouvrir un terminal local</button>
          {#if ssh.loaded && !ssh.hosts.length}
            <button class="btn" type="button" onclick={() => void ssh.importConfig()}>Importer ~/.ssh/config</button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<HostDialog />
<KeysDialog />
<ExportDialog />

<style>
  .ssh {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    gap: 8px;
    padding: 8px;
    height: 100%;
    min-height: 0;
    background: var(--side);
  }
  .book {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  .book header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 12px 12px 6px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 12px 8px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .search:hover:not(:focus-within) {
    border-color: color-mix(in srgb, var(--muted) 50%, var(--line));
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
    padding: 5px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .scroll {
    overflow-y: auto;
    min-height: 0;
    padding: 0 6px 8px;
  }
  .group {
    padding: 10px 6px 4px;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    transition: background var(--t-fast) var(--ease);
  }
  /* Le fond suit le pointeur, et le focus clavier : un clic ne laisse rien derrière lui, même si le
     bouton de la ligne garde le focus (charte, section 9.6). */
  li:hover,
  li:has(:focus-visible) {
    background: var(--hover);
  }
  .host {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: stretch;
    gap: 9px;
    padding: 7px 8px;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
    color: var(--ink);
  }
  .tint {
    flex: none;
    width: 4px;
    border-radius: var(--radius-pill);
    background: var(--c);
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    line-height: 1.3;
  }
  .text b,
  .text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .text small.wait {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .wait span:last-child {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 3px;
  }
  .chip {
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: var(--fs-micro);
  }
  .chip.via {
    color: var(--warn);
  }
  .acts {
    display: flex;
    gap: 2px;
    padding-right: 6px;
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  li:hover .acts,
  li:has(:focus-visible) .acts {
    opacity: 1;
  }
  .acts button {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .acts button:hover:not(:disabled) {
    background: var(--line);
    color: var(--ink);
  }
  .acts button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .none {
    margin: 0;
    padding: 14px 10px;
    color: var(--muted);
  }
  .book footer {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 12px 12px;
    border-top: 1px solid var(--line-soft);
  }

  .work {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 8px;
    min-width: 0;
    min-height: 0;
  }
  /* 4 px autour des onglets, rendus par une marge négative : la place de l'anneau de focus, que le
     défilement horizontal rognerait sinon. */
  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    margin: -4px;
    padding: 4px;
    overflow-x: auto;
    /* Un onglet qui prend le focus défile jusqu'à rester visible à gauche du « + », qui colle au bord. */
    scroll-padding-inline-end: 40px;
    scrollbar-width: none;
  }
  /* « + » après le dernier onglet (charte, section 9.7) : il reste visible quand les onglets débordent. */
  .add {
    position: sticky;
    right: 0;
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-s);
    background: var(--side);
    /* Les 4 px de marge à sa droite gardent le fond de la barre : aucun onglet n'y dépasse. */
    box-shadow: 4px 0 0 var(--side);
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .add:hover {
    background: var(--hover-side);
    color: var(--ink);
  }
  /* 6 px entre l'icône et le nom : l'écart de l'onglet (8 px) moins 2. */
  .local {
    flex: none;
    display: grid;
    margin-left: 4px;
    margin-right: -2px;
  }
  .local.stopped {
    color: var(--faint);
  }
  .tab {
    flex: 0 1 180px;
    display: flex;
    align-items: center;
    min-width: 90px;
    border-radius: var(--radius);
    color: var(--muted);
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .tab:hover {
    background: var(--hover-side);
    color: var(--ink);
  }
  .tab.current {
    background: var(--panel);
    color: var(--ink);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px 6px 10px;
    border: 0;
    /* Les angles de l'onglet : l'anneau de focus les suit. */
    border-radius: var(--radius);
    background: none;
    cursor: pointer;
    color: inherit;
    text-align: left;
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
  .tab .journal {
    flex: none;
    display: grid;
    color: var(--muted);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .dot.s-connected {
    background: var(--c);
  }
  .dot.s-reconnecting {
    background: var(--warn);
    animation: blink var(--t-wait) var(--ease) infinite;
  }
  /* L'anneau de 12 px tient dans la place du point de 8 px : le titre de l'onglet ne bouge pas. */
  .open .spinner {
    margin: -2px;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .close {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-right: 4px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition:
      background var(--t-fast) var(--ease),
      opacity var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  /* À l'appui, un bouton se resserre à 97 % (charte, sections 8 et 9.1). */
  .close:active,
  .add:active {
    transform: scale(0.97);
  }
  .tab:hover .close,
  .tab.current .close,
  .close:focus-visible {
    opacity: 1;
  }
  /* Sur un onglet survolé (--hover-side) ou actif (--panel), un voile de --ink se voit sur l'un et
     l'autre, en clair et en sombre (charte, 9.7). */
  .close:hover {
    background: color-mix(in srgb, var(--ink) 12%, transparent);
    color: var(--ink);
  }
  .views {
    position: relative;
    min-height: 0;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  .view {
    position: absolute;
    inset: 0;
  }
  .view.hidden {
    visibility: hidden;
    pointer-events: none;
  }
  .empty {
    grid-row: 1 / -1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 14px;
    padding: 24px;
    text-align: center;
    border-radius: var(--radius-card);
    background: var(--panel);
    animation: fade-in var(--t-slow) var(--ease);
  }
  .empty p {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-body);
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .art {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: var(--radius-card);
    color: var(--accent);
    background: var(--accent-soft);
  }
</style>
