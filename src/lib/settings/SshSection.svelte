<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { DEFAULTS, prefs, savePrefs } from '../git/prefs.svelte';
  import Icon from '../ui/Icon.svelte';
  import { journalDir } from '../ssh/journal';
  import Choice from '../ui/Choice.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';

  const RECONNECT = [
    { value: 'on', label: 'Activée', help: 'Une connexion perdue est retentée cinq fois, de 2 à 30 s d’écart.' },
    { value: 'off', label: 'Désactivée', help: 'L’onglet attend un clic sur « Reconnecter ».' },
  ] as const;

  const changed = $derived(prefs.sshReconnect !== DEFAULTS.sshReconnect || prefs.journalDir !== DEFAULTS.journalDir);
  const setReconnect = (on: boolean) => savePrefs({ sshReconnect: on });

  // Le dossier des journaux : choisi, ou celui par défaut, montré tel qu'il sera employé (9.18).
  let folder = $state('');
  let folderError = $state('');
  $effect(() => {
    void prefs.journalDir;
    void journalDir().then((dir) => (folder = dir));
  });

  async function browse() {
    folderError = '';
    try {
      const picked = await open({ title: 'Choisir le dossier des journaux', directory: true, multiple: false, defaultPath: folder || undefined });
      if (typeof picked === 'string') savePrefs({ journalDir: picked });
    } catch (e) {
      folderError = `La fenêtre de dossiers n’a pas pu s’ouvrir : réessaie, ou garde le dossier par défaut. (${e instanceof Error ? e.message : String(e)})`;
    }
  }

  async function openFolder() {
    folderError = '';
    try {
      await invoke('journal_open_dir', { path: folder });
    } catch (e) {
      folderError = typeof e === 'string' ? e : String(e);
    }
  }
</script>

<SectionHeader
  title="Terminal et SSH"
  {changed}
  onreset={() => {
    const before = { sshReconnect: prefs.sshReconnect, journalDir: prefs.journalDir };
    savePrefs({ sshReconnect: DEFAULTS.sshReconnect, journalDir: DEFAULTS.journalDir });
    return () => savePrefs(before);
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

<section class="group" aria-labelledby="group-journal">
  <h3 id="group-journal">Journaux de session</h3>
  <SettingRow id="set-journal" name="Dossier des journaux" help="Là où Ramure écrit un fichier par session enregistrée." error={folderError}>
    <div class="folder">
      <input id="set-journal" class="field mono" type="text" readonly value={folder} title={folder} aria-describedby={folderError ? 'set-journal-help set-journal-error' : 'set-journal-help'} />
      <div class="actions">
        <button class="btn small" type="button" onclick={() => void browse()}><Icon name="folder" size={14} /> Parcourir…</button>
        <button class="btn ghost small" type="button" onclick={() => void openFolder()}>Ouvrir le dossier</button>
      </div>
    </div>
  </SettingRow>
</section>

<style>
  .folder {
    display: grid;
    gap: 6px;
  }
  .folder input {
    text-overflow: ellipsis;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
</style>
