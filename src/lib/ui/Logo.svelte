<script lang="ts">
  /**
   * Le symbole de Ramure : une tête de cerf d'un seul trait (charte graphique, section « Logo »).
   * Le dessin vient de `docs/logo`, sa seule source. Il prend la couleur du texte, et passe au
   * dessin simplifié en dessous de 40 px.
   */
  import full from '../../../docs/logo/ramure-symbole.svg?raw';
  import small from '../../../docs/logo/ramure-symbole-petit.svg?raw';

  /** `label` : nom lu par un lecteur d'écran ; sans lui, le symbole est décoratif. */
  let { size = 32, label = '' }: { size?: number; label?: string } = $props();

  // Le fichier porte son propre titre : on ne garde que le dessin, le nom accessible vient de `label`.
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
