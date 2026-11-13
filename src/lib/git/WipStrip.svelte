<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import type { Repo } from './repo.svelte';

  /**
   * The working directory has uncommitted changes: said at the top of the detail panel, where the
   * eye is after a click, whatever the panel shows (guidelines, section 9.22). The "WIP" row of the
   * graph says it too, but it scrolls out of sight.
   */
  let { repo }: { repo: Repo } = $props();
  const numbers = new Intl.NumberFormat('en-US');
  const files = $derived(`${numbers.format(repo.wipCount)} file${repo.wipCount === 1 ? '' : 's'}`);
</script>

<div class="strip">
  <span class="glyph"><Icon name="pencil" size={16} /></span>
  <span class="text">{files} with uncommitted changes</span>
  <button class="btn small" type="button" onclick={() => repo.showWip()}>Show working changes</button>
</div>

<style>
  .strip {
    /* The whole width of the card, on two lines: the icon and the sentence, then the button,
       aligned with the sentence. A card as narrow as the detail panel would wrap them anyway:
       two lines keep the button in one place, whatever the width. */
    align-self: stretch;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--ink);
  }
  .glyph {
    display: grid;
    color: var(--accent);
  }
  .text {
    font-weight: var(--fw-strong);
  }
  .strip .btn {
    grid-column: 2;
    justify-self: start;
  }
</style>
