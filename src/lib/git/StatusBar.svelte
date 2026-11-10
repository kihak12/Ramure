<script lang="ts">
  import { fly } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import type { Repo } from './repo.svelte';
  import { motion } from '../ui/motion';

  let { repo }: { repo: Repo } = $props();

  let showJournal = $state(false);
  // Horloge de la barre d'état : fait vieillir « il y a 2 min » sans autre événement.
  let now = $state(Date.now());

  const numFmt = new Intl.NumberFormat('fr-FR');
  const ms = (n: number) => `${numFmt.format(Math.round(n))} ms`;

  const result = $derived(repo.result);
  const last = $derived(repo.journal.at(-1) ?? null);
  const count = $derived(repo.commits.length - (repo.commits[0]?.wip ? 1 : 0));

  const fetchText = $derived.by(() => {
    if (!result || !repo.refs?.remoteNames.length) return '';
    if (!repo.settings.autoFetch) return 'Fetch auto désactivé';
    if (!repo.lastFetch) return `Fetch auto toutes les ${repo.settings.autoFetch} min`;
    const minutes = Math.floor((now - repo.lastFetch.at) / 60_000);
    const when = minutes < 1 ? "à l'instant" : `il y a ${minutes} min`;
    return repo.lastFetch.ok ? `Dernier fetch ${when}` : `Dernier fetch en échec, ${when}`;
  });

  $effect(() => {
    const timer = window.setInterval(() => (now = Date.now()), 30_000);
    return () => window.clearInterval(timer);
  });
</script>

<footer class="status">
  <span class="branch mono">{#if repo.branch}<Icon name="branch" size={13} /> {repo.branch}{/if}</span>
  <button class="journal-btn mono" type="button" aria-expanded={showJournal} onclick={() => (showJournal = !showJournal)} disabled={!repo.journal.length}>
    {last ? `$ ${last.command}` : 'Journal des commandes'}
  </button>
  <div class="stats" aria-live="polite">
    {#if fetchText}<span class:bad={repo.lastFetch?.ok === false}>{fetchText}</span>{/if}
    {#if repo.loading}
      <span>Chargement…</span>
    {:else if result}
      <span><b>{numFmt.format(count)}</b> commit{count > 1 ? 's' : ''}{result.truncated ? ' (tronqué)' : ''}</span>
      <span title="Durée de git log, calcul des lanes, puis temps total jusqu'à l'affichage">
        git {ms(result.timings.gitMs)} · lanes {ms(result.timings.layoutMs)} · affiché en <b>{ms(repo.perf.paint)}</b>
      </span>
      {#if repo.frames}
        <span title="Temps moyen entre deux images pendant le dernier défilement, et pire image (99e centile). 16,7 ms = 60 images par seconde.">
          défilement <b>{repo.frames.avg.toFixed(1)} ms</b>/image, pire {repo.frames.worst.toFixed(0)} ms
        </span>
      {/if}
    {/if}
  </div>

  {#if showJournal}
    <div class="journal" transition:fly={{ y: 10, duration: motion.med }}>
      <span class="label">Journal des commandes · les plus récentes d'abord</span>
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
    border-top: 1px solid var(--line);
    background: var(--side);
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
  .journal-btn {
    flex: 0 1 auto;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--radius-s);
    font-size: var(--fs-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    transition: background var(--t-fast) var(--ease);
  }
  .journal-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
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
  .stats b {
    color: var(--ink);
    font-weight: var(--fw-strong);
  }
  .stats .bad {
    color: var(--bad);
  }
  .journal {
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
