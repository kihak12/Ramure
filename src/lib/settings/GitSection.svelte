<script lang="ts">
  import { AUTO_FETCH, DEFAULTS, prefs, savePrefs, type DiffMode, type GraphStyle, type PullMode, type Settings } from '../git/prefs.svelte';
  import { workspace } from '../git/workspace.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';

  const FIELDS = ['autoFetch', 'pullMode', 'graphStyle', 'limit', 'diffMode'] as const satisfies readonly (keyof Settings)[];
  const changed = $derived(FIELDS.some((field) => prefs[field] !== DEFAULTS[field]));

  const PULL_HELP: Record<PullMode, string> = {
    'ff-only': 'Le pull est refusé si ta branche et la branche distante ont divergé.',
    rebase: 'Tes commits sont rejoués au-dessus de ceux de la branche distante.',
    merge: 'La branche distante est fusionnée dans la tienne.',
  };
  const STYLE_HELP: Record<GraphStyle, string> = {
    ribbon: 'Pastilles d’auteur sur le graphe, messages en capsule.',
    classic: 'Points et courbes, plus compact.',
  };

  // Récupération automatique : le champ garde ce qu'on y tape ; la valeur n'est appliquée que valide.
  let minutes = $state(String(prefs.autoFetch));
  let minutesError = $state('');
  $effect(() => {
    minutes = String(prefs.autoFetch);
  });

  function applyMinutes() {
    const value = Number(minutes.trim());
    if (minutes.trim() === '' || !Number.isInteger(value) || value < AUTO_FETCH.min || value > AUTO_FETCH.max) {
      minutesError = `Entre un nombre entier de ${AUTO_FETCH.min} à ${AUTO_FETCH.max}.`;
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

  function reset() {
    const limit = prefs.limit;
    savePrefs({ autoFetch: DEFAULTS.autoFetch, pullMode: DEFAULTS.pullMode, graphStyle: DEFAULTS.graphStyle, diffMode: DEFAULTS.diffMode });
    minutesError = '';
    if (limit !== DEFAULTS.limit) setLimit(DEFAULTS.limit);
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
  <SettingRow id="set-pull" name="Tirer (pull)" help={PULL_HELP[prefs.pullMode]}>
    <select id="set-pull" class="field" aria-describedby="set-pull-help" value={prefs.pullMode} onchange={(e) => savePrefs({ pullMode: e.currentTarget.value as PullMode })}>
      <option value="ff-only">Avance rapide seulement</option>
      <option value="rebase">Rebaser mes commits</option>
      <option value="merge">Fusionner la branche distante</option>
    </select>
  </SettingRow>
</section>

<section class="group" aria-labelledby="group-graph">
  <h3 id="group-graph">Graphe</h3>
  <SettingRow id="set-style" name="Style du graphe" help={STYLE_HELP[prefs.graphStyle]}>
    <select id="set-style" class="field" aria-describedby="set-style-help" value={prefs.graphStyle} onchange={(e) => savePrefs({ graphStyle: e.currentTarget.value as GraphStyle })}>
      <option value="ribbon">Ruban</option>
      <option value="classic">Classique</option>
    </select>
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
  <SettingRow id="set-diff" name="Affichage du diff" help="Se change aussi depuis le diff lui-même.">
    <select id="set-diff" class="field" aria-describedby="set-diff-help" value={prefs.diffMode} onchange={(e) => savePrefs({ diffMode: e.currentTarget.value as DiffMode })}>
      <option value="unified">Unifié</option>
      <option value="split">Côte à côte</option>
    </select>
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
