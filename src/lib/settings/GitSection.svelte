<script lang="ts">
  import { AUTO_FETCH, DEFAULTS, prefs, savePrefs, type DiffMode, type GraphStyle, type PullMode, type Settings } from '../git/prefs.svelte';
  import { workspace } from '../git/workspace.svelte';
  import Choice from '../ui/Choice.svelte';
  import Select from '../ui/Select.svelte';
  import DiffPreview from './DiffPreview.svelte';
  import GraphPreview from './GraphPreview.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';

  const FIELDS = ['autoFetch', 'pullMode', 'graphStyle', 'limit', 'diffMode'] as const satisfies readonly (keyof Settings)[];
  const changed = $derived(FIELDS.some((field) => prefs[field] !== DEFAULTS[field]));

  // Each option says what it entails: they are compared without opening anything (guidelines, section 9.19).
  const PULL_MODES = [
    { value: 'ff-only', label: 'Fast-forward only', help: 'The pull is refused if your branch and the remote branch have diverged.' },
    { value: 'rebase', label: 'Rebase your commits', help: 'Your commits are replayed on top of those of the remote branch.' },
    { value: 'merge', label: 'Merge the remote branch', help: 'The remote branch is merged into yours.' },
  ] as const satisfies readonly { value: PullMode; label: string; help: string }[];
  const GRAPH_STYLES = [
    { value: 'ribbon', label: 'Ribbon', help: 'Author initials on the dots, messages in capsules.' },
    { value: 'classic', label: 'Classic', help: 'Points and curves, more compact.' },
  ] as const satisfies readonly { value: GraphStyle; label: string; help: string }[];
  const DIFF_MODES = [
    { value: 'unified', label: 'Unified', help: 'In one column.' },
    { value: 'split', label: 'Side by side', help: 'The old version on the left, the new one on the right.' },
  ] as const satisfies readonly { value: DiffMode; label: string; help: string }[];

  // Automatic fetch: the field keeps what is typed in it; the value is applied only when it is valid.
  let minutes = $state(String(prefs.autoFetch));
  let minutesError = $state('');
  $effect(() => {
    minutes = String(prefs.autoFetch);
  });

  function applyMinutes() {
    const typed = minutes.trim();
    const value = Number(typed);
    if (typed === '' || !Number.isInteger(value) || value < AUTO_FETCH.min || value > AUTO_FETCH.max) {
      // What happened, then what to do (guidelines, section 12).
      minutesError = `${typed ? `“${typed}” is` : 'An empty field is'} not applied. Enter a whole number from ${AUTO_FETCH.min} to ${AUTO_FETCH.max}.`;
      return;
    }
    minutesError = '';
    savePrefs({ autoFetch: value });
    minutes = String(value);
  }

  // The number of commits loaded changes what each open repository has read: they are all read again.
  const reloading = $derived(workspace.tabs.some((tab) => tab.repo.loading));
  function setLimit(limit: number) {
    if (limit === prefs.limit) return;
    savePrefs({ limit });
    for (const tab of workspace.tabs) {
      if (tab.repo.result) void tab.repo.load(tab.repo.result.repo, false);
    }
  }

  type GitSettings = Pick<Settings, (typeof FIELDS)[number]>;
  /** The settings of this section, and only them. */
  const own = (all: Settings): GitSettings => ({
    autoFetch: all.autoFetch,
    pullMode: all.pullMode,
    graphStyle: all.graphStyle,
    limit: all.limit,
    diffMode: all.diffMode,
  });

  /** Applies the settings of the section at once, and reads the repositories again if their size changes. */
  function apply({ limit, ...rest }: GitSettings) {
    savePrefs(rest);
    minutesError = '';
    setLimit(limit);
  }

  /** Goes back to the defaults, and returns what brings back the settings from before. */
  function reset() {
    const before = own(prefs);
    apply(own(DEFAULTS));
    return () => apply(before);
  }
</script>

<SectionHeader title="Git" note="Applies to every repository." {changed} onreset={reset} />

<section class="group" aria-labelledby="group-sync">
  <h3 id="group-sync">Sync</h3>
  <SettingRow id="set-fetch" name="Automatic fetch" help="In minutes. 0 turns it off." error={minutesError}>
    <span class="unit">
      <input
        id="set-fetch"
        class="field"
        class:invalid={!!minutesError}
        type="text"
        inputmode="numeric"
        autocomplete="off"
        aria-describedby={minutesError ? 'set-fetch-error' : 'set-fetch-help'}
        aria-invalid={minutesError ? 'true' : undefined}
        bind:value={minutes}
        onchange={applyMinutes}
      />
      <span aria-hidden="true">min</span>
    </span>
  </SettingRow>
  <SettingRow id="set-pull" name="Pull" group>
    <Choice name="set-pull" labelledby="set-pull-name" value={prefs.pullMode} options={PULL_MODES} onchange={(pullMode) => savePrefs({ pullMode })} />
  </SettingRow>
</section>

<section class="group" aria-labelledby="group-graph">
  <h3 id="group-graph">Graph</h3>
  <SettingRow id="set-style" name="Graph style" group>
    <Choice name="set-style" layout="tiles" labelledby="set-style-name" value={prefs.graphStyle} options={GRAPH_STYLES} onchange={(graphStyle) => savePrefs({ graphStyle })}>
      {#snippet preview(style: GraphStyle)}<GraphPreview {style} />{/snippet}
    </Choice>
  </SettingRow>
  <SettingRow
    id="set-limit"
    name="Commits loaded in the graph"
    help="“Whole history” can be slow on a large repository."
    busy={reloading ? 'Reloading the graph…' : ''}
  >
    <Select id="set-limit" aria-describedby="set-limit-help" value={prefs.limit} onchange={(e) => setLimit(Number(e.currentTarget.value))}>
      <option value={1000}>1,000</option>
      <option value={10000}>10,000</option>
      <option value={50000}>50,000</option>
      <option value={0}>Whole history</option>
    </Select>
  </SettingRow>
</section>

<section class="group" aria-labelledby="group-diff">
  <h3 id="group-diff">Diff</h3>
  <SettingRow id="set-diff" name="Diff layout" help="Can also be changed from the diff itself." group>
    <Choice name="set-diff" layout="tiles" labelledby="set-diff-name" describedby="set-diff-help" value={prefs.diffMode} options={DIFF_MODES} onchange={(diffMode) => savePrefs({ diffMode })}>
      {#snippet preview(mode: DiffMode)}<DiffPreview {mode} />{/snippet}
    </Choice>
  </SettingRow>
</section>

<style>
  .unit {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .unit span {
    color: var(--muted);
    font-size: var(--fs-label);
  }
</style>
