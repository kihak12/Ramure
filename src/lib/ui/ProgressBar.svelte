<script lang="ts">
  /**
   * The bar of an operation that takes time: a clone, the download of an update (guidelines, 9.10
   * and 9.23). `percent` from 0 to 100 when the progress is known; `null`, and the bar sweeps.
   * Words beside it say what is happening: the bar itself is hidden from screen readers.
   */
  let { percent }: { percent: number | null } = $props();
</script>

<div class="bar" class:known={percent !== null} aria-hidden="true"><span style:width="{percent ?? 35}%"></span></div>

<style>
  .bar {
    position: relative;
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--line);
    overflow: hidden;
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: var(--radius-pill);
    background: var(--accent);
    animation: sweep var(--t-wait) var(--ease) infinite;
  }
  /* Known percentage: the bar moves forward instead of sweeping. */
  .bar.known span {
    animation: none;
    transition: width var(--t-med) var(--ease);
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
</style>
