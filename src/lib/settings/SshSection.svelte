<script lang="ts">
  import { DEFAULTS, prefs, savePrefs } from '../git/prefs.svelte';
  import Choice from '../ui/Choice.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';

  const RECONNECT = [
    { value: 'on', label: 'Activée', help: 'Une connexion perdue est retentée cinq fois, de 2 à 30 s d’écart.' },
    { value: 'off', label: 'Désactivée', help: 'L’onglet attend un clic sur « Reconnecter ».' },
  ] as const;

  const changed = $derived(prefs.sshReconnect !== DEFAULTS.sshReconnect);
  const setReconnect = (on: boolean) => savePrefs({ sshReconnect: on });
</script>

<SectionHeader
  title="Terminal et SSH"
  {changed}
  onreset={() => {
    const before = prefs.sshReconnect;
    setReconnect(DEFAULTS.sshReconnect);
    return () => setReconnect(before);
  }}
/>

<section class="group" aria-labelledby="group-connection">
  <h3 id="group-connection">Connexion</h3>
  <SettingRow
    id="set-reconnect"
    name="Reconnexion automatique"
    help="Une session fermée par toi ou par le serveur ne se rouvre jamais d’elle-même."
    group
  >
    <Choice
      name="set-reconnect"
      labelledby="set-reconnect-name"
      describedby="set-reconnect-help"
      value={prefs.sshReconnect ? 'on' : 'off'}
      options={RECONNECT}
      onchange={(value) => setReconnect(value === 'on')}
    />
  </SettingRow>
</section>
