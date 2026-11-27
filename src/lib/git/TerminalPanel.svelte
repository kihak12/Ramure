<script lang="ts">
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { focusBack, focusLost, focusWithRing, usable } from '../ui/focus';
  import { display, keys } from '../ui/keys.svelte';
  import Terminal from '../ssh/Terminal.svelte';
  import { mayCloseTerminal } from '../ssh/guard';
  import type { Session } from '../ssh/ssh.svelte';
  import type { Repo } from './repo.svelte';

  /**
   * The terminals of a repository, under the graph (guidelines, section 9.20). `active`: the view of
   * the repository is on screen and the panel is shown.
   */
  let { repo, active }: { repo: Repo; active: boolean } = $props();

  const terminals = $derived(repo.terminals);
  let panelEl = $state<HTMLElement>();
  let tabsEl = $state<HTMLElement>();
  // The active tab stays on screen when the tabs overflow (9.7), a new one too, once it is laid out.
  $effect(() => {
    void terminals.activeKey;
    void tick().then(() =>
      requestAnimationFrame(() => tabsEl?.querySelector('.tab.current')?.scrollIntoView({ block: 'nearest', inline: 'nearest' })),
    );
  });

  /** The tab list (9.7, 9.20): one stop of the tab order, the current tab; ← and → go to the
   *  previous and the next tab, Home and End to the first and the last, which show as they take
   *  the keyboard; Enter gives the keyboard to the terminal of the tab. */
  function tabKey(e: KeyboardEvent) {
    const tabs = terminals.sessions;
    const at = tabs.findIndex((s) => s.key === terminals.activeKey);
    if (e.key === 'Enter') {
      e.preventDefault();
      panelEl?.querySelector<HTMLElement>(`.session[data-session="${terminals.activeKey}"] .xterm-helper-textarea`)?.focus();
      return;
    }
    const to =
      e.key === 'ArrowRight' ? at + 1 : e.key === 'ArrowLeft' ? at - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : null;
    if (to === null || !tabs.length) return;
    e.preventDefault();
    const next = tabs[(to + tabs.length) % tabs.length];
    terminals.activate(next.key);
    // A key: the tab shows its ring, even when the list was reached with a click.
    void tick().then(() => focusWithRing(tabsEl?.querySelectorAll<HTMLElement>('[role="tab"]')[tabs.indexOf(next)]));
  }

  const combo = $derived(keys.binding('toggle-terminal'));
  const hideTitle = $derived(`Hide terminal${combo ? ` (${display(combo)})` : ''}`);
  const NEW = 'New terminal in this repository';

  /** The cross or the middle click: if a program runs in the terminal, ask first. */
  async function closeTab(session: Session) {
    if (!(await mayCloseTerminal(session))) return;
    terminals.close(session.key);
    // The cross pressed is gone: the keyboard goes to the tab shown now, or to the graph (11).
    await tick();
    if (!focusLost()) return;
    const tab = tabsEl?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
    focusBack(tab ?? usable('[role="listbox"][aria-label="Commit history"]'));
  }

  /** What a tab says on hover: the shell and its starting folder, which the header does not show. */
  function about(session: Session) {
    const what = session.status === 'closed' ? 'Local terminal stopped' : 'Local terminal';
    return session.opened ? `${what} · ${session.opened.shell} · opened in ${session.opened.cwd}` : what;
  }

  // A request for the keyboard (a shortcut): the active terminal takes it, once the panel is shown.
  $effect(() => {
    if (!terminals.focusAsk) return;
    void tick().then(() =>
      requestAnimationFrame(() => panelEl?.querySelector<HTMLElement>('.view.current .xterm-helper-textarea')?.focus()),
    );
  });

  // --- The graph follows (guidelines, section 9.20) ---------------------------------------------
  // After a command, the repository is read again: once the output has been quiet for 600 ms, at
  // most once every 2 s.
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

  // The panel hides: what happened in the terminal shows in the graph.
  let wasShown = false;
  $effect(() => {
    const shown = terminals.shown;
    if (wasShown && !shown) void reread();
    wasShown = shown;
  });
  $effect(() => () => window.clearTimeout(quietTimer));
</script>

<section class="panel" bind:this={panelEl} data-terminals aria-label="Repository terminal">
  <header>
    <div class="tabs" role="tablist" aria-label="Repository terminals" bind:this={tabsEl}>
      {#each terminals.sessions as session (session.key)}
        {@const current = session.key === terminals.activeKey}
        {@const stopped = session.status === 'closed'}
        <div class="tab" class:current>
          <button
            class="open"
            type="button"
            role="tab"
            aria-selected={current}
            tabindex={current ? 0 : -1}
            title={about(session)}
            onclick={() => terminals.activate(session.key)}
            onkeydown={tabKey}
            onauxclick={(e) => {
              // Middle click: closes the tab.
              if (e.button === 1) void closeTab(session);
            }}
          >
            <!-- A local terminal carries the computer, as everywhere “local” is said (guidelines, section 7). -->
            <span class="local" class:stopped role="img" aria-label={stopped ? 'Local terminal stopped' : 'Local terminal'}>
              <Icon name="laptop" size={14} />
            </span>
            <span class="name">{session.title}</span>
          </button>
          <!-- Only the cross of the current tab is a stop, after it (9.7). -->
          <button
            class="close"
            type="button"
            tabindex={current ? 0 : -1}
            aria-label="Close terminal {session.title}"
            title="Close terminal"
            onclick={() => void closeTab(session)}
          >
            <Icon name="x" size={14} />
          </button>
        </div>
      {/each}
      <button class="add" type="button" aria-label={NEW} title={NEW} onclick={() => terminals.open(repo.path)}>
        <Icon name="plus" size={16} />
      </button>
    </div>
    <button class="hide" type="button" aria-label="Hide terminal" title={hideTitle} onclick={() => terminals.hide()}>
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
  /* The 4 px of padding, given back by a negative margin, leave room for the focus ring of the
     tabs: without them, the horizontal scrolling would clip it. */
  .tabs {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    margin: 0 -4px;
    padding: 4px;
    overflow-x: auto;
    /* A tab that takes the focus scrolls until it is whole, its ring too, to the left of the "+" (9.7). */
    scroll-padding-inline: 4px 40px;
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
  /* The active tab already sits on a card: it reads by the background of the selection. */
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
    /* The ring of a tab follows its corners (9.7). */
    border-radius: var(--radius);
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
    transition:
      opacity var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .tab:hover .close,
  .tab.current .close,
  .close:focus-visible {
    opacity: 1;
  }
  /* A veil of --ink, not a hover token: it sits on a tab hovered or current (9.7). */
  .close:hover {
    background: color-mix(in srgb, var(--ink) 12%, transparent);
    color: var(--ink);
  }
  /* “+” after the last tab: it stays visible when the tabs overflow. */
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
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .add:hover,
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
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  /* When pressed, a button shrinks to 97% (guidelines, sections 8 and 9.1). */
  .close:active,
  .add:active,
  .hide:active {
    transform: scale(0.97);
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
