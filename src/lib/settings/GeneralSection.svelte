<script lang="ts">
  import { DEFAULTS, prefs, savePrefs, type Palette, type ThemeChoice } from '../git/prefs.svelte';
  import Choice from '../ui/Choice.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import { theme } from './theme.svelte';
  import ThemePreview from './ThemePreview.svelte';

  const THEMES = [
    { value: 'system', label: 'Système' },
    { value: 'light', label: 'Clair' },
    { value: 'dark', label: 'Sombre' },
  ] as const satisfies readonly { value: ThemeChoice; label: string }[];

  // Chaque thème de couleurs dit ce qui le distingue ; Sous-bois et Braise disent aussi leur limite
  // (charte, section 2.5).
  const PALETTES = [
    { value: 'ardoise', label: 'Ardoise', help: 'Gris neutres, accent bleu. Par défaut.' },
    { value: 'encre', label: 'Encre', help: 'Gris presque noirs ou blancs, accent indigo.' },
    { value: 'sous-bois', label: 'Sous-bois', help: 'Verts sombres ou papier, accent mousse, comme la réussite.' },
    { value: 'braise', label: 'Braise', help: 'Charbon ou crème, accent ambre, proche de l’avertissement.' },
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
  title="Général"
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
  <h3 id="group-look">Apparence</h3>
  <SettingRow id="set-theme" name="Thème" help="« Système » suit le réglage clair ou sombre de ton ordinateur." group>
    <Choice name="set-theme" layout="tiles" labelledby="set-theme-name" describedby="set-theme-help" value={prefs.theme} options={THEMES} onchange={setTheme}>
      {#snippet preview(choice: ThemeChoice)}<ThemePreview {choice} palette={prefs.palette} />{/snippet}
    </Choice>
  </SettingRow>
  <SettingRow id="set-palette" name="Couleurs" help="Ne change que les couleurs : la disposition, les formes et le texte restent les mêmes." group>
    <Choice name="set-palette" layout="tiles" labelledby="set-palette-name" describedby="set-palette-help" value={prefs.palette} options={PALETTES} onchange={setPalette}>
      {#snippet preview(palette: Palette)}<ThemePreview choice={prefs.theme} {palette} />{/snippet}
    </Choice>
  </SettingRow>
</section>
