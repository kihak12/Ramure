<script lang="ts">
  // A pane of the Files module: one folder of one place, with its path, its rows and its foot
  // (guidelines, 9.24).
  import { onDestroy, tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { menu, type MenuItem } from '../ui/menu.svelte';
  import { focusWithRing } from '../ui/focus';
  import { ssh, STATUS } from '../ssh/ssh.svelte';
  import { count } from '../ssh/types';
  import ConnectPanel from './ConnectPanel.svelte';
  import LocationPicker from './LocationPicker.svelte';
  import { files, MAX_PANES, visible, type Pane, type SortBy } from './files.svelte';
  import { fullDate, joinPath, nameOf, parentPath, segments, shortDate, size, type Entry } from './types';

  let { pane }: { pane: Pane } = $props();

  const rows = $derived(visible(pane));
  const crumbs = $derived(segments(pane.path));
  const hiddenCount = $derived(pane.hidden ? 0 : pane.entries.filter((e) => e.hidden).length);
  const label = $derived(files.label(pane));
  const host = $derived(files.host(pane));
  const ready = $derived(pane.location?.kind === 'local' || pane.phase.step === 'ready');
  const selected = $derived(new Set(pane.selected));
  const selectedBytes = $derived(rows.filter((e) => selected.has(e.name) && !e.dir).reduce((n, e) => n + e.size, 0));
  /** Once connected, the header of a host with a color takes a tint of it (guidelines, 9.24). */
  const tint = $derived(host?.color && pane.phase.step === 'ready' ? `var(--l${host.color})` : null);

  /** The parent row, "..": the current row when it is chosen, never an item (guidelines, 9.24). */
  const UP = '..';
  const parent = $derived(ready && !pane.error ? parentPath(pane.path) : null);
  const atParent = $derived(parent !== null && pane.current === UP);

  /**
   * Under 464 px of pane, the date gives way to the eye only: it stays spoken (guidelines, 9.24 and
   * 11). The width is that of the pane, not of the window: two panes of a window at its minimum
   * keep their dates.
   */
  const NARROW = 464;
  let paneWidth = $state(0);
  const narrow = $derived(paneWidth > 0 && paneWidth < NARROW);

  /** The order of a column, in words: the chevron never says it alone (guidelines, 7 and 9.24). */
  const ORDER: Record<SortBy, [string, string]> = {
    name: ['A to Z', 'Z to A'],
    modified: ['oldest first', 'newest first'],
    size: ['smallest first', 'largest first'],
  };

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
  let crumbsEl = $state<HTMLElement>();
  // The end of a long path stays in view; a short one starts on the left (guidelines, 9.24).
  $effect(() => {
    void crumbs;
    void tick().then(() => {
      if (crumbsEl) crumbsEl.scrollLeft = crumbsEl.scrollWidth;
    });
  });
  let whereEl = $state<HTMLButtonElement>();
  let pathField = $state<HTMLInputElement>();
  let editing = $state(false);
  let typed = $state('');
  /** The pencil that opens the path field: Escape gives it the keyboard back (guidelines, 9.24). */
  let pencilEl = $state<HTMLButtonElement>();
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
    // The folder typed is shown: the list takes the keyboard, with its ring (11).
    void files.focus(pane, true);
  }

  async function pathKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      editing = false;
      // Nothing changed: the keyboard goes to the pencil, whatever opened the field, with its ring.
      await tick();
      focusWithRing(pencilEl);
    }
  }

  /** "Add pane": the new pane takes the keyboard, its ring shown when the key came from it. */
  function addPane(keyboard: boolean) {
    const added = files.add(pane);
    if (added) void files.focus(added, keyboard);
  }

  /** "Close pane": the keyboard goes to the pane that takes its place, as the focus would be lost. */
  async function closePane(keyboard: boolean) {
    // Canceled in its dialog, nothing closed: the keyboard stays where it was.
    if (!(await files.close(pane))) return;
    const next = files.pane(files.focused);
    if (next) void files.focus(next, keyboard);
  }

  // --- Selection ------------------------------------------------------------------------------

  const rowId = (index: number) => `pane-${pane.key}-row-${index}`;
  const upId = $derived(`pane-${pane.key}-row-up`);
  const currentIndex = $derived(pane.current === null ? -1 : rows.findIndex((e) => e.name === pane.current));

  function selectOnly(name: string) {
    pane.selected = [name];
    pane.current = name;
    pane.anchor = name;
  }

  function selectRange(to: string) {
    // From the parent row, which is never selected, the range starts at the first item (9.24).
    if (pane.anchor === null) pane.anchor = rows[0]?.name ?? to;
    const from = rows.findIndex((e) => e.name === pane.anchor);
    const end = rows.findIndex((e) => e.name === to);
    const [a, b] = from < end ? [from, end] : [end, from];
    pane.selected = rows.slice(Math.max(a, 0), b + 1).map((e) => e.name);
    pane.current = to;
  }

  /** The parent row becomes the current row; nothing is selected. */
  function toParentRow(scroll = true) {
    pane.current = UP;
    pane.anchor = null;
    pane.selected = [];
    if (scroll) void scrollToCurrent();
  }

  function click(e: MouseEvent, entry: Entry) {
    files.focused = pane.key;
    if (e.shiftKey) selectRange(entry.name);
    else if (e.ctrlKey || e.metaKey) {
      pane.selected = selected.has(entry.name) ? pane.selected.filter((n) => n !== entry.name) : [...pane.selected, entry.name];
      pane.current = entry.name;
      pane.anchor = entry.name;
    } else selectOnly(entry.name);
    takeKeyboard();
  }

  /**
   * A click on a row gives its list the keyboard, with no ring (guidelines, 9.6 and 9.24): a row
   * can be dragged, and WebKit does not focus the list when such a row is pressed.
   */
  function takeKeyboard() {
    if (document.activeElement !== listEl) listEl?.focus({ preventScroll: true });
  }

  /**
   * The last gesture was the mouse: the current row shows no ring until a key. WebKit would show
   * one after a click when the element left behind showed one, and none after an arrow when the
   * list was given the focus by a click (guidelines, 9.6 and 11).
   */
  let pointed = $state(false);
  const MODIFIERS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'AltGraph']);

  async function scrollToCurrent() {
    await tick();
    listEl?.querySelector<HTMLElement>(`#${atParent ? upId : rowId(currentIndex)}`)?.scrollIntoView({ block: 'nearest' });
  }

  function move(to: number, extend: boolean) {
    if (!rows.length) return;
    const index = Math.max(0, Math.min(rows.length - 1, to));
    const name = rows[index].name;
    if (extend) selectRange(name);
    else selectOnly(name);
    void scrollToCurrent();
  }

  /**
   * The stop of the tab order before the list. After a click, WebKit goes on from the row clicked,
   * which is inside the list: Shift + Tab would give the list the focus again, press after press.
   */
  function stopBefore(): HTMLElement | undefined {
    const stops = [...document.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], [tabindex]')].filter(
      (el) => el.tabIndex >= 0 && !el.matches(':disabled') && !el.closest('[inert], [hidden]') && el.getClientRects().length > 0,
    );
    const at = listEl ? stops.indexOf(listEl) : -1;
    return at > 0 ? stops[at - 1] : undefined;
  }

  function onListKey(e: KeyboardEvent) {
    // Tab by its code: WebKitGTK does not always name Shift + Tab.
    if (e.code === 'Tab' && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
      const to = stopBefore();
      if (to) {
        e.preventDefault();
        focusWithRing(to);
      }
      return;
    }
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
        else if (parent !== null && !e.shiftKey && currentIndex <= 0) toParentRow();
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
        if (parent !== null && !e.shiftKey) toParentRow();
        else move(0, e.shiftKey);
        return;
      case 'End':
        e.preventDefault();
        move(rows.length - 1, e.shiftKey);
        return;
      case 'Enter':
        if (atParent) {
          e.preventDefault();
          files.up(pane);
        } else if (entry?.dir) {
          e.preventDefault();
          files.open(pane, entry);
        } else if (entry) {
          // A file goes to the pane next to this one, with the selection when it is part of it.
          // Enter held down copies once.
          e.preventDefault();
          if (e.repeat) return;
          files.send(pane, selected.has(entry.name) ? [...pane.selected] : [entry.name]);
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
        if (band) {
          e.preventDefault();
          e.stopPropagation();
          cancelBand();
        } else if (pane.selected.length) {
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
    // A press on the empty part takes the selection away: the focus that follows keeps it so.
    if (band) return;
    if (pane.current === null && rows.length && !files.drag) {
      pane.current = rows[0].name;
      pane.anchor = rows[0].name;
      if (!pane.selected.length) pane.selected = [rows[0].name];
    }
  }

  // --- The rectangle that selects (guidelines, 9.24) -------------------------------------------

  /** In the coordinates of the content of the list, which scrolls under it. */
  let band = $state<{
    x0: number;
    y0: number;
    x: number;
    y: number;
    moved: boolean;
    /** What the rectangle adds to: the selection before it with Ctrl or Shift, nothing otherwise. */
    base: string[];
    /** What Escape puts back. */
    before: { selected: string[]; current: string | null; anchor: string | null };
    pointer: number;
    /** The height of the content of the list as the rectangle starts: it never goes past it. */
    limit: number;
  } | null>(null);
  let pointerY = 0;
  let scrolling = 0;

  /**
   * Where the pointer is, in the content of the list, kept inside it: a pointer that leaves the
   * list draws the rectangle to its edge, and neither widens nor lengthens what scrolls.
   */
  function point(e: PointerEvent, limit = listEl!.scrollHeight) {
    const box = listEl!.getBoundingClientRect();
    const x = Math.min(Math.max(e.clientX - box.left, 0), listEl!.clientWidth - 1) + listEl!.scrollLeft;
    const y = Math.min(Math.max(e.clientY - box.top + listEl!.scrollTop, 0), limit - 1);
    return { x, y };
  }

  /**
   * While the rectangle is drawn, nothing of the window is selected as text: the pointer may leave
   * the list for the other pane, its path or the foot (guidelines, 9.24).
   */
  function holdText(on: boolean) {
    const style = document.body.style as CSSStyleDeclaration & { webkitUserSelect: string };
    style.userSelect = on ? 'none' : '';
    style.webkitUserSelect = on ? 'none' : '';
    if (on) window.getSelection()?.removeAllRanges();
  }

  /** A press on the empty part takes the selection away and starts a rectangle; never on a row. */
  function pressList(e: PointerEvent) {
    if (e.button !== 0 || !listEl || !ready || (e.target instanceof Element && e.target.closest('.row'))) return;
    // A press on the scrollbar scrolls: it is outside the content.
    if (e.clientX - listEl.getBoundingClientRect().left >= listEl.clientWidth) return;
    files.focused = pane.key;
    const limit = listEl.scrollHeight;
    const at = point(e, limit);
    const adds = e.ctrlKey || e.metaKey || e.shiftKey;
    holdText(true);
    band = {
      x0: at.x,
      y0: at.y,
      x: at.x,
      y: at.y,
      moved: false,
      base: adds ? [...pane.selected] : [],
      before: { selected: [...pane.selected], current: pane.current, anchor: pane.anchor },
      pointer: e.pointerId,
      limit,
    };
    if (!adds) pane.selected = [];
    pointerY = e.clientY;
    listEl.setPointerCapture(e.pointerId);
  }

  function dragBand(e: PointerEvent) {
    if (!band || e.pointerId !== band.pointer) return;
    pointerY = e.clientY;
    const at = point(e, band.limit);
    band.x = at.x;
    band.y = at.y;
    if (!band.moved && Math.hypot(band.x - band.x0, band.y - band.y0) < 4) return;
    band.moved = true;
    pick();
    if (!scrolling) scrolling = requestAnimationFrame(follow);
  }

  /** The rows the rectangle touches, added to what was selected before it when it adds. */
  function pick() {
    if (!band || !listEl) return;
    const [top, bottom] = [Math.min(band.y0, band.y), Math.max(band.y0, band.y)];
    const [left, right] = [Math.min(band.x0, band.x), Math.max(band.x0, band.x)];
    const hit: string[] = [];
    for (const row of listEl.querySelectorAll<HTMLElement>('.row[data-index]')) {
      const inside = row.offsetTop < bottom && row.offsetTop + row.offsetHeight > top && row.offsetLeft < right && row.offsetLeft + row.offsetWidth > left;
      if (inside) hit.push(rows[Number(row.dataset.index)].name);
    }
    pane.selected = [...new Set([...band.base, ...hit])];
    if (hit.length) {
      pane.current = hit[band.y < band.y0 ? 0 : hit.length - 1];
      pane.anchor = pane.current;
    }
  }

  /** Near the top or the bottom of the list, it scrolls, and the rectangle follows. */
  function follow() {
    scrolling = 0;
    if (!band || !listEl) return;
    const box = listEl.getBoundingClientRect();
    const edge = 24;
    const step = pointerY < box.top + edge ? -Math.ceil((box.top + edge - pointerY) / 3) : pointerY > box.bottom - edge ? Math.ceil((pointerY - box.bottom + edge) / 3) : 0;
    if (!step) return;
    const before = listEl.scrollTop;
    listEl.scrollTop += step;
    band.y = Math.min(Math.max(band.y + listEl.scrollTop - before, 0), band.limit - 1);
    pick();
    scrolling = requestAnimationFrame(follow);
  }

  function endBand(e: PointerEvent) {
    if (!band || e.pointerId !== band.pointer) return;
    if (listEl?.hasPointerCapture(e.pointerId)) listEl.releasePointerCapture(e.pointerId);
    cancelAnimationFrame(scrolling);
    scrolling = 0;
    band = null;
    holdText(false);
  }

  // A pane closed, or left, while the rectangle is drawn gives the text of the window back.
  onDestroy(() => {
    if (band) holdText(false);
  });

  /** Escape while it is drawn: the selection goes back as it was. */
  function cancelBand() {
    if (!band) return;
    ({ selected: pane.selected, current: pane.current, anchor: pane.anchor } = band.before);
    if (listEl?.hasPointerCapture(band.pointer)) listEl.releasePointerCapture(band.pointer);
    cancelAnimationFrame(scrolling);
    scrolling = 0;
    band = null;
    holdText(false);
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
        icon: same ? 'folder' : 'copy',
        disabled: !!refusal,
        hint: refusal || undefined,
        run: () => void files.drop(dragged, other, other.path),
      });
    }
    return out;
  }

  /** The tooltip of a row: its name, and, opened in another program, what that means (9.24). */
  function rowTitle(entry: Entry): string {
    const watch = files.watchOf(pane, entry.name);
    if (!watch) return entry.name;
    return watch.pending
      ? `${entry.name} · Changed in another program, not sent to ${watch.label} yet.`
      : `${entry.name} · Opened in another program: its changes are sent when you answer.`;
  }

  function rowMenu(e: MouseEvent, entry: Entry) {
    if (!selected.has(entry.name)) selectOnly(entry.name);
    else pane.current = entry.name;
    files.focused = pane.key;
    // The menu closed, the keyboard comes back to the list, on the row asked for.
    takeKeyboard();
    const names = [...pane.selected];
    const single = names.length === 1;
    const send = sendItems(names);
    // A file opens in another program, from this machine for now (guidelines, 9.24, "Opening a file").
    const why = single ? undefined : 'Select one file to open it.';
    const watched = single ? files.watchOf(pane, entry.name) : undefined;
    const opening = entry.dir
      ? []
      : [
          { label: 'Open', icon: 'file' as const, disabled: !!why, hint: why, run: () => void files.openFile(pane, entry) },
          { label: 'Open with…', disabled: !!why, hint: why, run: () => void files.openWith(pane, entry) },
          ...(watched ? [{ label: 'Stop watching', icon: 'stop' as const, run: () => void files.stopWatching(watched) }] : []),
          { separator: true } as const,
        ];
    menu.open(e, [
      ...(single && entry.dir ? [{ label: 'Open', icon: 'folder' as const, run: () => files.open(pane, entry) }, { separator: true } as const] : []),
      ...opening,
      ...(send.length ? [...send, { separator: true } as const] : []),
      { label: 'Rename…', icon: 'pencil', keys: 'F2', disabled: !single, hint: single ? undefined : 'Select one item to rename it.', run: () => void files.rename(pane, entry) },
      { label: 'New folder…', icon: 'folder-plus', run: () => void files.mkdir(pane) },
      { label: 'New file…', icon: 'file-plus', run: () => void files.touch(pane) },
      { label: 'Copy path', icon: 'copy', disabled: !single, run: () => void files.copyPath(pane, entry) },
      { separator: true },
      { label: names.length === 1 ? 'Delete…' : `Delete ${names.length} items…`, icon: 'trash', keys: 'Delete', danger: true, run: () => void files.remove(pane, names) },
    ]);
  }

  function listMenu(e: MouseEvent) {
    if (e.target instanceof Element && e.target.closest('.row:not(.up)')) return;
    if (!ready || !pane.location) return;
    menu.open(e, [
      { label: 'New folder…', icon: 'folder-plus', run: () => void files.mkdir(pane) },
      { label: 'New file…', icon: 'file-plus', run: () => void files.touch(pane) },
      { label: 'Refresh', icon: 'refresh', keys: 'F5', run: () => void files.refresh(pane) },
      { label: pane.hidden ? 'Hide hidden files' : 'Show hidden files', icon: 'eye', run: () => toggleHidden() },
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

  /** Where the pointer would drop, and what that would do. `into`: the row that receives it;
   *  `named`: the folder the foot names. */
  let over = $state<{ dest: string; into: string | null; named: string | null; refusal: string; copy: boolean } | null>(null);

  // A drag that ends anywhere, canceled with Escape or dropped elsewhere, ends in the source pane
  // only: the other panes let go of where it would have landed.
  $effect(() => {
    if (!files.drag) over = null;
  });

  function dragStart(e: DragEvent, entry: Entry) {
    if (!selected.has(entry.name)) selectOnly(entry.name);
    takeKeyboard();
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

  /** Where the pointer of a drag aims in this pane: a folder row, or the folder the pane shows. */
  function aim(e: DragEvent) {
    const dragged = files.drag;
    if (!dragged || !pane.location) return null;
    const row = e.target instanceof Element ? e.target.closest<HTMLElement>('.row') : null;
    // The parent row receives the drop into the parent folder (guidelines, 9.24).
    if (row?.classList.contains('up') && parent !== null) {
      return { dragged, dest: parent, into: UP, named: nameOf(parent) || parent, refusal: files.refusal(dragged, pane, parent), copy: e.ctrlKey };
    }
    const entry = row ? rows[Number(row.dataset.index)] : undefined;
    // A folder row receives the drop into it, unless it is one of the rows dragged.
    const intoRow = entry?.dir && !(dragged.paneKey === pane.key && dragged.names.includes(entry.name));
    const dest = intoRow && entry ? joinPath(pane.path, entry.name) : pane.path;
    const into = intoRow && entry ? entry.name : null;
    return { dragged, dest, into, named: into, refusal: files.refusal(dragged, pane, dest), copy: e.ctrlKey };
  }

  function dragOver(e: DragEvent) {
    const aimed = aim(e);
    if (!aimed) return;
    const { dragged, refusal } = aimed;
    over = aimed;
    if (refusal) {
      if (e.dataTransfer) e.dataTransfer.dropEffect = 'none';
      return;
    }
    e.preventDefault();
    const source = files.pane(dragged.paneKey);
    const moves = !!source && files.samePlace(source, pane) && !e.ctrlKey;
    if (e.dataTransfer) e.dataTransfer.dropEffect = moves ? 'move' : 'copy';
  }

  /** The pointer left the pane: WebKit does not always say where it went, so its position decides. */
  function dragLeave(e: DragEvent) {
    const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const inside = e.clientX > box.left && e.clientX < box.right && e.clientY > box.top && e.clientY < box.bottom;
    if (!inside || (e.clientX === 0 && e.clientY === 0)) over = null;
  }

  /** The drop aims where the pointer is now, whatever the last `dragover` said. */
  function drop(e: DragEvent) {
    e.preventDefault();
    const aimed = aim(e);
    over = null;
    // The keyboard follows the files, into the pane that receives them; while the drag lasts, the
    // focus it gives selects no first row (9.24).
    if (aimed && !aimed.refusal) takeKeyboard();
    files.drag = null;
    if (!aimed || aimed.refusal) return;
    void files.drop(aimed.dragged, pane, aimed.dest, e.ctrlKey);
  }

  const dragging = $derived(files.drag?.paneKey === pane.key ? new Set(files.drag.names) : null);
  const footDrop = $derived(
    files.drag && over ? (over.refusal || files.verb(files.drag, pane, over.named, over.copy)) : '',
  );
</script>

<!-- A modifier alone (Shift, Ctrl held during a click) is not a use of the keyboard. -->
<svelte:window
  onpointerdowncapture={() => (pointed = true)}
  onkeydowncapture={(e) => {
    if (!MODIFIERS.has(e.key)) pointed = false;
  }}
/>

<section
  class="pane"
  class:active={files.focused === pane.key}
  class:narrow
  bind:clientWidth={paneWidth}
  data-pane={pane.key}
  aria-label={label}
  onfocusin={() => (files.focused = pane.key)}
  onpointerdown={() => (files.focused = pane.key)}
  ondragover={dragOver}
  ondragleave={dragLeave}
  ondrop={drop}
>
  <header class:tinted={!!tint} style:--c={tint}>
    {#if pane.location}
      <button
        class="where"
        type="button"
        bind:this={whereEl}
        aria-haspopup="menu"
        aria-expanded={menu.isUnder(whereEl)}
        title={menu.isUnder(whereEl) ? undefined : `${label}${host ? ` · ${files.address(pane)} · ${pane.phase.step === 'ready' ? 'Connected' : pane.phase.step === 'connecting' ? 'Connecting' : 'Not connected'}` : ''} — choose another location`}
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
        <span class="name">{label}</span>
        {#if host}<span class="address mono">{files.address(pane)}</span>{/if}
        {#if stateText}<span class="state">{stateText}</span>{/if}
        <!-- Turned down: it opens a menu, as a list does (guidelines, 9.3). -->
        <span class="down" aria-hidden="true"><Icon name="chevron" size={14} /></span>
      </button>
    {:else}
      <span class="title">Empty pane</span>
    {/if}
    <span class="acts">
      <button
        class="icon-btn"
        type="button"
        aria-label="Add pane"
        title={files.panes.length >= MAX_PANES ? 'Four panes at most. Close one first.' : 'Add pane'}
        disabled={files.panes.length >= MAX_PANES}
        onclick={(e) => addPane(e.detail === 0)}
      >
        <Icon name="plus" size={16} />
      </button>
      <button
        class="icon-btn"
        type="button"
        aria-label="Close pane"
        title={files.panes.length <= 2 && !pane.location ? 'Two panes always stay. Choose a location instead.' : 'Close pane'}
        disabled={files.panes.length <= 2 && !pane.location}
        onclick={(e) => void closePane(e.detail === 0)}
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
        <div class="crumbs" bind:this={crumbsEl} onclick={(e) => e.target === e.currentTarget && ready && void editPath()} title={ready ? 'Click to type a path' : undefined}>
          {#each crumbs as segment, i (segment.path)}
            <!-- The root ("/", "C:\") already ends with its separator. -->
            {#if i > 0 && !/[\\/]$/.test(crumbs[i - 1].name)}<span class="sep" aria-hidden="true">/</span>{/if}
            <button class="crumb" type="button" disabled={!ready} title={segment.path} onclick={() => void files.list(pane, segment.path)}>{segment.name}</button>
          {/each}
        </div>
      {/if}
      <button class="icon-btn" type="button" aria-label="Type a path" title="Type a path" disabled={!ready} bind:this={pencilEl} onclick={() => void editPath()}>
        <Icon name="pencil" size={16} />
      </button>
      <button class="icon-btn" type="button" aria-label="Refresh" title="Refresh (F5)" disabled={!ready} onclick={() => void files.refresh(pane)}>
        <Icon name="refresh" size={16} />
      </button>
      <button class="icon-btn toggle" type="button" aria-label="Show hidden files" title="Show hidden files" aria-pressed={pane.hidden} disabled={!ready} onclick={toggleHidden}>
        <Icon name="eye" size={16} />
      </button>
      <button class="icon-btn" type="button" aria-label="New folder…" title="New folder…" disabled={!ready} onclick={() => void files.mkdir(pane)}>
        <Icon name="folder-plus" size={16} />
      </button>
      <button class="icon-btn" type="button" aria-label="New file…" title="New file…" disabled={!ready} onclick={() => void files.touch(pane)}>
        <Icon name="file-plus" size={16} />
      </button>
    </div>
    {#if editing && pathError}<p class="path-error" id="path-error-{pane.key}" role="alert">{pathError}</p>{/if}

    <div
      class="body"
      class:target={!!over && !over.refusal && !over.into}
      role="presentation"
      oncontextmenu={listMenu}
    >
      <div class="cols" role="presentation">
        {#each [['name', 'Name'], ['modified', 'Modified'], ['size', 'Size']] as [by, title] (by)}
          {@const order = pane.sort.by === by ? ORDER[by as SortBy][pane.sort.desc ? 1 : 0] : ''}
          <button
            class="col {by}"
            type="button"
            onclick={() => sortBy(by as SortBy)}
            aria-label={order ? `Sort by ${title.toLowerCase()}, sorted ${order}` : `Sort by ${title.toLowerCase()}`}
            title={order ? `Sorted ${order}: click to reverse` : `Sort by ${title.toLowerCase()}`}
          >
            <span>{title}</span>
            {#if pane.sort.by === by}<span class="arrow" class:desc={pane.sort.desc} aria-hidden="true"><Icon name="chevron" size={14} /></span>{/if}
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
          class:pointed
          class:stale={pane.loading}
          role="listbox"
          tabindex="0"
          data-main={files.focused === pane.key ? '' : undefined}
          aria-label="Files of {label}"
          aria-multiselectable="true"
          aria-activedescendant={atParent ? upId : currentIndex >= 0 ? rowId(currentIndex) : undefined}
          aria-busy={pane.loading}
          inert={!ready}
          bind:this={listEl}
          onkeydown={onListKey}
          onfocus={onListFocus}
          onpointerdown={pressList}
          onpointermove={dragBand}
          onpointerup={endBand}
          onpointercancel={endBand}
          onlostpointercapture={endBand}
        >
          {#if parent !== null}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_interactive_supports_focus -->
            <div
              class="row up"
              class:current={atParent}
              class:drop={over?.into === UP && !over.refusal}
              id={upId}
              role="option"
              aria-selected="false"
              aria-label="Parent folder"
              title={parent}
              onclick={() => {
                files.focused = pane.key;
                toParentRow(false);
                takeKeyboard();
              }}
              ondblclick={() => files.up(pane)}
            >
              <span class="kind" aria-hidden="true"><Icon name="up" size={16} /></span>
              <span class="fname"><span class="label-text">..</span></span>
            </div>
          {/if}
          {#each rows as entry, i (entry.name)}
            {@const picked = selected.has(entry.name)}
            {@const watch = files.watchOf(pane, entry.name)}
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
              title={rowTitle(entry)}
              draggable="true"
              onclick={(e) => click(e, entry)}
              ondblclick={() => (entry.dir ? files.open(pane, entry) : files.send(pane, [entry.name]))}
              oncontextmenu={(e) => rowMenu(e, entry)}
              ondragstart={(e) => dragStart(e, entry)}
              ondragend={dragEnd}
            >
              <!-- Downloaded to be opened: the waiting ring in place of its icon (guidelines, 8 and 9.24). -->
              <span class="kind" class:dir={entry.dir} aria-hidden="true">
                {#if files.opening.includes(`${pane.key}:${joinPath(pane.path, entry.name)}`)}<span class="spinner"></span>{:else}<Icon name={entry.dir ? 'folder' : 'file'} size={16} />{/if}
              </span>
              <span class="fname">
                <span class="label-text">{entry.name}</span>
                {#if entry.kind === 'link'}<span class="link">link</span>{/if}
                <!-- Downloaded, or opened in another program: a word, never a ring or a color alone
                     (guidelines, 8 and 9.24). -->
                {#if files.opening.includes(`${pane.key}:${joinPath(pane.path, entry.name)}`)}
                  <span class="link" role="status">opening…</span>
                {:else if watch}
                  <span class="link" class:unsent={watch.pending}>{watch.pending ? 'not sent' : 'open'}</span>
                {/if}
              </span>
              <span class="modified" class:spoken={narrow} title={fullDate(entry.modified)}>{shortDate(entry.modified)}</span>
              <span class="size">{entry.dir ? '' : size(entry.size)}</span>
            </div>
          {:else}
            {#if !pane.loading && ready}<p class="empty">{pane.entries.length ? 'Only hidden files here.' : 'This folder is empty.'}</p>{/if}
          {/each}
          {#if band?.moved}
            <div
              class="band"
              aria-hidden="true"
              style:left="{Math.min(band.x0, band.x)}px"
              style:top="{Math.min(band.y0, band.y)}px"
              style:width="{Math.abs(band.x - band.x0)}px"
              style:height="{Math.abs(band.y - band.y0)}px"
            ></div>
          {/if}
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
    transition: background var(--t-fast) var(--ease);
  }
  /* The server is seen before it is read: a tint of the color of the host (guidelines, 9.24). */
  header.tinted {
    background: color-mix(in srgb, var(--c) var(--tint-veil), var(--panel));
  }
  header.tinted .address {
    color: var(--ink);
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
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .where:active {
    transform: scale(0.97);
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
    font-weight: var(--fw-strong);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .down {
    flex: none;
    display: grid;
    transform: rotate(90deg);
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
  /* The first segments give way on the left: the bar scrolls to the end of the path, with no
     scrollbar. */
  .crumbs {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    overflow-x: auto;
    scrollbar-width: none;
    height: 28px;
    cursor: text;
  }
  .crumb {
    flex: none;
    min-width: 24px;
    max-width: 180px;
    height: 24px;
    padding: 0 4px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    text-align: center;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .crumb:active:not(:disabled) {
    transform: scale(0.97);
  }
  .crumb:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  /* The bar scrolls sideways and would cut a ring drawn outside the segment: it is drawn inside. */
  .crumb:focus-visible {
    outline-offset: -2px;
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
    padding: 0 12px 6px 32px;
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
  /* The drop goes into the folder the pane shows (guidelines, 9.24): the tint of a hover, which
     stays apart from the selection in both themes (2.4). */
  .body.target {
    background: var(--hover);
  }
  .cols,
  .row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) 152px 72px;
    align-items: center;
    gap: 6px;
  }
  /* The header keeps the room of the scrollbar of the rows, shown or not: its columns stay over
     theirs (guidelines, 9.24). */
  .cols {
    height: 26px;
    margin: 0 6px;
    padding: 0 6px;
    border-bottom: 1px solid var(--line-soft);
    overflow: hidden;
    scrollbar-gutter: stable;
  }
  .col {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    height: 24px;
    padding: 0 4px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .col:hover {
    background: var(--hover);
    color: var(--ink);
  }
  /* The header clips what goes past it: the ring is drawn inside the button (11). */
  .col:focus-visible {
    outline-offset: -2px;
  }
  /* The words of the header start where the text of their column starts: past the icon for the
     name, and the 4 px of the button given back for the date and the size (guidelines, 9.24). */
  .col.name {
    grid-column: 1 / 3;
    padding-left: 28px;
  }
  .col.modified {
    margin-left: -4px;
  }
  .col.size {
    justify-content: flex-end;
    margin-right: -4px;
  }
  .arrow {
    display: grid;
    transform: rotate(-90deg);
  }
  .arrow.desc {
    transform: rotate(90deg);
  }
  /* Two rows of empty part under the last row: a full folder has one too, to start a rectangle
     or to take the selection away (guidelines, 9.24). */
  .rows {
    position: relative;
    overflow-y: auto;
    scrollbar-gutter: stable;
    min-height: 0;
    padding: 4px 6px 48px;
    outline: none;
  }
  .band {
    position: absolute;
    z-index: 1;
    border: 1px solid var(--accent);
    border-radius: var(--radius-xs);
    pointer-events: none;
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
  /* Rows selected one after another form one block: the corners round at its two ends only. A
     folder that receives a drop keeps its four, its outline standing out of the block
     (guidelines, 9.6). */
  .row.sel:not(.drop):has(+ .row.sel) {
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
  }
  .row.sel + .row.sel:not(.drop) {
    border-top-left-radius: 0;
    border-top-right-radius: 0;
  }
  /* The selection of a pane that is not the active one steps back to a neutral tint: the strong
     selection says which pane Enter, Delete, F2 and the double-click act on (guidelines, 9.24). */
  .pane:not(.active) .row.sel:not(.drop) {
    background: color-mix(in srgb, var(--muted) var(--tint-sel-quiet), var(--panel));
  }
  /* The row of the keyboard: an inner ring, never around the whole list (guidelines, 9.6). */
  .rows:focus:not(.pointed) .row.current,
  .rows:global(.focus-ring) .row.current {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  /* A folder that receives a drop: the outline of a branch label over the tint of the foot that
     says where it goes (guidelines, 9.4 and 9.24). */
  .row.drop {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .row.up .fname {
    color: var(--muted);
  }
  .row.dragged {
    opacity: 0.5;
  }
  .kind {
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  /* What opens in place: a folder, in the accent (guidelines, 2.3 and 9.24). */
  .kind.dir {
    color: var(--accent);
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
  .link.unsent {
    color: var(--warn);
  }
  /* On the selection, --warn does not read: the word says it in --muted (guidelines, 2.4). */
  .row.sel .link.unsent {
    color: var(--muted);
  }
  .modified {
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .size {
    text-align: right;
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  /* A narrow pane gives up the date first (guidelines, 9.24): the name keeps the same room as
     before the time was added to the date. The date of a row stays spoken (.spoken), its sort
     button goes: a stop of the tab order is never hidden from the eye. */
  .narrow .cols,
  .narrow .row {
    grid-template-columns: 22px minmax(0, 1fr) 72px;
  }
  .narrow .col.modified {
    display: none;
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
