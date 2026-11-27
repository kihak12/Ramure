<script module lang="ts">
  /** "Retry" pressed: the transfers there before it, while the copy started again has not come.
   *  Kept out of the card, which goes while its last row is started again. */
  let retrying: { before: Set<string> } | null = null;
</script>

<script lang="ts">
  // The card of the transfers, under the panes: progress, cancel, result (guidelines, 9.24).
  import { tick } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { byKeyboard, focusBack } from '../ui/focus';
  import ProgressBar from '../ui/ProgressBar.svelte';
  import { count } from '../ssh/types';
  import { files, type Transfer } from './files.svelte';
  import { sizeOf } from './types';

  const WORDS: Record<Transfer['state'], string> = {
    waiting: 'To send',
    counting: 'Counting…',
    copying: 'Copying…',
    done: 'Copied',
    incomplete: 'Incomplete',
    failed: 'Failed',
    canceled: 'Canceled',
  };

  const running = (t: Transfer) => t.state === 'counting' || t.state === 'copying';

  let cardEl = $state<HTMLElement>();

  /**
   * The card keeps the keyboard (guidelines, 9.24 and 11). A button of a row is replaced when the
   * state of its transfer changes, later than the press ("Cancel transfer" becomes "Remove" once
   * the copy has stopped), or its row goes: whenever the card changes and the keyboard is then
   * nowhere, it goes to the last button of the same row, else of the row in its place, the next
   * then the previous; with no row left, to the active pane.
   */
  let last: { id: string; at: number } | null = null;
  /** The rows of the card, those on screen. */
  const rowsNow = () => [...(cardEl?.isConnected ? cardEl.querySelectorAll<HTMLElement>('li[data-transfer]') : [])];
  const lastButton = (row: HTMLElement | undefined) => (row ? [...row.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')].at(-1) : undefined);
  function noteFocus(event: FocusEvent) {
    const row = event.target instanceof Element ? event.target.closest<HTMLElement>('li[data-transfer]') : null;
    const rows = [...(cardEl?.querySelectorAll<HTMLElement>('li[data-transfer]') ?? [])];
    last = { id: row?.dataset.transfer ?? '', at: row ? rows.indexOf(row) : 0 };
  }
  function leaving(event: FocusEvent) {
    // Gone elsewhere with the keyboard or the mouse: the card no longer holds it.
    if (event.relatedTarget && !cardEl?.contains(event.relatedTarget as Node)) last = null;
  }
  function giveBack() {
    const active = document.activeElement;
    const lost = !active || active === document.body || !active.isConnected || (active as HTMLButtonElement).disabled;
    const rows = rowsNow();
    if (retrying) {
      // "Retry": the copy started again takes the row's place once its checks are done; the
      // keyboard waits for it, then goes to its "Cancel transfer".
      const before = retrying.before;
      const fresh = rows.find((r) => !before.has(r.dataset.transfer ?? ''));
      if (fresh) {
        retrying = null;
        if (lost) focusBack(lastButton(fresh));
        return;
      }
      if (lost) return;
      retrying = null;
    }
    if (!last || !lost) return;
    const row = rows.find((r) => r.dataset.transfer === last?.id) ?? rows[Math.min(last.at, rows.length - 1)];
    const target = lastButton(row);
    if (target) return focusBack(target);
    const pane = files.pane(files.focused);
    last = null;
    if (pane) void files.focus(pane, byKeyboard());
  }
  function retry(t: Transfer) {
    // The changes of a watched file are sent again in the same row: nothing to wait for.
    if (t.watch === undefined) {
      retrying = { before: new Set(files.transfers.filter((o) => o.id !== t.id).map((o) => String(o.id))) };
      // The copy started again lasts until it ends; one that is over with no row came to nothing
      // (it could not start, every name was skipped): the keyboard goes on as after a row that goes.
      void files.retry(t).finally(() => {
        if (!retrying) return;
        retrying = null;
        giveBack();
      });
      return;
    }
    void files.retry(t);
  }

  $effect(() => {
    if (!cardEl) return;
    // A card that comes back with the copy started again by "Retry" (its row was the last one).
    giveBack();
    const watch = new MutationObserver(() => giveBack());
    watch.observe(cardEl, { childList: true, subtree: true, attributes: true, attributeFilter: ['disabled'] });
    return () => watch.disconnect();
  });
  // The card goes with its last row (Remove, Clear finished): the keyboard goes to the active pane.
  $effect(() => () => {
    if (!last || retrying) return;
    const pane = files.pane(files.focused);
    void tick().then(() => {
      const active = document.activeElement;
      if ((!active || active === document.body) && pane) void files.focus(pane, byKeyboard());
    });
  });

  /** The bar takes the color of the result once it has ended (guidelines, 9.24). */
  const TONE = { waiting: 'warn', counting: 'accent', copying: 'accent', done: 'ok', incomplete: 'warn', failed: 'bad', canceled: 'muted' } as const;
  const finished = $derived(files.transfers.some((t) => !running(t) && t.state !== 'waiting'));

  const percent = (t: Transfer) =>
    t.state === 'counting' || t.state === 'waiting' ? null : t.progress.bytesTotal ? Math.min(100, (t.progress.bytesDone / t.progress.bytesTotal) * 100) : t.state === 'done' ? 100 : 0;

  /** "18.4 of 60 MB · 12 of 40 files". */
  function amounts(t: Transfer): string {
    const p = t.progress;
    if (t.state === 'waiting') return '';
    if (t.state === 'counting') return p.filesTotal ? count(p.filesTotal, 'file') : '';
    return `${sizeOf(p.bytesDone, p.bytesTotal)} · ${p.filesDone} of ${count(p.filesTotal, 'file')}`;
  }

  /** What a transfer that ended badly says under its row. */
  function details(t: Transfer): { text: string; bad: boolean }[] {
    if (t.state === 'failed') return [{ text: t.error, bad: true }];
    const s = t.summary;
    if (!s) return [];
    const out: { text: string; bad: boolean }[] = [];
    if (s.failed.length) {
      const first = s.failed[0];
      out.push({ text: `${count(s.failed.length, 'item')} could not be copied. ${first.path}: ${first.reason}`, bad: false });
    }
    if (s.skipped.length) {
      out.push({ text: `${s.skipped.join(', ')}: left out, already there.`, bad: false });
    }
    if (s.links.length) {
      out.push({
        text: `${count(s.links.length, 'link')} to a folder ${s.links.length === 1 ? 'was' : 'were'} not followed: ${s.links.slice(0, 3).join(', ')}${s.links.length > 3 ? '…' : ''}`,
        bad: false,
      });
    }
    return out;
  }
</script>

<!-- A click elsewhere, on a part that takes no focus: the card no longer holds the keyboard. -->
<svelte:window
  onpointerdowncapture={(e) => {
    if (!cardEl?.contains(e.target as Node)) last = null;
  }}
/>

<section class="transfers" aria-label="Transfers" bind:this={cardEl} onfocusin={noteFocus} onfocusout={leaving}>
  <header>
    <span class="label count">Transfers · {files.transfers.length}</span>
    <button class="btn ghost small" type="button" disabled={!finished} onclick={() => files.clearFinished()}>Clear finished</button>
  </header>
  <ul>
    {#each files.transfers as t (t.id)}
      {@const more = details(t)}
      <!-- A result to act on sits on a tinted box (guidelines, 9.24). -->
      <li data-transfer={t.id} class:warn={t.state === 'incomplete' || t.state === 'waiting'} class:bad={t.state === 'failed'}>
        <div class="row">
          <span class="state s-{t.state}">
            {#if running(t)}
              <span class="spinner" aria-hidden="true"></span>
            {:else}
              <Icon name={t.state === 'done' ? 'check' : t.state === 'canceled' ? 'stop' : t.state === 'waiting' ? 'up' : 'warning'} size={14} />
            {/if}
            <!-- The state is announced when it changes: "Copied" is the answer (guidelines, 11). -->
            <span role="status">{WORDS[t.state]}</span>
          </span>
          <span class="what" title="{t.route}, into {t.dest}{t.relayed ? '. The files go through this machine.' : ''}">
            <span class="name">{t.title}</span>
            <span class="route"
              >{#each [t.from, t.to] as end, i (i)}{#if i}{' → '}{/if}{#if end.color}<span class="dot" style:--c="var(--l{end.color})" aria-hidden="true"></span>{/if}{end.label}{/each}</span
            >
          </span>
          {#if t.state === 'waiting'}
            <!-- Changes of a file opened elsewhere: sent only on this answer (guidelines, 9.24). -->
            <span class="bar"></span>
            <!-- Not in the cell of the amounts, which clips: the ring of the button stays whole (11). -->
            <span class="send"
              ><button class="btn small" type="button" data-send={t.id} aria-label="Send changes of {t.title}" onclick={() => void files.sendChanges(t)}
                >Send changes</button
              ></span
            >
          {:else}
            <span class="bar"><ProgressBar percent={percent(t)} tone={TONE[t.state]} /></span>
            <span class="amounts">{amounts(t)}</span>
          {/if}
          {#if t.state === 'waiting'}
            <button class="icon-btn" type="button" aria-label="Stop watching {t.title}" title="Stop watching" onclick={() => void files.stopWatchingOf(t)}>
              <Icon name="stop" size={14} />
            </button>
          {:else if running(t)}
            <button class="icon-btn" type="button" aria-label="Cancel transfer of {t.title}" title="Cancel transfer" onclick={() => files.cancel(t)}>
              <Icon name="stop" size={14} />
            </button>
          {:else if t.state === 'failed'}
            <button class="icon-btn" type="button" aria-label="Retry transfer of {t.title}" title="Retry" onclick={() => retry(t)}>
              <Icon name="refresh" size={14} />
            </button>
          {:else}
            <button class="icon-btn" type="button" aria-label="Remove transfer of {t.title}" title="Remove" onclick={() => files.dismiss(t)}>
              <Icon name="x" size={14} />
            </button>
          {/if}
        </div>
        {#each more as line, i (i)}
          <p class="more" class:bad={line.bad} role={line.bad ? 'alert' : undefined}>{line.text}</p>
        {/each}
      </li>
    {/each}
  </ul>
</section>

<style>
  .transfers {
    display: flex;
    flex-direction: column;
    min-height: 0;
    max-height: 132px;
    border-radius: var(--radius-card);
    background: var(--panel);
    overflow: hidden;
    animation: fade-in var(--t-med) var(--ease);
  }
  .count {
    font-variant-numeric: tabular-nums;
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 12px 2px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 12px 8px;
    overflow-y: auto;
    min-height: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr) 120px minmax(0, max-content) 24px;
    align-items: center;
    gap: 12px;
    min-height: 28px;
  }
  .state {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .state :global(.icon) {
    color: var(--muted);
  }
  .s-done :global(.icon) {
    color: var(--ok);
  }
  .s-incomplete :global(.icon),
  .s-waiting :global(.icon) {
    color: var(--warn);
  }
  .s-failed :global(.icon) {
    color: var(--bad);
  }
  .what {
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    white-space: nowrap;
  }
  .name {
    flex: none;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .route {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  /* The dot of a host, as on the header of its pane (guidelines, 9.24). */
  .route .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin: 0 4px 0 2px;
    border-radius: 50%;
    background: var(--c);
  }
  li {
    margin: 0 -8px 2px;
    padding: 0 8px;
    border-radius: var(--radius);
  }
  li.warn {
    background: var(--warn-soft);
  }
  li.bad {
    background: var(--bad-soft);
  }
  .send {
    display: flex;
  }
  .amounts {
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .icon-btn:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .icon-btn:active {
    transform: scale(0.97);
  }
  .more {
    margin: 0 0 4px 122px;
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .more.bad {
    color: var(--bad);
  }
</style>
