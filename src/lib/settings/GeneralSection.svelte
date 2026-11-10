<script lang="ts">
  import { DEFAULTS, prefs, savePrefs, type ThemeChoice } from '../git/prefs.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import { theme } from './theme.svelte';

  const changed = $derived(prefs.theme !== DEFAULTS.theme);

  function setTheme(choice: ThemeChoice) {
    savePrefs({ theme: choice });
    theme.apply();
  }
</script>

<SectionHeader title="Général" {changed} onreset={() => setTheme(DEFAULTS.theme)} />

<section class="group" aria-labelledby="group-look">
  <h3 id="group-look">Apparence</h3>
  <SettingRow id="set-theme" name="Thème" help="« Système » suit le réglage clair ou sombre de ton ordinateur.">
    <select
      id="set-theme"
      class="field"
      aria-describedby="set-theme-help"
      value={prefs.theme}
      onchange={(e) => setTheme(e.currentTarget.value as ThemeChoice)}
    >
      <option value="system">Système</option>
      <option value="light">Clair</option>
      <option value="dark">Sombre</option>
    </select>
  </SettingRow>
</section>
