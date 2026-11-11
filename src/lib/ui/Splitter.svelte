<script lang="ts">
  /**
   * Poignée de redimensionnement entre deux cartes (charte graphique, section 9.16).
   * Elle se glisse à la souris et se règle au clavier ; un double-clic, ou Entrée, revient à la
   * taille par défaut. Verticale entre deux cartes côte à côte (elle règle une largeur),
   * horizontale entre deux cartes l'une sous l'autre (elle règle une hauteur).
   */
  let {
    value,
    min,
    max,
    initial,
    direction = 1,
    orientation = 'vertical',
    label,
    onchange,
    oncommit,
  }: {
    /** Largeur actuelle de la carte réglée, en pixels. */
    value: number;
    min: number;
    max: number;
    /** Largeur par défaut. */
    initial: number;
    /** 1 : la carte est à gauche (ou au-dessus) de la poignée ; -1 : à droite (ou au-dessous). */
    direction?: 1 | -1;
    /** `vertical` : la poignée sépare deux cartes côte à côte ; `horizontal` : l'une sous l'autre. */
    orientation?: 'vertical' | 'horizontal';
    label: string;
    onchange: (value: number) => void;
    /** Fin du geste : c'est le moment d'enregistrer. */
    oncommit: () => void;
  } = $props();

  const STEP = 16; // un appui sur une flèche
  let origin: { at: number; value: number } | null = null;
  const horizontal = $derived(orientation === 'horizontal');
  const along = (e: PointerEvent) => (horizontal ? e.clientY : e.clientX);
  let dragging = $state(false);

  const clamp = (width: number) => Math.round(Math.min(max, Math.max(min, width)));

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault(); // pas de sélection de texte pendant le geste
    const handle = e.currentTarget as HTMLElement;
    handle.focus({ preventScroll: true }); // l'annulation ci-dessus empêche aussi la prise de focus
    handle.setPointerCapture(e.pointerId);
    origin = { at: along(e), value };
    dragging = true;
  }

  function move(e: PointerEvent) {
    if (origin) onchange(clamp(origin.value + (along(e) - origin.at) * direction));
  }

  function up() {
    if (!origin) return;
    origin = null;
    dragging = false;
    oncommit();
  }

  function set(width: number) {
    onchange(clamp(width));
    oncommit();
  }

  function key(e: KeyboardEvent) {
    // Une flèche déplace la poignée : vers la droite, elle élargit la carte de gauche et rétrécit
    // celle de droite ; vers le haut, elle agrandit la carte du dessous.
    const [back, forth] = horizontal ? ['ArrowUp', 'ArrowDown'] : ['ArrowLeft', 'ArrowRight'];
    const moves: Record<string, number> = {
      [back]: value - STEP * direction,
      [forth]: value + STEP * direction,
      Home: min,
      End: max,
      Enter: initial,
    };
    if (!(e.key in moves)) return;
    e.preventDefault();
    set(moves[e.key]);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging
  class:horizontal
  role="separator"
  aria-orientation={orientation}
  aria-label={label}
  aria-valuenow={Math.round(value)}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  title="Glisser pour redimensionner. Double-clic : {horizontal ? 'hauteur' : 'largeur'} par défaut."
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={() => set(initial)}
  onkeydown={key}
></div>

<style>
  .splitter {
    position: relative;
    min-height: 0;
    cursor: col-resize;
    touch-action: none;
  }
  /* Au repos, rien : le fond de la fenêtre sépare déjà les cartes. Le repère n'apparaît qu'au survol,
     au focus et pendant le geste. */
  .splitter::before {
    content: '';
    position: absolute;
    top: 12px;
    bottom: 12px;
    left: 50%;
    width: 2px;
    margin-left: -1px;
    border-radius: var(--radius-pill);
    background: var(--faint);
    opacity: 0;
    transition:
      opacity var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease);
  }
  .splitter:hover::before {
    opacity: 1;
  }
  /* Le focus se lit sur le repère lui-même, pas par un cadre autour de la poignée. */
  .splitter:focus-visible {
    outline: none;
  }
  .splitter.horizontal {
    min-height: 0;
    min-width: 0;
    cursor: row-resize;
  }
  .splitter.horizontal::before {
    top: 50%;
    bottom: auto;
    left: 12px;
    right: 12px;
    width: auto;
    height: 2px;
    margin-left: 0;
    margin-top: -1px;
  }
  .splitter.dragging::before,
  .splitter:focus-visible::before {
    opacity: 1;
    background: var(--accent);
  }
</style>
