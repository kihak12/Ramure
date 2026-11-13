<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import type { Repo } from './repo.svelte';

  /**
   * The working directory has uncommitted changes: said at the top of the detail panel, where the
   * eye is after a click, whatever the panel shows (guidelines, section 9.22). The "WIP" row of the
   * graph says it too, but it scrolls out of sight.
   */
  let { repo }: { repo: Repo } = $props();
  const files = $derived(`${repo.wipCount} file${repo.wipCount === 1 ? '' : 's'}`);
</script>

<div class="strip">
  <span class="glyph"><Icon name="pencil" size={16} /></span>
  <span class="text">{files} with uncommitted changes</span>
  <button class="btn small" type="button" onclick={() => repo.showWip()}>Show working changes</button>
</div>

<style>
  .strip {
    /* The whole width of the card. When it is narrow, the button goes under the sentence. */
    align-self: stretch;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    padding: 6px 6px 6px 10px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--ink);
  }
  .glyph {
    display: grid;
    color: var(--accent);
  }
  .text {
    flex: 1 1 140px;
    min-width: 0;
    font-weight: var(--fw-strong);
  }
</style>
