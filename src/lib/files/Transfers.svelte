<script lang="ts">
  // The card of the transfers, under the panes: progress, cancel, result (guidelines, 9.24).
  import Icon from '../ui/Icon.svelte';
  import ProgressBar from '../ui/ProgressBar.svelte';
  import { count } from '../ssh/types';
  import { files, type Transfer } from './files.svelte';
  import { sizeOf } from './types';

  const WORDS: Record<Transfer['state'], string> = {
    counting: 'Counting…',
    copying: 'Copying…',
    done: 'Copied',
    incomplete: 'Incomplete',
    failed: 'Failed',
    canceled: 'Canceled',
  };

  const running = (t: Transfer) => t.state === 'counting' || t.state === 'copying';
  const finished = $derived(files.transfers.some((t) => !running(t)));

  const percent = (t: Transfer) =>
    t.state === 'counting' ? null : t.progress.bytesTotal ? Math.min(100, (t.progress.bytesDone / t.progress.bytesTotal) * 100) : t.state === 'copying' ? 0 : 100;

  /** "18.4 of 60 MB · 12 of 40 files". */
  function amounts(t: Transfer): string {
    const p = t.progress;
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

<section class="transfers" aria-label="Transfers">
  <header>
    <span class="label count">Transfers · {files.transfers.length}</span>
    <button class="btn ghost small" type="button" disabled={!finished} onclick={() => files.clearFinished()}>Clear finished</button>
  </header>
  <ul>
    {#each files.transfers as t (t.id)}
      {@const more = details(t)}
      <li>
        <div class="row">
          <span class="state s-{t.state}">
            {#if running(t)}
              <span class="spinner" aria-hidden="true"></span>
            {:else}
              <Icon name={t.state === 'done' ? 'check' : t.state === 'canceled' ? 'stop' : 'warning'} size={14} />
            {/if}
            <!-- The state is announced when it changes: "Copied" is the answer (guidelines, 11). -->
            <span role="status">{WORDS[t.state]}</span>
          </span>
          <span class="what" title="{t.route}, into {t.dest}{t.relayed ? '. The files go through this machine.' : ''}">
            <span class="name">{t.title}</span>
            <span class="route">{t.route}</span>
          </span>
          <span class="bar"><ProgressBar percent={percent(t)} /></span>
          <span class="amounts">{amounts(t)}</span>
          {#if running(t)}
            <button class="icon-btn" type="button" aria-label="Cancel transfer of {t.title}" title="Cancel transfer" onclick={() => files.cancel(t)}>
              <Icon name="stop" size={14} />
            </button>
          {:else if t.state === 'failed'}
            <button class="icon-btn" type="button" aria-label="Retry transfer of {t.title}" title="Retry" onclick={() => void files.retry(t)}>
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
  .s-incomplete :global(.icon) {
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
