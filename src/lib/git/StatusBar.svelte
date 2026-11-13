<script lang="ts">
  import { fly } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import type { Repo } from './repo.svelte';
  import { motion } from '../ui/motion';

  let { repo }: { repo: Repo } = $props();

  let showLog = $state(false);
  // Clock of the status bar: ages “2 min ago” without any other event.
  let now = $state(Date.now());

  const numFmt = new Intl.NumberFormat('en-US');
  const ms = (n: number) => `${numFmt.format(Math.round(n))} ms`;

  const result = $derived(repo.result);
  const last = $derived(repo.journal.at(-1) ?? null);
  const count = $derived(repo.commits.length - (repo.commits[0]?.wip ? 1 : 0));

  const fetchText = $derived.by(() => {
    if (!result || !repo.refs?.remoteNames.length) return '';
    if (!repo.settings.autoFetch) return 'Automatic fetch off';
    if (!repo.lastFetch) return `Automatic fetch every ${repo.settings.autoFetch} min`;
    const minutes = Math.floor((now - repo.lastFetch.at) / 60_000);
    const when = minutes < 1 ? 'just now' : `${minutes} min ago`;
    return repo.lastFetch.ok ? `Last fetch ${when}` : `Last fetch failed, ${when}`;
  });

  $effect(() => {
    const timer = window.setInterval(() => (now = Date.now()), 30_000);
    return () => window.clearInterval(timer);
  });
</script>

<footer class="status">
  <span class="branch mono">{#if repo.branch}<Icon name="branch" size={14} /> {repo.branch}{/if}</span>
  <button class="log-btn" type="button" aria-expanded={showLog} onclick={() => (showLog = !showLog)} disabled={!repo.journal.length}>
    {last ? `$ ${last.command}` : 'Command log'}
  </button>
  <div class="stats" aria-live="polite">
    {#if repo.busy}
      {@const doing = repo.switching ? `Switching to ${repo.switching}…` : 'Working…'}
      <span class="busy" title={doing}><span class="spinner" aria-hidden="true"></span><span class="doing">{doing}</span></span>
    {/if}
    {#if fetchText}<span class="fetch" class:bad={repo.lastFetch?.ok === false} title={fetchText}>{fetchText}</span>{/if}
    {#if repo.loading}
      <span>Loading…</span>
    {:else if result}
      <span><b>{numFmt.format(count)}</b> commit{count === 1 ? '' : 's'}{result.truncated ? ' (truncated)' : ''}</span>
      <span title="Time of git log, then of the lane layout, then total time until the graph is shown">
        git {ms(result.timings.gitMs)} · lanes {ms(result.timings.layoutMs)} · shown in <b>{ms(repo.perf.paint)}</b>
      </span>
      {#if repo.frames}
        <span title="Average time between two frames during the last scroll, and worst frame (99th percentile). 16.7 ms = 60 frames per second.">
          scrolling <b>{repo.frames.avg.toFixed(1)} ms</b>/frame, worst {repo.frames.worst.toFixed(0)} ms
        </span>
      {/if}
    {/if}
  </div>

  {#if showLog}
    <div class="log" transition:fly={{ y: 10, duration: motion.med }}>
      <span class="label">Command log · most recent first</span>
      {#each [...repo.journal].reverse() as entry, i (i)}
        <div class="entry mono" class:failed={!entry.ok}>
          <span>{ms(entry.ms)}</span><span>$ {entry.command}</span>
        </div>
      {/each}
    </div>
  {/if}
</footer>

<style>
  .status {
    flex: none;
    height: 26px;
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    color: var(--muted);
    font-size: var(--fs-small);
    min-width: 0;
  }
  .branch {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ink);
    white-space: nowrap;
  }
  .log-btn {
    flex: 0 1 auto;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
    min-height: 24px;
    padding: 2px 6px;
    border-radius: var(--radius-s);
    font-size: var(--fs-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    transition: background var(--t-fast) var(--ease);
  }
  .log-btn:hover:not(:disabled) {
    background: var(--hover-side);
    color: var(--ink);
  }
  .log-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .stats {
    margin-left: auto;
    display: flex;
    gap: 14px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    min-width: 0;
  }
  .busy {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    /* During the operation, this message is what matters: it does not give its place up to the timings. */
    flex: none;
  }
  /* When room runs short, this text is the one that is cut, with an ellipsis. */
  .fetch {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* A long branch name must not push the rest of the bar away. */
  .doing {
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .stats b {
    color: var(--ink);
    font-weight: var(--fw-strong);
  }
  .stats .bad {
    color: var(--bad);
  }
  .log {
    position: absolute;
    left: 10px;
    bottom: 30px;
    width: min(780px, calc(100vw - 80px));
    max-height: 300px;
    overflow: auto;
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    padding: 10px 12px;
    display: grid;
    gap: 4px;
    z-index: 20;
    box-shadow: var(--shadow-pop);
  }
  .entry {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    gap: 10px;
    font-size: var(--fs-small);
    color: var(--ink);
  }
  .entry span:first-child {
    color: var(--muted);
    text-align: right;
  }
  .entry span:last-child {
    overflow-wrap: anywhere;
  }
  .entry.failed span:last-child {
    color: var(--bad);
  }
</style>
