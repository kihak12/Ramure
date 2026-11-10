<script lang="ts">
  import { AUTO_FETCH, DEFAULTS, prefs, savePrefs, type DiffMode, type GraphStyle, type PullMode, type Settings } from '../git/prefs.svelte';
  import { workspace } from '../git/workspace.svelte';
  import Choice from '../ui/Choice.svelte';
  import DiffPreview from './DiffPreview.svelte';
  import GraphPreview from './GraphPreview.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';

  const FIELDS = ['autoFetch', 'pullMode', 'graphStyle', 'limit', 'diffMode'] as const satisfies readonly (keyof Settings)[];
  const changed = $derived(FIELDS.some((field) => prefs[field] !== DEFAULTS[field]));

  // Chaque option dit ce qu'elle entraîne : on compare sans rien ouvrir (charte, section 9.19).
  const PULL_MODES = [
    { value: 'ff-only', label: 'Avance rapide seulement', help: 'Le tirage est refusé si ta branche et la branche distante ont divergé.' },
    { value: 'rebase', label: 'Rebaser mes commits', help: 'Tes commits sont rejoués au-dessus de ceux de la branche distante.' },
    { value: 'merge', label: 'Fusionner la branche distante', help: 'La branche distante est fusionnée dans la tienne.' },
  ] as const satisfies readonly { value: PullMode; label: string; help: string }[];
  const GRAPH_STYLES = [
    { value: 'ribbon', label: 'Ruban', help: 'Pastilles d’auteur, messages en capsule.' },
    { value: 'classic', label: 'Classique', help: 'Points et courbes, plus compact.' },
  ] as const satisfies readonly { value: GraphStyle; label: string; help: string }[];
  const DIFF_MODES = [
    { value: 'unified', label: 'Unifié', help: 'Sur une colonne.' },
    { value: 'split', label: 'Côte à côte', help: 'L’ancien à gauche, le nouveau à droite.' },
  ] as const satisfies readonly { value: DiffMode; label: string; help: string }[];

  // Récupération automatique : le champ garde ce qu'on y tape ; la valeur n'est appliquée que valide.
  let minutes = $state(String(prefs.autoFetch));
  let minutesError = $state('');
  $effect(() => {
    minutes = String(prefs.autoFetch);
  });

  function applyMinutes() {
    const value = Number(minutes.trim());
    if (minutes.trim() === '' || !Number.isInteger(value) || value < AUTO_FETCH.min || value > AUTO_FETCH.max) {
      // Ce qui s'est passé, puis quoi faire (charte, section 12).
      minutesError = `« ${minutes.trim() || 'vide'} » n’est pas appliqué. Entre un nombre entier de ${AUTO_FETCH.min} à ${AUTO_FETCH.max}.`;
      return;
    }
    minutesError = '';
    savePrefs({ autoFetch: value });
    minutes = String(value);
  }

  // Le nombre de commits chargés change ce que chaque dépôt ouvert a lu : on les relit tous.
  const reloading = $derived(workspace.tabs.some((tab) => tab.repo.loading));
  function setLimit(limit: number) {
    if (limit === prefs.limit) return;
    savePrefs({ limit });
    for (const tab of workspace.tabs) {
      if (tab.repo.result) void tab.repo.load(tab.repo.result.repo, false);
    }
  }

  type GitSettings = Pick<Settings, (typeof FIELDS)[number]>;
  /** Les réglages de cette section, et eux seuls. */
  const own = (all: Settings): GitSettings => ({
    autoFetch: all.autoFetch,
    pullMode: all.pullMode,
    graphStyle: all.graphStyle,
    limit: all.limit,
    diffMode: all.diffMode,
  });

  /** Applique d'un coup les réglages de la section, et relit les dépôts si leur taille change. */
  function apply({ limit, ...rest }: GitSettings) {
    savePrefs(rest);
    minutesError = '';
    setLimit(limit);
  }

  /** Revient aux valeurs par défaut, et rend de quoi retrouver les réglages d'avant. */
  function reset() {
    const before = own(prefs);
    apply(own(DEFAULTS));
    return () => apply(before);
  }
</script>

<SectionHeader title="Git" note="S’applique à tous les dépôts." {changed} onreset={reset} />

<section class="group" aria-labelledby="group-sync">
  <h3 id="group-sync">Synchronisation</h3>
  <SettingRow id="set-fetch" name="Récupération automatique" help="En minutes. 0 la désactive." error={minutesError}>
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
  <SettingRow id="set-pull" name="Tirage (pull)" group>
    <Choice name="set-pull" labelledby="set-pull-name" value={prefs.pullMode} options={PULL_MODES} onchange={(pullMode) => savePrefs({ pullMode })} />
  </SettingRow>
</section>

<section class="group" aria-labelledby="group-graph">
  <h3 id="group-graph">Graphe</h3>
  <SettingRow id="set-style" name="Style du graphe" group>
    <Choice name="set-style" layout="tiles" labelledby="set-style-name" value={prefs.graphStyle} options={GRAPH_STYLES} onchange={(graphStyle) => savePrefs({ graphStyle })}>
      {#snippet preview(style: GraphStyle)}<GraphPreview {style} />{/snippet}
    </Choice>
  </SettingRow>
  <SettingRow
    id="set-limit"
    name="Commits chargés dans le graphe"
    help="« Tout l’historique » peut être long sur un gros dépôt."
    busy={reloading ? 'Rechargement du graphe…' : ''}
  >
    <select id="set-limit" class="field" aria-describedby="set-limit-help" value={prefs.limit} onchange={(e) => setLimit(Number(e.currentTarget.value))}>
      <option value={1000}>1 000</option>
      <option value={10000}>10 000</option>
      <option value={50000}>50 000</option>
      <option value={0}>Tout l’historique</option>
    </select>
  </SettingRow>
</section>

<section class="group" aria-labelledby="group-diff">
  <h3 id="group-diff">Diff</h3>
  <SettingRow id="set-diff" name="Affichage du diff" help="Se change aussi depuis le diff lui-même." group>
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
  .field.invalid {
    border-color: var(--bad);
  }
</style>
