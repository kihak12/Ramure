<script lang="ts">
  // A pane of the Files module: one folder of one place, with its path, its rows and its foot
  // (guidelines, 9.24).
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu, type MenuItem } from '../ui/menu.svelte';
  import { ssh, STATUS } from '../ssh/ssh.svelte';
  import { count } from '../ssh/types';
  import ConnectPanel from './ConnectPanel.svelte';
  import LocationPicker from './LocationPicker.svelte';
  import { files, MAX_PANES, visible, type Pane, type SortBy } from './files.svelte';
  import { fullDate, joinPath, parentPath, segments, shortDate, size, type Entry } from './types';

  let { pane }: { pane: Pane } = $props();

  const rows = $derived(visible(pane));
  const hiddenCount = $derived(pane.hidden ? 0 : pane.entries.filter((e) => e.hidden).length);
  const label = $derived(files.label(pane));
  const host = $derived(files.host(pane));
  const ready = $derived(pane.location?.kind === 'local' || pane.phase.step === 'ready');
  const selected = $derived(new Set(pane.selected));
  const selectedBytes = $derived(rows.filter((e) => selected.has(e.name) && !e.dir).reduce((n, e) => n + e.size, 0));
  const focused = $derived(files.focused === pane.key);

  /** What the header says of the connection, as a fragment (guidelines, 12). */
  const stateText = $derived(
    pane.location?.kind !== 'host'
      ? ''
      : pane.phase.step === 'connecting'
        ? 'connecting…'
        : pane.phase.step === 'idle'
          ? 'not connected'
          : pane.phase.step === 'lost'
            ? 'connection lost'
            : pane.phase.step === 'ready'
              ? ''
              : 'not connected',
  );

  let listEl = $state<HTMLElement>();
  let whereEl = $state<HTMLButtonElement>();
  let pathField = $state<HTMLInputElement>();
  let editing = $state(false);
  let typed = $state('');
  let pathError = $state('');

  // --- The location ---------------------------------------------------------------------------

  function locations(): MenuItem[] {
    return [
      { label: 'This machine', icon: 'laptop', run: () => files.choose(pane, { kind: 'local' }) },
      ...(ssh.hosts.length ? [{ separator: true } as const] : []),
      ...ssh.hosts.map((h) => ({
        label: h.alias,
        icon: 'cloud' as const,
        hint: h.proxyJump ? 'This host goes through a bastion: not supported yet' : undefined,
        disabled: h.proxyJump,
        run: () => files.choose(pane, { kind: 'host', hostId: h.id }),
      })),
    ];
  }

  // --- The path bar ---------------------------------------------------------------------------

  async function editPath() {
    typed = pane.path;
    pathError = '';
    editing = true;
    await tick();
    pathField?.select();
  }

  async function goTyped(e: SubmitEvent) {
    e.preventDefault();
    const wanted = typed.trim();
    if (!wanted) return;
    const failed = await files.list(pane, wanted, undefined, undefined, true);
    if (failed) {
      pathError = failed;
      return;
    }
    editing = false;
    void files.focus(pane);
  }

  function pathKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      editing = false;
      void files.focus(pane);
    }
  }

  // --- Selection ------------------------------------------------------------------------------

  const rowId = (index: number) => `pane-${pane.key}-row-${index}`;
  const currentIndex = $derived(pane.current === null ? -1 : rows.findIndex((e) => e.name === pane.current));

  function selectOnly(name: string) {
    pane.selected = [name];
    pane.current = name;
    pane.anchor = name;
  }

  function selectRange(to: string) {
    const from = rows.findIndex((e) => e.name === (pane.anchor ?? to));
    const end = rows.findIndex((e) => e.name === to);
    const [a, b] = from < end ? [from, end] : [end, from];
    pane.selected = rows.slice(Math.max(a, 0), b + 1).map((e) => e.name);
    pane.current = to;
  }

  function click(e: MouseEvent, entry: Entry) {
    files.focused = pane.key;
    if (e.shiftKey) selectRange(entry.name);
    else if (e.ctrlKey || e.metaKey) {
      pane.selected = selected.has(entry.name) ? pane.selected.filter((n) => n !== entry.name) : [...pane.selected, entry.name];
      pane.current = entry.name;
      pane.anchor = entry.name;
    } else selectOnly(entry.name);
  }

  async function scrollToCurrent() {
    await tick();
    listEl?.querySelector<HTMLElement>(`#${rowId(currentIndex)}`)?.scrollIntoView({ block: 'nearest' });
  }

  function move(to: number, extend: boolean) {
    if (!rows.length) return;
    const index = Math.max(0, Math.min(rows.length - 1, to));
    const name = rows[index].name;
    if (extend) selectRange(name);
    else selectOnly(name);
    void scrollToCurrent();
  }

  function onListKey(e: KeyboardEvent) {
    const entry = currentIndex >= 0 ? rows[currentIndex] : null;
    const page = Math.max(1, Math.floor((listEl?.clientHeight ?? 240) / 24) - 1);
    switch (e.key) {
      case 'ArrowDown':
        if (e.altKey) return;
        e.preventDefault();
        move(currentIndex + 1, e.shiftKey);
        return;
      case 'ArrowUp':
        e.preventDefault();
        if (e.altKey) files.up(pane);
        else move(currentIndex < 0 ? 0 : currentIndex - 1, e.shiftKey);
        return;
      case 'PageDown':
        e.preventDefault();
        move(currentIndex + page, e.shiftKey);
        return;
      case 'PageUp':
        e.preventDefault();
        move(currentIndex - page, e.shiftKey);
        return;
      case 'Home':
        e.preventDefault();
        move(0, e.shiftKey);
        return;
      case 'End':
        e.preventDefault();
        move(rows.length - 1, e.shiftKey);
        return;
      case 'Enter':
        if (entry?.dir) {
          e.preventDefault();
          files.open(pane, entry);
        }
        return;
      case 'Backspace':
        e.preventDefault();
        files.up(pane);
        return;
      case 'F2':
        if (entry) {
          e.preventDefault();
          void files.rename(pane, entry);
        }
        return;
      case 'Delete':
        if (pane.selected.length) {
          e.preventDefault();
          void files.remove(pane, [...pane.selected]);
        }
        return;
      case 'F5':
        e.preventDefault();
        void files.refresh(pane);
        return;
      case ' ':
        if (entry) {
          e.preventDefault();
          pane.selected = selected.has(entry.name) ? pane.selected.filter((n) => n !== entry.name) : [...pane.selected, entry.name];
          pane.anchor = entry.name;
        }
        return;
      case 'Escape':
        if (pane.selected.length) {
          e.preventDefault();
          pane.selected = [];
        }
        return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      pane.selected = rows.map((r) => r.name);
    }
  }

  /** A list that takes the focus with no current row starts on the first (guidelines, 9.6). */
  function onListFocus() {
    files.focused = pane.key;
    if (pane.current === null && rows.length && !files.drag) {
      pane.current = rows[0].name;
      pane.anchor = rows[0].name;
      if (!pane.selected.length) pane.selected = [rows[0].name];
    }
  }

  // --- Menus ----------------------------------------------------------------------------------

  /** "Copy to …" and "Move to …": the keyboard way to do what dragging does (guidelines, 9.24). */
  function sendItems(names: string[]): MenuItem[] {
    const out: MenuItem[] = [];
    for (const other of files.panes) {
      if (other.key === pane.key || !files.place(other)) continue;
      const same = files.samePlace(pane, other);
      const dragged = { paneKey: pane.key, names };
      const refusal = files.refusal(dragged, other, other.path);
      out.push({
        label: `${same ? 'Move' : 'Copy'} to ${files.label(other)} · ${other.path}`,
        icon: same ? undefined : 'copy',
        disabled: !!refusal,
        hint: refusal || undefined,
        run: () => void files.drop(dragged, other, other.path),
      });
    }
    return out;
  }

  function rowMenu(e: MouseEvent, entry: Entry) {
    if (!selected.has(entry.name)) selectOnly(entry.name);
    else pane.current = entry.name;
    files.focused = pane.key;
    const names = [...pane.selected];
    const single = names.length === 1;
    const send = sendItems(names);
    menu.open(e, [
      ...(single && entry.dir ? [{ label: 'Open', icon: 'folder' as const, run: () => files.open(pane, entry) }, { separator: true } as const] : []),
      ...(send.length ? [...send, { separator: true } as const] : []),
      { label: 'Rename…', icon: 'pencil', keys: 'F2', disabled: !single, hint: single ? undefined : 'Select one item to rename it.', run: () => void files.rename(pane, entry) },
      { label: 'New folder…', icon: 'folder-plus', run: () => void files.mkdir(pane) },
      { label: 'Copy path', icon: 'copy', disabled: !single, run: () => void files.copyPath(pane, entry) },
      { separator: true },
      { label: names.length === 1 ? 'Delete…' : `Delete ${names.length} items…`, icon: 'trash', keys: 'Delete', danger: true, run: () => void files.remove(pane, names) },
    ]);
  }

  function listMenu(e: MouseEvent) {
    if (e.target instanceof Element && e.target.closest('.row')) return;
    if (!ready || !pane.location) return;
    menu.open(e, [
      { label: 'New folder…', icon: 'folder-plus', run: () => void files.mkdir(pane) },
      { label: 'Refresh', icon: 'refresh', keys: 'F5', run: () => void files.refresh(pane) },
      { label: 'Show hidden files', checked: pane.hidden, run: () => toggleHidden() },
    ]);
  }

  function toggleHidden() {
    pane.hidden = !pane.hidden;
    files.save();
  }

  function sortBy(by: SortBy) {
    pane.sort = pane.sort.by === by ? { by, desc: !pane.sort.desc } : { by, desc: false };
    files.save();
  }

  // --- Dragging -------------------------------------------------------------------------------

  /** Where the pointer would drop, and what that would do. */
  let over = $state<{ dest: string; into: string | null; refusal: string; copy: boolean } | null>(null);

  function dragStart(e: DragEvent, entry: Entry) {
    if (!selected.has(entry.name)) selectOnly(entry.name);
    files.drag = { paneKey: pane.key, names: [...pane.selected] };
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'copyMove';
      e.dataTransfer.setData('text/plain', pane.selected.map((n) => joinPath(pane.path, n)).join('\n'));
    }
  }

  function dragEnd() {
    files.drag = null;
    over = null;
  }

  function dragOver(e: DragEvent) {
    const dragged = files.drag;
    if (!dragged || !pane.location) return;
    const row = e.target instanceof Element ? e.target.closest<HTMLElement>('.row') : null;
    const entry = row ? rows[Number(row.dataset.index)] : undefined;
    // A folder row receives the drop into it, unless it is one of the rows dragged.
    const intoRow = entry?.dir && !(dragged.paneKey === pane.key && dragged.names.includes(entry.name));
    const dest = intoRow && entry ? joinPath(pane.path, entry.name) : pane.path;
    const refusal = files.refusal(dragged, pane, dest);
    over = { dest, into: intoRow && entry ? entry.name : null, refusal, copy: e.ctrlKey };
    if (refusal) {
      if (e.dataTransfer) e.dataTransfer.dropEffect = 'none';
      return;
    }
    e.preventDefault();
    const source = files.pane(dragged.paneKey);
    const moves = !!source && files.samePlace(source, pane) && !e.ctrlKey;
    if (e.dataTransfer) e.dataTransfer.dropEffect = moves ? 'move' : 'copy';
  }

  function dragLeave(e: DragEvent) {
    const into = e.relatedTarget;
    if (!(into instanceof Node) || !e.currentTarget || !(e.currentTarget as Node).contains(into)) over = null;
  }

  function drop(e: DragEvent) {
    e.preventDefault();
    const dragged = files.drag;
    const target = over;
    over = null;
    files.drag = null;
    if (!dragged || !target || target.refusal) return;
    void files.drop(dragged, pane, target.dest, e.ctrlKey);
  }

  const dragging = $derived(files.drag?.paneKey === pane.key ? new Set(files.drag.names) : null);
  const footDrop = $derived(
    files.drag && over ? (over.refusal || files.verb(files.drag, pane, over.into, over.copy)) : '',
  );
</script>

<section
  class="pane"
  data-pane={pane.key}
  aria-label={label}
  onfocusin={() => (files.focused = pane.key)}
  onpointerdown={() => (files.focused = pane.key)}
>
  <header>
    {#if pane.location}
      <button
        class="where"
        type="button"
        bind:this={whereEl}
        aria-haspopup="menu"
        aria-expanded={menu.isUnder(whereEl)}
        title={menu.isUnder(whereEl) ? undefined : `${label}${host ? ` · ${files.address(pane)}` : ''} — choose another location`}
        onclick={(e) => menu.openUnder(e.currentTarget, locations, e.detail === 0)}
      >
        {#if pane.location.kind === 'local'}
          <Icon name="laptop" size={14} />
        {:else if pane.phase.step === 'connecting'}
          <span class="spinner" role="img" aria-label={STATUS.connecting}></span>
        {:else}
          <span
            class="dot"
            class:on={pane.phase.step === 'ready'}
            role="img"
            aria-label={pane.phase.step === 'ready' ? 'Connected' : 'Not connected'}
            style:--c={host?.color ? `var(--l${host.color})` : 'var(--ok)'}
          ></span>
        {/if}
        <span class="name" class:strong={focused}>{label}</span>
        {#if host}<span class="address mono">{files.address(pane)}</span>{/if}
        {#if stateText}<span class="state">{stateText}</span>{/if}
        <Icon name="chevron" size={14} />
      </button>
    {:else}
      <span class="title" class:strong={focused}>Empty pane</span>
    {/if}
    <span class="acts">
      <button
        class="icon-btn"
        type="button"
        aria-label="Add pane"
        title={files.panes.length >= MAX_PANES ? 'Four panes at most. Close one first.' : 'Add pane'}
        disabled={files.panes.length >= MAX_PANES}
        onclick={() => files.add(pane)}
      >
        <Icon name="plus" size={16} />
      </button>
      <button
        class="icon-btn"
        type="button"
        aria-label="Close pane"
        title={files.panes.length === 1 && !pane.location ? 'The last pane stays' : 'Close pane'}
        disabled={files.panes.length === 1 && !pane.location}
        onclick={() => void files.close(pane)}
      >
        <Icon name="x" size={16} />
      </button>
    </span>
  </header>

  {#if !pane.location}
    <LocationPicker {pane} />
  {:else}
    <div class="pathbar">
      <button class="icon-btn" type="button" aria-label="Up" title="Up (Backspace)" disabled={!ready || parentPath(pane.path) === null} onclick={() => files.up(pane)}>
        <Icon name="up" size={16} />
      </button>
      {#if editing}
        <form class="typed" onsubmit={goTyped}>
          <input
            class="field mono"
            class:invalid={!!pathError}
            bind:this={pathField}
            bind:value={typed}
            spellcheck="false"
            autocomplete="off"
            aria-label="Folder path"
            aria-invalid={pathError ? 'true' : undefined}
            aria-describedby={pathError ? `path-error-${pane.key}` : undefined}
            onkeydown={pathKey}
            oninput={() => (pathError = '')}
            onblur={(e) => {
              if (!pathError && !(e.relatedTarget instanceof Node && e.currentTarget.form?.contains(e.relatedTarget))) editing = false;
            }}
          />
        </form>
      {:else}
        <!-- A click past the segments types a path (guidelines, 9.24). -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="crumbs" onclick={(e) => e.target === e.currentTarget && ready && void editPath()} title={ready ? 'Click to type a path' : undefined}>
          {#each segments(pane.path) as segment, i (segment.path)}
            {#if i > 0}<span class="sep" aria-hidden="true">/</span>{/if}
            <button class="crumb" type="button" disabled={!ready} title={segment.path} onclick={() => void files.list(pane, segment.path)}>{segment.name}</button>
          {/each}
        </div>
      {/if}
      <button class="icon-btn" type="button" aria-label="Type a path" title="Type a path" disabled={!ready} onclick={() => void editPath()}>
        <Icon name="pencil" size={16} />
      </button>
      <button class="icon-btn" type="button" aria-label="Refresh" title="Refresh (F5)" disabled={!ready} onclick={() => void files.refresh(pane)}>
        <Icon name="refresh" size={16} />
      </button>
      <button class="icon-btn toggle" type="button" aria-label="Show hidden files" title="Show hidden files" aria-pressed={pane.hidden} onclick={toggleHidden}>
        <Icon name="eye" size={16} />
      </button>
      <button class="icon-btn" type="button" aria-label="New folder…" title="New folder…" disabled={!ready} onclick={() => void files.mkdir(pane)}>
        <Icon name="folder-plus" size={16} />
      </button>
    </div>
    {#if editing && pathError}<p class="path-error" id="path-error-{pane.key}" role="alert">{pathError}</p>{/if}

    <div
      class="body"
      class:target={!!over && !over.refusal && !over.into}
      role="presentation"
      ondragover={dragOver}
      ondragleave={dragLeave}
      ondrop={drop}
      oncontextmenu={listMenu}
    >
      <div class="cols" role="presentation">
        {#each [['name', 'Name'], ['modified', 'Modified'], ['size', 'Size']] as [by, title] (by)}
          <button class="col {by}" type="button" onclick={() => sortBy(by as SortBy)} aria-label="Sort by {title.toLowerCase()}">
            <span>{title}</span>
            {#if pane.sort.by === by}<span class="arrow" class:desc={pane.sort.desc} aria-hidden="true"><Icon name="chevron" size={12} /></span>{/if}
          </button>
        {/each}
      </div>
      {#if pane.error}
        <div class="problem">
          <p><Icon name="warning" size={16} /> <span role="alert">{pane.error}</span></p>
          {#if parentPath(pane.path) !== null}<button class="btn small" type="button" onclick={() => files.up(pane)}>Go up</button>{/if}
        </div>
      {:else}
        <div
          class="rows"
          class:stale={pane.loading}
          role="listbox"
          tabindex="0"
          aria-label="Files of {label}"
          aria-multiselectable="true"
          aria-activedescendant={currentIndex >= 0 ? rowId(currentIndex) : undefined}
          aria-busy={pane.loading}
          bind:this={listEl}
          onkeydown={onListKey}
          onfocus={onListFocus}
        >
          {#each rows as entry, i (entry.name)}
            {@const picked = selected.has(entry.name)}
            <!-- The list keeps the focus and points to its current row (guidelines, 9.6). -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_interactive_supports_focus -->
            <div
              class="row"
              class:sel={picked}
              class:current={pane.current === entry.name}
              class:drop={over?.into === entry.name && !over.refusal}
              class:dragged={dragging?.has(entry.name)}
              id={rowId(i)}
              data-index={i}
              role="option"
              aria-selected={picked}
              title={entry.name}
              draggable="true"
              onclick={(e) => click(e, entry)}
              ondblclick={() => files.open(pane, entry)}
              oncontextmenu={(e) => rowMenu(e, entry)}
              ondragstart={(e) => dragStart(e, entry)}
              ondragend={dragEnd}
            >
              <span class="kind" aria-hidden="true"><Icon name={entry.dir ? 'folder' : 'file'} size={16} /></span>
              <span class="fname">
                <span class="label-text">{entry.name}</span>
                {#if entry.kind === 'link'}<span class="link">link</span>{/if}
              </span>
              <span class="modified" title={fullDate(entry.modified)}>{shortDate(entry.modified)}</span>
              <span class="size">{entry.dir ? '' : size(entry.size)}</span>
            </div>
          {:else}
            {#if !pane.loading && ready}<p class="empty">{pane.entries.length ? 'Only hidden files here.' : 'This folder is empty.'}</p>{/if}
          {/each}
        </div>
      {/if}
      {#if pane.location.kind === 'host' && pane.phase.step !== 'ready'}
        <ConnectPanel {pane} />
      {/if}
    </div>

    <footer class:dropping={!!footDrop} class:refused={!!over?.refusal}>
      {#if footDrop}
        <span role="status">{footDrop}</span>
      {:else if pane.loading}
        <span class="spinner" aria-hidden="true"></span><span role="status">Loading…</span>
      {:else if ready && !pane.error}
        <span>
          {#if pane.selected.length}
            {pane.selected.length} of {count(rows.length, 'item')} selected{selectedBytes ? ` · ${size(selectedBytes)}` : ''}
          {:else}
            {count(rows.length, 'item')}
          {/if}
          {#if hiddenCount}· {hiddenCount} hidden{/if}
        </span>
      {/if}
    </footer>
  {/if}
</section>

<style>
  .pane {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--panel);
    container-type: inline-size;
  }
  header,
  .pathbar,
  .path-error,
  footer {
    flex: none;
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 35px;
    padding: 4px 6px 0 6px;
  }
  .where {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 6px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--ink);
    cursor: pointer;
    text-align: left;
    transition: background var(--t-fast) var(--ease);
  }
  .where:hover,
  .where[aria-expanded='true'] {
    background: var(--hover);
  }
  .where :global(.icon) {
    color: var(--muted);
  }
  .title {
    flex: 1;
    padding: 0 6px;
    color: var(--muted);
  }
  .name {
    flex: none;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .strong {
    font-weight: var(--fw-strong);
  }
  .address {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .state {
    flex: none;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .dot.on {
    background: var(--c);
  }
  .where .spinner {
    margin: -2px;
  }
  .acts {
    display: flex;
    gap: 2px;
  }
  .icon-btn {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
  }
  .icon-btn:active:not(:disabled) {
    transform: scale(0.97);
  }
  .icon-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  /* A toggle (guidelines, 9.1): pressed, the tint of the accent and its border. */
  .icon-btn.toggle[aria-pressed='true'] {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--ink);
  }
  .pathbar {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 36px;
    padding: 0 6px;
  }
  .crumbs {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    overflow: hidden;
    height: 28px;
    cursor: text;
  }
  /* The first segments give way on the left: the end of the path stays in view. */
  .crumbs > :first-child {
    margin-left: auto;
  }
  .crumb {
    flex: none;
    max-width: 180px;
    height: 24px;
    padding: 0 4px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: background var(--t-fast) var(--ease);
  }
  .crumb:last-child {
    color: var(--ink);
  }
  .crumb:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
  }
  .sep {
    flex: none;
    color: var(--faint);
  }
  .typed {
    flex: 1;
    min-width: 0;
  }
  .typed .field {
    height: 28px;
    padding: 3px 8px;
    font-size: var(--fs-code);
  }
  .path-error {
    margin: 0;
    padding: 0 12px 6px 36px;
    color: var(--bad);
    font-size: var(--fs-small);
  }
  .body {
    position: relative;
    flex: 1;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
    transition: background var(--t-fast) var(--ease);
  }
  /* The drop goes into the folder the pane shows (guidelines, 9.24). */
  .body.target {
    background: var(--accent-soft);
  }
  .cols,
  .row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) 96px 72px;
    align-items: center;
    gap: 6px;
  }
  .cols {
    height: 26px;
    margin: 0 6px;
    padding: 0 6px;
    border-bottom: 1px solid var(--line-soft);
  }
  .col {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    height: 24px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    cursor: pointer;
  }
  .col:hover {
    color: var(--ink);
  }
  .col.name {
    grid-column: 1 / 3;
    padding-left: 28px;
  }
  .col.size {
    justify-content: flex-end;
  }
  .arrow {
    display: grid;
    transform: rotate(-90deg);
  }
  .arrow.desc {
    transform: rotate(90deg);
  }
  .rows {
    overflow-y: auto;
    min-height: 0;
    padding: 4px 6px 6px;
    outline: none;
  }
  .rows.stale {
    opacity: 0.45;
    pointer-events: none;
  }
  .row {
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius);
    cursor: default;
    user-select: none;
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--sel);
  }
  /* The row of the keyboard: an inner ring, never around the whole list (guidelines, 9.6). */
  .rows:focus-visible .row.current,
  .rows:global(.focus-ring) .row.current {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  /* A folder that receives a drop: the outline of a branch label (guidelines, 9.4 and 9.24). */
  .row.drop {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .row.dragged {
    opacity: 0.5;
  }
  .kind {
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .fname {
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .label-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link {
    flex: none;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .modified {
    color: var(--muted);
    font-size: var(--fs-label);
    white-space: nowrap;
    overflow: hidden;
  }
  .size {
    text-align: right;
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  /* A narrow pane gives up the date first (guidelines, 9.24). */
  @container (max-width: 420px) {
    .cols,
    .row {
      grid-template-columns: 22px minmax(0, 1fr) 72px;
    }
    .modified,
    .col.modified {
      display: none;
    }
  }
  .empty {
    margin: 0;
    padding: 12px 6px;
    color: var(--muted);
  }
  .problem {
    display: grid;
    gap: 8px;
    justify-items: start;
    align-content: start;
    padding: 12px;
  }
  .problem p {
    margin: 0;
    display: flex;
    gap: 6px;
    align-items: flex-start;
    overflow-wrap: anywhere;
  }
  .problem p :global(.icon) {
    color: var(--bad);
    margin-top: 1px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 26px;
    padding: 0 12px;
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    border-top: 1px solid var(--line-soft);
    white-space: nowrap;
    overflow: hidden;
  }
  footer > span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* While a drop is aimed at the pane, its foot says what it would do (guidelines, 9.24). */
  footer.dropping {
    color: var(--ink);
    font-weight: var(--fw-strong);
    background: var(--accent-soft);
  }
  footer.dropping.refused {
    background: none;
    color: var(--muted);
    font-weight: var(--fw-regular);
  }
</style>
