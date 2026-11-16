<script lang="ts">
  import { tick } from 'svelte';
  import { prefs, savePrefs } from '../git/prefs.svelte';
  import { workspace } from '../git/workspace.svelte';
  import { mayRestart, restartLosses } from '../ssh/guard';
  import { ssh } from '../ssh/ssh.svelte';
  import Choice from '../ui/Choice.svelte';
  import { focusWithRing } from '../ui/focus';
  import Icon from '../ui/Icon.svelte';
  import ProgressBar from '../ui/ProgressBar.svelte';
  import { updates } from '../ui/updates.svelte';
  import SettingRow from './SettingRow.svelte';

  // The group "Updates" of the settings (guidelines, section 9.23): the version that runs, the
  // check, and the box of a version that waits, whose install takes two steps.

  const RELEASES = 'github.com/kihak12/Ramure/releases';

  const AUTO = [
    { value: 'on', label: 'On', help: 'At startup, then once a day. Nothing is downloaded or installed on its own.' },
    { value: 'off', label: 'Off', help: 'Ramure never contacts the network on its own. Use “Check for updates” when you want to.' },
  ] as const;

  const info = $derived(updates.info);
  const found = $derived(updates.found);
  const phase = $derived(updates.phase);
  const checked = $derived(updates.checked);

  // "Checked 5 min ago": the words follow the clock.
  let now = $state(Date.now());
  $effect(() => {
    const timer = window.setInterval(() => (now = Date.now()), 30_000);
    return () => window.clearInterval(timer);
  });

  function ago(at: number) {
    const minutes = Math.floor((now - at) / 60_000);
    if (minutes < 1) return 'just now';
    if (minutes < 60) return `${minutes} min ago`;
    const hours = Math.floor(minutes / 60);
    return hours < 24 ? `${hours} h ago` : `${Math.floor(hours / 24)} d ago`;
  }

  const day = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  const megabytes = (bytes: number) => (bytes / 1_048_576).toFixed(1);

  const percent = $derived(updates.progress.total ? Math.min(100, (updates.progress.done / updates.progress.total) * 100) : null);
  const received = $derived(
    updates.progress.total ? `${megabytes(updates.progress.done)} of ${megabytes(updates.progress.total)} MB` : `${megabytes(updates.progress.done)} MB`,
  );

  // The button of a step gives way to the next one: the focus follows, so that the keyboard does
  // not lose its place in the box.
  let box = $state<HTMLElement>();
  async function follow() {
    await tick();
    focusWithRing(box?.querySelector<HTMLButtonElement>('button'));
  }

  function download() {
    void updates.download().then(follow);
    void follow();
  }

  async function cancel() {
    await updates.cancel();
    await follow();
  }

  /** "Restart and install…": always asks, and says what the restart would close. */
  async function restart() {
    if (!found || !info) return;
    const sessions = [...ssh.sessions, ...workspace.tabs.flatMap((tab) => tab.repo.terminals.sessions)];
    const live = ['starting', 'listening', 'waiting'];
    const tunnels = ssh.sessions
      .filter((session) => session.kind === 'ssh')
      .reduce((n, session) => n + ssh.tunnelsOf(session).filter((row) => live.includes(row.state.state)).length, 0);
    const repos = workspace.tabs
      .filter((tab) => tab.repo.path)
      .map((tab) => ({ name: workspace.title(tab), message: tab.repo.message.trim() !== '', busy: tab.repo.busy !== '' }));
    if (await mayRestart(found.version, info.installer, await restartLosses(sessions, tunnels, repos))) {
      void updates.install().then(follow);
    } else {
      await follow();
    }
  }
</script>

<section class="group" aria-labelledby="group-updates">
  <h3 id="group-updates">Updates</h3>
  {#if info}
    <SettingRow
      id="set-version"
      name="Version {info.version}"
      help={info.canUpdate ? '' : `This copy was built from the sources and cannot update itself. New versions are on ${RELEASES}.`}
      busy={updates.checking ? 'Checking for updates…' : ''}
      fact
    >
      {#if info.canUpdate}
        <button class="btn small check" type="button" disabled={updates.checking || (phase !== 'idle' && phase !== 'available')} onclick={() => void updates.check(true)}>
          Check for updates
        </button>
        {#if !updates.checking && checked}
          {#if checked.failure}
            <!-- Announced when it follows a click; a check that failed on its own waits to be read. -->
            <p class="result" role={checked.manual ? 'alert' : undefined}>
              <span class="bad"><Icon name="warning" size={14} /></span>
              <span>
                {#if checked.manual}{checked.failure.message}{:else}The last automatic check failed {ago(checked.at)}. Check your connection, then try again.{/if}
              </span>
            </p>
            {#if checked.failure.reason}<p class="reason mono">{checked.failure.reason}</p>{/if}
          {:else if !found}
            <p class="result" role="status">
              <span class="ok"><Icon name="check" size={14} /></span>
              <span>Ramure is up to date. <span class="muted">Checked {ago(checked.at)}.</span></span>
            </p>
          {/if}
        {/if}
      {/if}
    </SettingRow>

    {#if info.canUpdate}
      <SettingRow id="set-update-check" name="Automatic update check" help="Ramure asks GitHub whether a newer version exists." group>
        <Choice
          name="set-update-check"
          labelledby="set-update-check-name"
          describedby="set-update-check-help"
          value={prefs.updateCheck ? 'on' : 'off'}
          options={AUTO}
          onchange={(value) => savePrefs({ updateCheck: value === 'on' })}
        />
      </SettingRow>
    {/if}

    {#if found}
      <div class="box" bind:this={box}>
        <!-- The state of the version, announced as it changes. -->
        <div class="text" role="status">
          {#if phase === 'downloading' || phase === 'verifying'}
            <p class="head">Downloading version {found.version}…</p>
            {#if phase === 'verifying'}
              <p class="wait"><span class="spinner" aria-hidden="true"></span>Verifying…</p>
            {:else}
              <!-- On one line, as high as a line of text: the button under it keeps its place from
                   one state to the next. The figures change with every chunk: they are not read. -->
              <div class="progress">
                <ProgressBar {percent} />
                <p class="figures" aria-hidden="true">{received}</p>
              </div>
            {/if}
          {:else if phase === 'ready'}
            <p class="head">Version {found.version} is ready to install</p>
            <p>Nothing changes until you restart. You can keep working.</p>
          {:else if phase === 'installing'}
            <p class="head">Installing version {found.version}…</p>
            <p class="wait"><span class="spinner" aria-hidden="true"></span>Ramure restarts when it is done.</p>
          {:else}
            <p class="head">Version {found.version} is available</p>
            <p>{found.published ? `Published ${day.format(found.published)}. ` : ''}What changed is on {RELEASES}.</p>
          {/if}
        </div>
        {#if phase === 'available'}
          <div><button class="btn small" type="button" onclick={download}>Download update</button></div>
        {:else if phase === 'downloading'}
          <div><button class="btn small" type="button" onclick={() => void cancel()}>Cancel</button></div>
        {:else if phase === 'ready'}
          <div><button class="btn small" type="button" onclick={() => void restart()}>Restart and install…</button></div>
        {/if}
      </div>
      <!-- Under the box, on the card: the color of a failure does not read on the tint. -->
      {#if updates.failed}
        <div class="failure">
          <p class="result" role="alert">
            <span class="bad"><Icon name="warning" size={14} /></span>
            <span>{updates.failed.message}</span>
          </p>
          {#if updates.failed.reason}<p class="reason mono">{updates.failed.reason}</p>{/if}
        </div>
      {/if}
    {/if}
  {/if}
</section>

<style>
  /* A small button, 24 px, centered where a field, 32 px, would be: level with the name. */
  .check {
    justify-self: start;
    margin-top: 4px;
  }
  p {
    margin: 0;
    font-size: var(--fs-small);
  }
  /* A result: its icon, in the color of the state, then the words in --ink. */
  .result {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    overflow-wrap: anywhere;
  }
  .result :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .ok {
    color: var(--ok);
  }
  .bad {
    color: var(--bad);
  }
  .muted,
  .reason {
    color: var(--muted);
  }
  .reason {
    overflow-wrap: anywhere;
  }
  /* The box of a version that waits: a sign, as the strip of uncommitted changes (9.22). */
  .box {
    display: grid;
    gap: 8px;
    padding: 10px;
    border-radius: var(--radius);
    background: var(--accent-soft);
  }
  .text {
    display: grid;
    gap: 6px;
  }
  /* In --ink: the only text color that reads on the tint (guidelines, 2.4). */
  .text p {
    overflow-wrap: anywhere;
  }
  .text .head {
    font-size: var(--fs-body);
    font-weight: var(--fw-strong);
  }
  .failure {
    display: grid;
    gap: 2px;
  }
  .progress {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
  }
  .figures {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .wait {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
