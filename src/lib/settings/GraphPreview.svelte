<script lang="ts">
  import type { GraphStyle } from '../git/prefs.svelte';

  /** Aperçu d'un style de graphe : deux branches et quelques commits, dessinés avec les jetons. */
  let { style }: { style: GraphStyle } = $props();
  const ribbon = $derived(style === 'ribbon');
  // Commits : leur ligne (y) et leur voie (0 ou 1).
  const COMMITS = [
    { y: 12, lane: 0 },
    { y: 26, lane: 1 },
    { y: 40, lane: 0 },
    { y: 54, lane: 0 },
  ] as const;
  const x = (lane: number) => (ribbon ? 16 + lane * 18 : 12 + lane * 12);
</script>

<svg class="graph" class:ribbon viewBox="0 0 160 64" preserveAspectRatio="xMinYMid meet">
  <path class="l0" d="M{x(0)} 6V58" />
  <path class="l1" d="M{x(1)} 26C{x(1)} 40 {x(0)} 36 {x(0)} 50" />
  {#each COMMITS as commit (commit.y)}
    {#if ribbon}
      <rect class="band c{commit.lane}" x={x(commit.lane)} y={commit.y - 5} width={100 - commit.y} height="10" rx="5" />
    {:else}
      <path class="text" d="M{x(1) + 12} {commit.y}h{96 - commit.y}" />
    {/if}
    <circle class="node c{commit.lane}" cx={x(commit.lane)} cy={commit.y} r={ribbon ? 5 : 2.5} />
  {/each}
</svg>

<style>
  .graph {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--panel);
    fill: none;
    stroke-linecap: round;
  }
  .l0 {
    stroke: var(--l0);
  }
  .l1 {
    stroke: var(--l1);
  }
  path {
    stroke-width: 1.5;
  }
  .ribbon path {
    stroke-width: 2.5;
  }
  .node.c0 {
    fill: var(--l0);
  }
  .node.c1 {
    fill: var(--l1);
  }
  .band.c0 {
    fill: color-mix(in srgb, var(--l0) 18%, transparent);
  }
  .band.c1 {
    fill: color-mix(in srgb, var(--l1) 18%, transparent);
  }
  .text {
    stroke: var(--line);
    stroke-width: 3;
  }
</style>
