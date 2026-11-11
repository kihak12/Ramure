<script lang="ts">
  import { DEFAULTS, prefs, savePrefs, type ThemeChoice } from '../git/prefs.svelte';
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

  const changed = $derived(prefs.theme !== DEFAULTS.theme);

  function setTheme(choice: ThemeChoice) {
    savePrefs({ theme: choice });
    theme.apply();
  }
</script>

<SectionHeader
  title="Général"
  {changed}
  onreset={() => {
    const before = prefs.theme;
    setTheme(DEFAULTS.theme);
    return () => setTheme(before);
  }}
/>

<section class="group" aria-labelledby="group-look">
  <h3 id="group-look">Apparence</h3>
  <SettingRow id="set-theme" name="Thème" help="« Système » suit le réglage clair ou sombre de ton ordinateur." group>
    <Choice name="set-theme" layout="tiles" labelledby="set-theme-name" describedby="set-theme-help" value={prefs.theme} options={THEMES} onchange={setTheme}>
      {#snippet preview(choice: ThemeChoice)}<ThemePreview {choice} />{/snippet}
    </Choice>
  </SettingRow>
</section>
