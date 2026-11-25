<script lang="ts">
  import { DEFAULTS, prefs, savePrefs, type Clock, type DateFormat, type Palette, type ThemeChoice } from '../git/prefs.svelte';
  import Choice from '../ui/Choice.svelte';
  import { date, time } from '../ui/dates';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import { updates } from '../ui/updates.svelte';
  import { theme } from './theme.svelte';
  import ThemePreview from './ThemePreview.svelte';
  import UpdatesGroup from './UpdatesGroup.svelte';

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

  // Each format shows itself on the same moment, an afternoon, so that the clocks differ
  // (guidelines, 9.18).
  const SAMPLE = new Date(2026, 10, 25, 14, 30, 5).getTime();
  const DATE_FORMATS = (['mdy', 'dmy', 'ymd', 'text'] as const).map((value) => ({
    value,
    label: { mdy: 'MM-DD-YYYY', dmy: 'DD-MM-YYYY', ymd: 'YYYY-MM-DD', text: 'Written out' }[value],
    help: date(SAMPLE, value),
  })) satisfies { value: DateFormat; label: string; help: string }[];
  const CLOCKS = (['24', '12'] as const).map((value) => ({
    value,
    label: value === '24' ? '24-hour' : '12-hour',
    help: time(SAMPLE, value),
  })) satisfies { value: Clock; label: string; help: string }[];

  // The automatic check is a setting only on a copy that can update itself: elsewhere it is not shown.
  const checkShown = $derived(updates.info?.canUpdate ?? false);
  const changed = $derived(
    prefs.theme !== DEFAULTS.theme ||
      prefs.palette !== DEFAULTS.palette ||
      prefs.dateFormat !== DEFAULTS.dateFormat ||
      prefs.clock !== DEFAULTS.clock ||
      (checkShown && prefs.updateCheck !== DEFAULTS.updateCheck),
  );

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
    const before = { theme: prefs.theme, palette: prefs.palette, updateCheck: prefs.updateCheck, dateFormat: prefs.dateFormat, clock: prefs.clock };
    setTheme(DEFAULTS.theme);
    setPalette(DEFAULTS.palette);
    savePrefs({ dateFormat: DEFAULTS.dateFormat, clock: DEFAULTS.clock });
    if (checkShown) savePrefs({ updateCheck: DEFAULTS.updateCheck });
    return () => {
      setTheme(before.theme);
      setPalette(before.palette);
      savePrefs({ updateCheck: before.updateCheck, dateFormat: before.dateFormat, clock: before.clock });
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

<section class="group" aria-labelledby="group-dates">
  <h3 id="group-dates">Date and time</h3>
  <SettingRow id="set-date" name="Date format" help="Every date of the application: the files, the commits, the graph." group>
    <Choice name="set-date" labelledby="set-date-name" describedby="set-date-help" value={prefs.dateFormat} options={DATE_FORMATS} onchange={(dateFormat: DateFormat) => savePrefs({ dateFormat })} />
  </SettingRow>
  <SettingRow id="set-clock" name="Clock" help="The time that goes with a date, to the second." group>
    <Choice name="set-clock" labelledby="set-clock-name" describedby="set-clock-help" value={prefs.clock} options={CLOCKS} onchange={(clock: Clock) => savePrefs({ clock })} />
  </SettingRow>
</section>

<UpdatesGroup />
