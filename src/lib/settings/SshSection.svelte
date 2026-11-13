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
    { value: 'on', label: 'On', help: 'A lost connection is retried five times, 2 to 30 s apart.' },
    { value: 'off', label: 'Off', help: 'The tab waits for a click on “Reconnect”.' },
  ] as const;

  const changed = $derived(prefs.sshReconnect !== DEFAULTS.sshReconnect || prefs.journalDir !== DEFAULTS.journalDir);
  const setReconnect = (on: boolean) => savePrefs({ sshReconnect: on });

  // The log folder: the one chosen, or the default one, shown as it will be used (9.18).
  let folder = $state('');
  let folderError = $state('');
  $effect(() => {
    void prefs.journalDir;
    void journalDir().then((dir) => (folder = dir));
  });

  async function browse() {
    folderError = '';
    try {
      const picked = await open({ title: 'Choose the log folder', directory: true, multiple: false, defaultPath: folder || undefined });
      if (typeof picked === 'string') savePrefs({ journalDir: picked });
    } catch (e) {
      folderError = `The folder picker could not open. Try again, or keep the default folder. (${e instanceof Error ? e.message : String(e)})`;
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
  title="Terminal and SSH"
  {changed}
  onreset={() => {
    const before = { sshReconnect: prefs.sshReconnect, journalDir: prefs.journalDir };
    savePrefs({ sshReconnect: DEFAULTS.sshReconnect, journalDir: DEFAULTS.journalDir });
    return () => savePrefs(before);
  }}
/>

<section class="group" aria-labelledby="group-connection">
  <h3 id="group-connection">Connection</h3>
  <SettingRow
    id="set-reconnect"
    name="Automatic reconnection"
    help="A session closed by you or by the server never reopens on its own."
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

<section class="group" aria-labelledby="group-logs">
  <h3 id="group-logs">Session logs</h3>
  <SettingRow id="set-log-folder" name="Log folder" help="Where Ramure writes one file per recorded session." error={folderError}>
    <div class="folder">
      <input id="set-log-folder" class="field mono" type="text" readonly value={folder} title={folder} aria-describedby={folderError ? 'set-log-folder-help set-log-folder-error' : 'set-log-folder-help'} />
      <div class="actions">
        <button class="btn small" type="button" onclick={() => void browse()}><Icon name="folder" size={14} /> Browse…</button>
        <button class="btn ghost small" type="button" onclick={() => void openFolder()}>Open folder</button>
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
