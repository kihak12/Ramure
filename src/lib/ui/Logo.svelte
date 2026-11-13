<script lang="ts">
  /**
   * Ramure's symbol: a deer's head in a single stroke (design guidelines, section 13, "Logo").
   * The drawing comes from `docs/logo`, its only source. It takes the color of the text, and
   * switches to the simplified drawing below 40 px.
   */
  import full from '../../../docs/logo/ramure-symbole.svg?raw';
  import small from '../../../docs/logo/ramure-symbole-petit.svg?raw';

  /** `label`: name read by a screen reader; without it, the symbol is decorative. */
  let { size = 32, label = '' }: { size?: number; label?: string } = $props();

  // The file carries its own title: only the drawing is kept, the accessible name comes from `label`.
  const drawing = $derived((size < 40 ? small : full).replace(/<title>.*?<\/title>/, ''));
</script>

<span class="logo" style:width="{size}px" style:height="{size}px" role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : 'true'}>
  {@html drawing}
</span>

<style>
  .logo {
    flex: none;
    display: inline-block;
  }
  .logo :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
