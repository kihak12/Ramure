<script lang="ts">
  import { tick } from 'svelte';
  import GeneralSection from './GeneralSection.svelte';
  import GitSection from './GitSection.svelte';
  import KeysSection from './KeysSection.svelte';
  import SshSection from './SshSection.svelte';
  import { SECTIONS, settingsPage } from './settings.svelte';

  // La page de réglages (charte, section 9.18) : la navigation à gauche, la section à droite.
  let nav = $state<HTMLElement>();

  // À l'ouverture, le focus arrive sur l'entrée de la section courante.
  $effect(() => {
    if (!settingsPage.open) return;
    void tick().then(() => nav?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus());
  });

  /** Les flèches passent d'une section à l'autre, et la section s'affiche aussitôt. */
  function onNavKey(event: KeyboardEvent) {
    const index = SECTIONS.findIndex((s) => s.id === settingsPage.section);
    const moves: Record<string, number> = { ArrowDown: index + 1, ArrowUp: index - 1, Home: 0, End: SECTIONS.length - 1 };
    if (!(event.key in moves)) return;
    event.preventDefault();
    const next = SECTIONS[Math.max(0, Math.min(SECTIONS.length - 1, moves[event.key]))];
    settingsPage.section = next.id;
    void tick().then(() => nav?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus());
  }
</script>

<div class="page">
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <nav class="nav" bind:this={nav} role="tablist" aria-label="Sections des réglages" aria-orientation="vertical" onkeydown={onNavKey}>
    <span class="label">Réglages</span>
    {#each SECTIONS as section (section.id)}
      {@const current = section.id === settingsPage.section}
      <button
        type="button"
        role="tab"
        id="settings-tab-{section.id}"
        aria-selected={current}
        aria-controls="settings-panel"
        tabindex={current ? 0 : -1}
        onclick={() => (settingsPage.section = section.id)}
      >
        {section.title}
      </button>
    {/each}
  </nav>

  <div class="content" id="settings-panel" role="tabpanel" aria-labelledby="settings-tab-{settingsPage.section}">
    <div class="column">
      {#if settingsPage.section === 'general'}
        <GeneralSection />
      {:else if settingsPage.section === 'git'}
        <GitSection />
      {:else if settingsPage.section === 'ssh'}
        <SshSection />
      {:else}
        <KeysSection />
      {/if}
    </div>
  </div>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 8px;
    height: 100%;
    padding: 8px;
  }
  .nav,
  .content {
    min-width: 0;
    min-height: 0;
    border-radius: var(--radius-card);
    background: var(--panel);
  }
  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 6px;
    overflow-y: auto;
  }
  .nav .label {
    padding: 0 8px 6px;
  }
  .nav button {
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .nav button:hover {
    background: var(--hover);
  }
  .nav button[aria-selected='true'] {
    background: var(--sel);
    font-weight: var(--fw-strong);
  }
  /* Focus au clavier : un anneau intérieur sur la ligne, comme dans toute liste (section 9.6). */
  .nav button:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .content {
    overflow-y: auto;
    container-type: inline-size;
  }
  .column {
    display: grid;
    gap: 20px;
    max-width: 640px;
    padding: 24px;
  }
  /* Un groupe de réglages : son titre, puis ses lignes, à 12 px l'une de l'autre. */
  .column :global(.group) {
    display: grid;
    gap: 12px;
  }
  .column :global(.group h3) {
    margin: 0;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  /* Un champ fait 32 px de haut (section 4.3) ; une liste les fait d'elle-même (ui/Select). */
  .column :global(.field) {
    height: 32px;
  }
</style>
