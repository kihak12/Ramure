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

  /** True when the SSH module is on screen. */
  let { active = false }: { active?: boolean } = $props();

  // The active tab stays on screen when the tabs overflow: the bar scrolls to it, and the scroll
  // padding keeps it to the left of the "+".
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
  // The hosts come sorted by group: they are cut into sections.
  const sections = $derived.by(() => {
    const out: { group: string; hosts: Host[] }[] = [];
    for (const host of shown) {
      const last = out.at(-1);
      if (last && last.group === host.group) last.hosts.push(host);
      else out.push({ group: host.group, hosts: [host] });
    }
    return out;
  });

  // The hosts whose session is opening: their row says so at once, where the user has just acted
  // (guidelines, section 9.15). The state comes from the session, not from the terminal: it does
  // not wait for the terminal to mount.
  const waiting = $derived(
    new Map(
      ssh.sessions
        .filter((s) => s.kind === 'ssh' && (s.status === 'connecting' || s.status === 'reconnecting'))
        .map((s) => [s.host.id, s.status]),
    ),
  );

  const address = (host: Host) => addressOf(host.user || ssh.defaultUser, host.hostname, host.port);


  // The "+" of the tabs: a local terminal, in one click. The tooltip says the shortcut.
  const newLocal = $derived(`New local terminal${keys.binding('terminal-new') ? ` (${display(keys.binding('terminal-new'))})` : ''}`);

  /** The cross or the middle click: if a program runs in a local terminal, ask first. */
  async function closeTab(session: Session) {
    if (await mayCloseTerminal(session)) ssh.close(session.key);
  }

  /** The menu of the row: the deletion asked for, then the focus to the filter of the book. */
  async function removeHost(host: Host) {
    if (await ssh.removeAsked(host)) await ssh.focusBook();
  }

  function hostMenu(event: MouseEvent, host: Host) {
    menu.open(event, [
      { label: 'Open terminal', icon: 'terminal', disabled: host.proxyJump, run: () => ssh.open(host) },
      { separator: true },
      { label: 'Edit…', icon: 'pencil', run: () => (ssh.editing = host) },
      { label: 'Duplicate…', icon: 'copy', run: () => (ssh.editing = { ...host, id: '', alias: `${host.alias} (copy)` }) },
      { label: 'Export to ~/.ssh/config…', icon: 'export', run: () => (ssh.exporting = host.id) },
      { separator: true },
      { label: 'Delete host…', icon: 'trash', danger: true, run: () => void removeHost(host) },
    ]);
  }

  onMount(() => void ssh.load());
</script>

<div class="ssh">
  <aside class="book" aria-label="Hosts">
    <header>
      <span class="label">Hosts · {ssh.hosts.length}</span>
      <button class="btn small" type="button" onclick={() => (ssh.editing = emptyHost())}><Icon name="plus" size={14} /> Add host…</button>
    </header>
    <label class="search">
      <Icon name="search" size={14} />
      <input type="search" placeholder="Filter by name, address, group, tag" aria-label="Filter hosts" bind:value={filter} />
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
                title="{host.alias} — double-click to open a terminal, right-click for actions"
                ondblclick={() => !host.proxyJump && ssh.open(host)}
                onkeydown={(e) => {
                  if (e.key === 'Enter' && !host.proxyJump) ssh.open(host);
                }}
                oncontextmenu={(e) => hostMenu(e, host)}
              >
                <!-- The chosen color names the host; without a color, the place stays empty (guidelines, 9.21). -->
                <span class="tint" aria-hidden="true" style:--c={host.color ? `var(--l${host.color})` : 'transparent'}></span>
                <span class="text">
                  <b>{host.alias}</b>
                  <!-- While it connects, the wait takes the place of the address: the row keeps its height. -->
                  {#if wait}
                    <small class="wait"><span class="spinner" aria-hidden="true"></span><span>{STATUS[wait]}…</span></small>
                  {:else}
                    <small class="mono">{address(host)}</small>
                  {/if}
                  {#if host.tags.length || host.proxyJump}
                    <span class="tags">
                      {#if host.proxyJump}<span class="chip via" title="Jump through a bastion: not supported yet">bastion</span>{/if}
                      {#each host.tags as tag (tag)}<span class="chip">{tag}</span>{/each}
                    </span>
                  {/if}
                </span>
              </button>
              <span class="acts">
                <button type="button" aria-label="Open terminal on {host.alias}" title="Open terminal" disabled={host.proxyJump} onclick={() => ssh.open(host)}><Icon name="play" size={14} /></button>
                <button type="button" aria-label="Edit {host.alias}" title="Edit" onclick={() => (ssh.editing = host)}><Icon name="pencil" size={14} /></button>
              </span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="none">
          {#if !ssh.loaded}
            Loading hosts…
          {:else if ssh.hosts.length}
            No host matches “{filter}”.
          {:else}
            No hosts. Add one, or import those from ~/.ssh/config.
          {/if}
        </p>
      {/each}
    </div>

    <footer>
      <button class="btn small" type="button" onclick={() => void ssh.importConfig()} title="Add the hosts of ~/.ssh/config that are not listed yet">
        <Icon name="import" size={14} /> Import from ~/.ssh/config
      </button>
      <button class="btn small" type="button" onclick={() => (ssh.exporting = 'all')} title="Write the hosts to ~/.ssh/config, after showing what changes">
        <Icon name="export" size={14} /> Export to ~/.ssh/config…
      </button>
      <button class="btn small" type="button" onclick={() => (ssh.showKeys = true)}><Icon name="key" size={14} /> Manage keys…</button>
    </footer>
  </aside>

  <div class="work">
    {#if ssh.sessions.length}
      <div class="tabs" role="tablist" aria-label="Open terminals" bind:this={tabsEl}>
        {#each ssh.sessions as session (session.key)}
          {@const current = session.key === ssh.activeKey}
          <div class="tab" class:current animate:flip={{ duration: motion.med }} in:fade={{ duration: motion.med }}>
            <button
              class="open"
              type="button"
              role="tab"
              aria-selected={current}
              title={session.kind === 'local' ? (session.status === 'closed' ? 'Local terminal stopped' : 'Local terminal') : undefined}
              onclick={() => (ssh.activeKey = session.key)}
              onauxclick={(e) => {
                if (e.button === 1) void closeTab(session);
              }}
            >
              {#if session.kind === 'local'}
                <!-- A local terminal carries the computer icon, as everywhere "local" is said. -->
                <span
                  class="local"
                  class:stopped={session.status === 'closed'}
                  role="img"
                  aria-label={session.status === 'closed' ? 'Local terminal stopped' : 'Local terminal'}
                  title={session.status === 'closed' ? 'Local terminal stopped' : 'Local terminal'}
                >
                  <Icon name="laptop" size={14} />
                </span>
              {:else if session.status === 'connecting'}
                <!-- The connection is opening: the waiting ring takes the place of the dot. -->
                <span class="spinner" role="img" aria-label={STATUS.connecting} title={STATUS.connecting}></span>
              {:else}
                <!-- The state of the session is said in words, not only by the color of the dot. -->
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
                <!-- The log being recorded: an icon, said in words, with no color nor animation (9.7). -->
                <span class="journal" role="img" aria-label="Recording log" title="Recording log">
                  <Icon name="log" size={14} />
                </span>
              {/if}
            </button>
            <button class="close" type="button" aria-label="Close terminal {session.title}" title="Close terminal" onclick={() => void closeTab(session)}>
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
        <p>Double-click a host to open a remote terminal, or open a terminal on this machine.</p>
        <div class="row">
          <button class="btn primary" type="button" onclick={() => (ssh.editing = emptyHost())}><Icon name="plus" size={14} /> Add host…</button>
          <button class="btn" type="button" onclick={() => ssh.openLocal()}><Icon name="laptop" size={14} /> Open local terminal</button>
          {#if ssh.loaded && !ssh.hosts.length}
            <button class="btn" type="button" onclick={() => void ssh.importConfig()}>Import from ~/.ssh/config</button>
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
  /* The background follows the pointer, and the keyboard focus: a click leaves nothing behind it,
     even if the button of the row keeps the focus (guidelines, section 9.6). */
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
  /* 4 px around the tabs, given back by a negative margin: the room of the focus ring, which the
     horizontal scrolling would cut otherwise. */
  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    margin: -4px;
    padding: 4px;
    overflow-x: auto;
    /* A tab that takes the focus scrolls until it stays visible to the left of the "+", which sticks to the edge. */
    scroll-padding-inline-end: 40px;
    scrollbar-width: none;
  }
  /* "+" after the last tab (guidelines, section 9.7): it stays visible when the tabs overflow. */
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
    /* The 4 px of margin on its right keep the background of the bar: no tab shows past it. */
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
  /* 6 px between the icon and the name: the gap of the tab (8 px) minus 2. */
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
    /* The corners of the tab: the focus ring follows them. */
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
  /* The 12 px ring fits in the place of the 8 px dot: the title of the tab does not move. */
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
  /* When pressed, a button shrinks to 97% (guidelines, sections 8 and 9.1). */
  .close:active,
  .add:active {
    transform: scale(0.97);
  }
  .tab:hover .close,
  .tab.current .close,
  .close:focus-visible {
    opacity: 1;
  }
  /* On a hovered tab (--hover-side) or an active one (--panel), a veil of --ink shows on both, in
     light and in dark (guidelines, 9.7). */
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
