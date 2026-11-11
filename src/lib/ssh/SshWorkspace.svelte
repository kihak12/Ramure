<script lang="ts">
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { scale } from 'svelte/transition';
  import { dialogs } from '../ui/dialogs.svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu } from '../ui/menu.svelte';
  import HostDialog from './HostDialog.svelte';
  import KeysDialog from './KeysDialog.svelte';
  import { ssh } from './ssh.svelte';
  import Terminal from './Terminal.svelte';
  import { emptyHost, type Host } from './types';
  import { motion } from '../ui/motion';

  /** Vrai quand le module SSH est à l'écran. */
  let { active = false }: { active?: boolean } = $props();

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

  const address = (host: Host) => `${host.user || ssh.defaultUser}@${host.hostname}${host.port !== 22 ? `:${host.port}` : ''}`;

  const STATUS = { connecting: 'Connexion en cours', connected: 'Connecté', closed: 'Session terminée' } as const;

  async function remove(host: Host) {
    const confirmed = await dialogs.confirm({
      title: `Supprimer ${host.alias} du carnet ?`,
      body: 'La fiche est supprimée et ne pourra pas être récupérée. Le serveur, ses clés et ~/.ssh/config ne sont pas touchés.',
      confirmLabel: 'Supprimer',
      danger: true,
    });
    if (confirmed) await ssh.remove(host);
  }

  function hostMenu(event: MouseEvent, host: Host) {
    menu.open(event, [
      { label: 'Ouvrir un terminal', icon: 'terminal', disabled: host.proxyJump, run: () => ssh.open(host) },
      { separator: true },
      { label: 'Modifier…', icon: 'pencil', run: () => (ssh.editing = host) },
      { label: 'Dupliquer…', icon: 'copy', run: () => (ssh.editing = { ...host, id: '', alias: `${host.alias} (copie)` }) },
      { separator: true },
      { label: 'Supprimer…', icon: 'trash', danger: true, run: () => void remove(host) },
    ]);
  }

  onMount(() => void ssh.load());
</script>

<div class="ssh">
  <aside class="book" aria-label="Carnet d’hôtes">
    <header>
      <span class="label">Hôtes · {ssh.hosts.length}</span>
      <button class="btn small" type="button" onclick={() => (ssh.editing = emptyHost())}><Icon name="plus" size={14} /> Ajouter un hôte</button>
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
                <span class="tint" style:--c={host.color ? `var(--l${host.color})` : 'var(--faint)'}></span>
                <span class="text">
                  <b>{host.alias}</b>
                  <small class="mono">{address(host)}</small>
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
        <Icon name="pull" size={14} /> Importer ~/.ssh/config
      </button>
      <button class="btn small" type="button" onclick={() => (ssh.showKeys = true)}><Icon name="tag" size={14} /> Gérer les clés</button>
    </footer>
  </aside>

  <div class="work">
    {#if ssh.sessions.length}
      <div class="tabs" role="tablist" aria-label="Terminaux ouverts">
        {#each ssh.sessions as session (session.key)}
          {@const current = session.key === ssh.activeKey}
          <div class="tab" class:current animate:flip={{ duration: motion.med }} in:scale={{ duration: motion.med, start: 0.9 }}>
            <button
              class="open"
              type="button"
              role="tab"
              aria-selected={current}
              onclick={() => (ssh.activeKey = session.key)}
              onauxclick={(e) => {
                if (e.button === 1) ssh.close(session.key);
              }}
            >
              <!-- L'état de la session est dit en mots, pas seulement par la couleur du point. -->
              <span
                class="dot s-{session.status}"
                role="img"
                aria-label={STATUS[session.status]}
                title={STATUS[session.status]}
                style:--c={session.host.color ? `var(--l${session.host.color})` : 'var(--ok)'}
              ></span>
              <span class="name">{session.title}</span>
            </button>
            <button class="close" type="button" aria-label="Fermer le terminal {session.title}" title="Fermer le terminal" onclick={() => ssh.close(session.key)}>
              <Icon name="x" size={14} />
            </button>
          </div>
        {/each}
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
        <p>Double-clique un hôte pour ouvrir un terminal.</p>
        <div class="row">
          <button class="btn primary" type="button" onclick={() => (ssh.editing = emptyHost())}><Icon name="plus" size={14} /> Ajouter un hôte</button>
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
  li:hover,
  li:focus-within {
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
  li:focus-within .acts {
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
  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
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
    background: var(--hover);
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
  .dot.s-connecting {
    background: var(--warn);
    animation: blink var(--t-wait) var(--ease) infinite;
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
