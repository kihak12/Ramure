<script lang="ts">
  import { DEFAULTS, prefs, savePrefs, type Palette, type ThemeChoice } from '../git/prefs.svelte';
  import Choice from '../ui/Choice.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import { theme } from './theme.svelte';
  import ThemePreview from './ThemePreview.svelte';

  const THEMES = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ] as const satisfies readonly { value: ThemeChoice; label: string }[];

  // Each color theme says what sets it apart; Undergrowth and Ember also say their limit
  // (guidelines, section 2.5).
  const PALETTES = [
    { value: 'slate', label: 'Slate', help: 'Neutral grays, blue accent. The default.' },
    { value: 'ink', label: 'Ink', help: 'Grays close to black or white, indigo accent.' },
    { value: 'undergrowth', label: 'Undergrowth', help: 'Dark green or paper, moss accent: the color of success.' },
    { value: 'ember', label: 'Ember', help: 'Charcoal or cream, amber accent, close to the warning color.' },
  ] as const satisfies readonly { value: Palette; label: string; help: string }[];

  const changed = $derived(prefs.theme !== DEFAULTS.theme || prefs.palette !== DEFAULTS.palette);

  function setTheme(choice: ThemeChoice) {
    savePrefs({ theme: choice });
    theme.apply();
  }

  function setPalette(palette: Palette) {
    savePrefs({ palette });
    theme.apply();
  }
</script>

<SectionHeader
  title="General"
  {changed}
  onreset={() => {
    const before = { theme: prefs.theme, palette: prefs.palette };
    setTheme(DEFAULTS.theme);
    setPalette(DEFAULTS.palette);
    return () => {
      setTheme(before.theme);
      setPalette(before.palette);
    };
  }}
/>

<section class="group" aria-labelledby="group-look">
  <h3 id="group-look">Appearance</h3>
  <SettingRow id="set-theme" name="Theme" help="“System” follows the light or dark setting of your computer." group>
    <Choice name="set-theme" layout="tiles" labelledby="set-theme-name" describedby="set-theme-help" value={prefs.theme} options={THEMES} onchange={setTheme}>
      {#snippet preview(choice: ThemeChoice)}<ThemePreview {choice} palette={prefs.palette} />{/snippet}
    </Choice>
  </SettingRow>
  <SettingRow id="set-palette" name="Colors" help="Changes the colors only: the layout, the shapes and the text stay the same." group>
    <Choice name="set-palette" layout="tiles" labelledby="set-palette-name" describedby="set-palette-help" value={prefs.palette} options={PALETTES} onchange={setPalette}>
      {#snippet preview(palette: Palette)}<ThemePreview choice={prefs.theme} {palette} />{/snippet}
    </Choice>
  </SettingRow>
</section>
