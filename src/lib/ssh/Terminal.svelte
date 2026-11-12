<script lang="ts">
  import { theme as appTheme } from '../settings/theme.svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { FitAddon } from '@xterm/addon-fit';
  import { SearchAddon, type ISearchOptions } from '@xterm/addon-search';
  import { tick, untrack } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import '@xterm/xterm/css/xterm.css';
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { prefs } from '../git/prefs.svelte';
  import { STATUS, type Session } from './ssh.svelte';
  import { focusWithRing } from '../ui/focus';
  import { toasts } from '../ui/toasts.svelte';
  import { keyringAvailable, NO_KEYRING } from './secrets';
  import { explain } from './reasons';
  import { newJournal, resumed, shown, type JournalOpen } from './journal';
  import type { Closed, Connected, SshError } from './types';

  let {
    session,
    active,
    onclose,
    bare = false,
    autofocus = true,
    onoutput,
  }: {
    session: Session;
    active: boolean;
    onclose: () => void;
    /** Sans en-tête : le panneau qui l'accueille dit lui-même ce qu'est le terminal (charte, 9.20). */
    bare?: boolean;
    /** Le terminal prend le clavier quand son onglet s'affiche. Sous le graphe, jamais de lui-même. */
    autofocus?: boolean;
    /** Appelé à chaque sortie du shell : le dépôt se relit quand elle se tait. */
    onoutput?: () => void;
  } = $props();

  /** Ce que la session attend de l'utilisateur, ou son état. */
  type Phase =
    | { step: 'connecting' }
    | { step: 'connected' }
    | { step: 'hostKey'; fingerprint: string; algorithm: string }
    | { step: 'hostKeyChanged'; fingerprint: string; line: number }
    | { step: 'passphrase'; path: string; error: string }
    | { step: 'password'; error: string }
    | { step: 'error'; message: string }
    /** `brief` : la même chose en une ligne, pour la bande d'un terminal de dépôt. */
    | { step: 'closed'; message: string; lost?: boolean; brief?: string }
    /** Connexion perdue : la tentative `attempt` (de 1 à 5) part à l'instant `at`. */
    | { step: 'lost'; attempt: number; at: number; reason: string };

  // Délais entre deux tentatives de reconnexion, en secondes (charte, section 9.15).
  const RETRIES = [2, 5, 10, 20, 30];
  const NB = '\u00a0'; // espace insécable, avant « : » et entre un nombre et son unité

  /** Ce qu'on sait déjà pour cette tentative : rien n'est conservé une fois la session ouverte. */
  interface Attempt {
    accept?: string;
    remember?: boolean;
    keyPath?: string;
    passphrase?: string;
    password?: string;
    /** Retenir le secret tapé dans le trousseau du système, une fois qu'il a servi. */
    keep?: boolean;
  }

  let phase = $state<Phase>({ step: 'connecting' });
  // Le trousseau du système : s'il répond, la demande d'un secret propose de le retenir (9.15).
  let keyring = $state<boolean | null>(null);
  let keep = $state(false);
  // Demandé dès l'ouverture d'une session, pour que la demande d'un secret s'affiche d'un bloc, et
  // à nouveau quand elle s'affiche : le trousseau a pu arriver ou partir entre-temps.
  $effect(() => {
    if (session.kind !== 'ssh') return;
    void phase.step;
    if (phase.step !== 'connecting' && phase.step !== 'passphrase' && phase.step !== 'password') return;
    void keyringAvailable().then((answer) => (keyring = answer));
  });
  // Un secret est demandé : son champ prend le clavier, dès que l'onglet est à l'écran. L'attribut
  // `autofocus` ne suffit pas : il ne joue que si rien d'autre n'a le focus, or la ligne de l'hôte
  // qu'on vient de double-cliquer l'a encore.
  let secretEl = $state<HTMLInputElement>();
  $effect(() => {
    if ((phase.step !== 'passphrase' && phase.step !== 'password') || !active) return;
    void tick().then(() => secretEl?.focus());
  });
  // Reconnexion automatique : la tentative en cours (0 quand il n'y en a pas), son minuteur, et
  // l'heure, rafraîchie chaque seconde pour le compte à rebours.
  let retry = $state(0);
  let retryTimer: number | undefined;
  let now = $state(Date.now());
  $effect(() => {
    if (phase.step !== 'lost') return;
    const tick = window.setInterval(() => (now = Date.now()), 1000);
    return () => window.clearInterval(tick);
  });
  const waitText = $derived.by(() => {
    if (phase.step !== 'lost') return '';
    const seconds = Math.max(0, Math.ceil((phase.at - now) / 1000));
    const which = `${phase.attempt}${NB}sur${NB}${RETRIES.length}`;
    return seconds > 0 ? `Nouvelle tentative dans ${seconds}${NB}s, ${which}.` : `Tentative ${which}…`;
  });
  // Ce qu'un lecteur d'écran entend : une fois par tentative, pas à chaque seconde.
  const announce = $derived(
    phase.step === 'lost' ? `Connexion perdue. Tentative ${phase.attempt} sur ${RETRIES.length} dans ${RETRIES[phase.attempt - 1]} secondes.` : '',
  );
  let retryButton = $state<HTMLButtonElement>();
  let endButton = $state<HTMLButtonElement>();
  // Un panneau de fin apparaît : le focus va sur son bouton principal, que le clavier n'atteindrait
  // pas autrement, le terminal gardant la touche Tab (charte, section 9.15).
  let rootEl = $state<HTMLElement>();
  $effect(() => {
    if (phase.step !== 'closed' && phase.step !== 'error') return;
    // Sous le graphe, le terminal ne prend jamais le clavier de lui-même (section 9.20) : le bouton
    // ne le prend que si le clavier était déjà dans ce terminal.
    if (bare && !untrack(() => rootEl?.contains(document.activeElement))) return;
    void tick().then(() => focusWithRing(endButton));
  });
  // La carte apparaît : le focus va sur « Reconnecter maintenant ». Le terminal, coupé, ne sert plus,
  // et il garde la touche Tab pour lui : sans cela, le clavier n'atteindrait pas la carte.
  $effect(() => {
    if (phase.step !== 'lost' || phase.attempt !== 1) return;
    void tick().then(() => focusWithRing(retryButton));
  });

  /** Programme la tentative `attempt`, ou s'arrête après la dernière. */
  function scheduleRetry(attempt: number, reason: string) {
    window.clearTimeout(retryTimer);
    if (attempt > RETRIES.length) {
      retry = 0;
      journalEnded(true);
      session.status = 'closed';
      phase = { step: 'closed', lost: true, message: `Les ${RETRIES.length} tentatives de reconnexion ont échoué${reason ? `${NB}: ${explain(reason)}` : '.'}` };
      return;
    }
    retry = attempt;
    session.status = 'reconnecting';
    const delay = RETRIES[attempt - 1] * 1000;
    now = Date.now();
    phase = { step: 'lost', attempt, at: now + delay, reason: explain(reason) };
    retryTimer = window.setTimeout(() => void connect({}, attempt), delay);
  }

  /** « Reconnecter maintenant » : la tentative prévue part sans attendre. */
  function retryNow() {
    if (phase.step !== 'lost') return;
    window.clearTimeout(retryTimer);
    void connect({}, phase.attempt);
  }

  /** « Arrêter » : plus de tentative, l'onglet attend un clic. */
  function stopRetrying() {
    window.clearTimeout(retryTimer);
    retry = 0;
    journalEnded(true);
    session.status = 'closed';
    phase = { step: 'closed', lost: true, message: 'La reconnexion automatique est arrêtée.' };
  }
  let info = $state.raw<Connected | null>(null);
  let echo = $state<number | null>(null);
  let secret = $state('');

  let termEl = $state<HTMLDivElement>();
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let search: SearchAddon | undefined;

  // --- Recherche dans le terminal (charte, section 9.15) -------------------------------------
  const MATCH = 0.3; // part de l'accent dans le fond d'un résultat, comme dans verifier-contrastes.mjs
  let query = $state('');
  let caseSensitive = $state(false);
  let regex = $state(false);
  /** Résultats de la dernière recherche ; `index` vaut -1 au-delà de 1 000 résultats surlignés. */
  let results = $state<{ index: number; count: number } | null>(null);
  let searchEl = $state<HTMLInputElement>();
  const invalid = $derived.by(() => {
    if (!regex || !query) return false;
    try {
      new RegExp(query);
      return false;
    } catch {
      return true;
    }
  });
  const countText = $derived(
    !query || !results
        ? ''
        : results.count === 0
          ? 'Aucun résultat'
          : results.index < 0
            ? `${results.count} résultats`
            : `${results.index + 1} sur ${results.count}`,
  );

  /** `#rrggbb` mêlé à `p` sur `#rrggbb` : le terminal veut une couleur pleine, pas `color-mix()`. */
  function blend(hex: string, p: number, over: string) {
    const channels = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
    const [a, b] = [channels(hex), channels(over)];
    return `#${a.map((v, i) => Math.round(v * p + b[i] * (1 - p)).toString(16).padStart(2, '0')).join('')}`;
  }

  /** Les couleurs des résultats, lues dans les jetons au moment de chercher : elles suivent le thème.
   *  Un résultat est l'accent à 30 % sur le fond du terminal (charte, section 9.15). */
  function searchOptions(incremental = false): ISearchOptions {
    const css = getComputedStyle(document.documentElement);
    const v = (name: string) => css.getPropertyValue(name).trim();
    const match = blend(v('--accent'), MATCH, v('--term-bg'));
    return {
      caseSensitive,
      regex,
      incremental,
      decorations: {
        matchBackground: match,
        matchOverviewRuler: v('--accent'),
        activeMatchBackground: match,
        activeMatchBorder: v('--accent'),
        activeMatchColorOverviewRuler: v('--accent'),
      },
    };
  }

  function find(direction: 'next' | 'previous', incremental = false) {
    if (!search) return;
    if (!query || invalid) {
      search.clearDecorations();
      results = null;
      return;
    }
    if (direction === 'next') search.findNext(query, searchOptions(incremental));
    else search.findPrevious(query, searchOptions());
  }

  function closeSearch() {
    session.searching = false;
    search?.clearDecorations();
    results = null;
    term?.focus();
  }

  function onFieldKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      find(e.shiftKey ? 'previous' : 'next');
    }
  }

  /** Échap ferme la barre, d'où que vienne la frappe : le champ, un bouton ou une bascule. */
  function onBarKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    closeSearch();
  }

  // Ouverture demandée (raccourci, palette), même barre déjà ouverte : le champ prend le focus et
  // son texte est sélectionné.
  $effect(() => {
    void session.searchAsk;
    if (!session.searching) return;
    void tick().then(() => {
      searchEl?.focus();
      searchEl?.select();
    });
  });

  // Les options changent : la recherche est refaite sur place.
  $effect(() => {
    void caseSensitive;
    void regex;
    if (session.searching && query) find('next', true);
  });
  // Hors réactivité : lus dans les rappels du terminal, à chaque frappe.
  let sessionId: number | null = null;
  /** Quelque chose a été tapé dans ce shell : un arrêt en erreur n'est alors pas un échec de démarrage. */
  let typed = false;
  let lastKey = 0;
  let echoes: number[] = [];
  let attempt: Attempt = {};

  const host = $derived(session.host);
  // Un terminal local parle au cœur Rust par ses propres commandes ; le reste du composant est commun.
  // La nature d'un onglet ne change pas : la lire une fois suffit.
  // svelte-ignore state_referenced_locally
  const local = session.kind === 'local';
  // Le terminal d'un dépôt dit l'état de son shell dans une bande, pas sur un voile.
  const strip = $derived(bare && local);
  const API = local
    ? { write: 'local_write', resize: 'local_resize', close: 'local_close', closed: 'local-closed' }
    : { write: 'ssh_write', resize: 'ssh_resize', close: 'ssh_close', closed: 'ssh-closed' };
  /** Ce que le cœur dit d'un terminal local ouvert : son shell et son dossier de départ. */
  let opened = $state<{ shell: string; cwd: string } | null>(null);
  const onWindows = navigator.userAgent.includes('Windows');

  // --- Journal (charte, 9.15, « The journal of a session ») -----------------------------------
  const recording = $derived(!!session.journal?.live);
  /** La clé du journal dans le cœur : celle de la session en cours. */
  const journalKey = () => `${local ? 'local' : 'ssh'}:${sessionId}`;
  const stem = () => (local ? 'terminal-local' : host.alias || host.hostname);
  const title = () => (local ? 'Journal du terminal local' : `Journal de ${host.alias || host.hostname}`);

  /** Le journal à ouvrir avec cette connexion : celui de l'onglet tant qu'on ne l'a pas arrêté,
   *  avec une ligne de reprise s'il a déjà servi ; ou un neuf, pour un hôte qui enregistre chaque
   *  session. */
  async function journalToOpen(): Promise<JournalOpen | null> {
    if (session.journalStopped) return null;
    const previous = session.journal;
    // Une connexion perdue qui revient continue son fichier, après une ligne de reprise.
    if (previous?.resumable) return { path: previous.path, header: previous.header, note: previous.opened ? resumed() : null };
    // Une session nouvelle : l'en-tête ne parle plus du journal de la précédente.
    session.journal = null;
    // Sinon, c'est une session nouvelle : un fichier neuf, pour un hôte qui enregistre chacune.
    if (local || !host.logSessions) return null;
    session.journal = { ...(await newJournal(stem(), title())), live: false, opened: false, resumable: true };
    return { path: session.journal.path, header: session.journal.header, note: null };
  }

  /** Le cœur a ouvert le journal demandé, ou dit pourquoi il n'a pas pu : la session s'ouvre quand
   *  même. L'onglet ne dit « en cours » qu'une fois le fichier ouvert. */
  function journalOpened(asked: JournalOpen | null, failed: string | null | undefined) {
    if (!asked || !session.journal) return;
    if (failed) {
      session.journal = null;
      toasts.warn('Le journal n’a pas pu être ouvert', failed);
    } else {
      session.journal.live = true;
      session.journal.opened = true;
    }
  }

  /** La session s'est terminée : le cœur a fermé le fichier, l'en-tête dit où il est. Après une
   *  connexion perdue, « Reconnecter » le reprendra ; après une fin normale, non. */
  function journalEnded(lost = false) {
    if (!lost) session.journalStopped = false; // la session suivante enregistre de nouveau
    if (!session.journal) return;
    session.journal.live = false;
    session.journal.resumable = lost && session.journal.resumable;
  }

  /** Les mots de l'en-tête, après le nom : ils cèdent la place aux boutons, coupés par une
   *  ellipse, et l'infobulle les redonne en entier (charte, 3.4 et 9.15). */
  type Word = { text: string; mono?: boolean; before?: string };
  const metaWords = $derived.by((): Word[] => {
    // Le shell et son dossier de départ en police de code (charte, 9.15).
    if (local) return opened ? [{ text: opened.shell, mono: true }, { before: 'ouvert dans ', text: opened.cwd, mono: true }] : [];
    if (phase.step === 'connected' && info) {
      const words: Word[] = [
        { text: info.knownHost ? 'serveur connu' : 'empreinte acceptée pour cette session' },
        { text: info.authMethod },
        { text: `connexion en ${Math.round(info.connectMs)} ms` },
      ];
      if (echo !== null) words.push({ text: `écho ${Math.round(echo)} ms` });
      return words;
    }
    if (phase.step === 'connecting') return [{ text: session.status === 'reconnecting' ? 'reconnexion…' : 'connexion…' }];
    return [];
  });
  const metaTitle = $derived(metaWords.map((w) => `${w.before ?? ''}${w.text}`).join(' · '));

  /** « Enregistrer le journal » : commence, avec ce que le terminal montre déjà, ou s'arrête. */
  async function toggleJournal() {
    if (sessionId === null) return;
    if (session.journal?.live) {
      const path = await invoke<string | null>('journal_stop', { key: journalKey() }).catch(() => null);
      session.journal.live = false;
      session.journalStopped = true;
      toasts.ok('Journal enregistré', path ?? session.journal.path);
      return;
    }
    try {
      const fresh = await newJournal(stem(), title());
      const path = await invoke<string>('journal_start', { key: journalKey(), open: { ...fresh, backlog: term ? shown(term) : null } });
      session.journal = { ...fresh, path, live: true, opened: true, resumable: true };
      session.journalStopped = false;
    } catch (e) {
      toasts.warn('Le journal n’a pas pu être ouvert', typeof e === 'string' ? e : String(e));
    }
  }

  // Bascule demandée par la palette ou un raccourci.
  $effect(() => {
    if (!session.journalAsk) return;
    untrack(() => void toggleJournal());
  });

  /** Lance le shell de cette machine dans un pseudo-terminal. */
  async function openLocal() {
    if (!term || !fit) return;
    phase = { step: 'connecting' };
    session.status = 'connecting';
    fit.fit();
    const onData = new Channel<ArrayBuffer | number[]>();
    onData.onmessage = (chunk) => {
      term?.write(new Uint8Array(chunk));
      onoutput?.();
    };
    try {
      const journal = await journalToOpen();
      const result = await invoke<{ id: number; shell: string; cwd: string; journalFailed?: string | null }>('local_open', {
        cols: term.cols,
        rows: term.rows,
        cwd: session.cwd || null,
        journal,
        onData,
      });
      journalOpened(journal, result.journalFailed);
      sessionId = result.id;
      opened = { shell: result.shell, cwd: result.cwd };
      session.opened = opened;
      session.localId = result.id;
      typed = false;
      phase = { step: 'connected' };
      session.status = 'connected';
      if (autofocus || relaunched) term.focus();
      relaunched = false;
    } catch (e) {
      session.status = 'closed';
      const what = onWindows ? 'Vérifie que PowerShell est installé.' : 'Vérifie la variable SHELL de ton compte.';
      phase = { step: 'error', message: `${typeof e === 'string' ? e : String(e)} ${what}` };
    }
  }
  const target = $derived(`${host.user}@${host.hostname}${host.port !== 22 ? `:${host.port}` : ''}`);

  function median(values: number[]) {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)];
  }

  function normalize(e: unknown): SshError {
    if (e && typeof e === 'object' && 'kind' in e) return e as SshError;
    return { kind: 'other', message: typeof e === 'string' ? e : String(e) };
  }

  /** Ouvre la session. `auto` : numéro de la tentative de reconnexion automatique, 0 sinon. */
  async function connect(next: Attempt = {}, auto = 0) {
    if (!term || !fit) return;
    if (local) return openLocal();
    retry = auto;
    attempt = { ...attempt, ...next };
    if (host.proxyJump) {
      phase = { step: 'error', message: 'Cet hôte passe par un bastion (ProxyJump). Le rebond par bastion n’est pas encore pris en charge.' };
      session.status = 'closed';
      return;
    }
    if (!host.user) {
      phase = { step: 'error', message: 'Cet hôte n’a pas d’utilisateur : ajoute-le dans sa fiche.' };
      session.status = 'closed';
      return;
    }
    phase = { step: 'connecting' };
    session.status = auto ? 'reconnecting' : 'connecting';
    secret = '';
    fit.fit();
    echoes = [];
    echo = null;

    const onData = new Channel<ArrayBuffer | number[]>();
    onData.onmessage = (chunk) => {
      if (lastKey) {
        echoes.push(performance.now() - lastKey);
        lastKey = 0;
        if (echoes.length > 40) echoes.shift();
        echo = median(echoes);
      }
      term?.write(new Uint8Array(chunk));
    };

    const keyPath = attempt.keyPath ?? (host.auth === 'keyFile' ? host.keyPath : '');
    const auth =
      host.auth === 'password'
        ? // Sans mot de passe tapé, le cœur prend celui que le trousseau retient, ou le demande.
          { kind: 'password', password: attempt.password ?? null }
        : keyPath
          ? { kind: 'keyFile', path: keyPath, passphrase: attempt.passphrase ?? null }
          : { kind: 'auto' };

    const journal = await journalToOpen();
    try {
      const connected = await invoke<Connected>('ssh_connect', {
        params: {
          host: host.hostname,
          port: host.port || 22,
          user: host.user,
          auth,
          cols: term.cols,
          rows: term.rows,
          acceptFingerprint: attempt.accept ?? null,
          rememberHost: attempt.remember ?? false,
          startupCommand: host.startupCommand.trim() || null,
          rememberSecret: attempt.keep ?? false,
          journal,
        },
        onData,
      });
      journalOpened(journal, connected.journalFailed);
      info = connected;
      sessionId = connected.id;
      phase = { step: 'connected' };
      session.status = 'connected';
      // Les secrets ne survivent pas à l'ouverture de la session.
      attempt = {};
      if (connected.learnFailed) toasts.warn('L’empreinte n’a pas pu être mémorisée', connected.learnFailed);
      if (connected.keepFailed) toasts.warn('Le secret n’a pas pu être retenu', `Le trousseau du système l’a refusé${NB}: ${connected.keepFailed}`);
      retry = 0;
      term.focus();
    } catch (e) {
      const problem = normalize(e);
      session.status = 'closed';
      if (problem.kind === 'hostKeyUnknown') {
        phase = { step: 'hostKey', fingerprint: problem.fingerprint, algorithm: problem.algorithm };
      } else if (problem.kind === 'hostKeyChanged') {
        phase = { step: 'hostKeyChanged', fingerprint: problem.fingerprint, line: problem.line };
      } else if (problem.kind === 'needPassphrase') {
        attempt.passphrase = undefined;
        phase = {
          step: 'passphrase',
          path: problem.path,
          error:
            problem.refused === 'typed'
              ? 'Cette phrase secrète ne déverrouille pas la clé.'
              : problem.refused === 'retained'
                ? 'La phrase secrète retenue ne déverrouille plus la clé. Elle a été oubliée.'
                : '',
        };
      } else if (problem.kind === 'needPassword') {
        // Rien de retenu, ou ce qui l'était a été refusé : la série de tentatives s'arrête, l'onglet
        // attend l'utilisateur.
        retry = 0;
        attempt.password = undefined;
        phase = { step: 'password', error: problem.refused ? 'Le serveur a refusé le mot de passe retenu. Il a été oublié.' : '' };
      } else if (problem.kind === 'auth' && host.auth === 'password') {
        attempt.password = undefined;
        phase = { step: 'password', error: problem.message };
      } else if (auto) {
        // Le serveur ne répond toujours pas : la tentative suivante est programmée.
        scheduleRetry(auto + 1, problem.message);
      } else {
        // Le motif du système, dit en français quand on le connaît (charte, section 9.15).
        phase = { step: 'error', message: explain(problem.message) };
      }
    }
  }

  function submitSecret(e: SubmitEvent) {
    e.preventDefault();
    // Sans trousseau, rien n'est proposé : rien n'est retenu.
    const kept = keep && keyring === true;
    if (phase.step === 'passphrase') void connect({ keyPath: phase.path, passphrase: secret, keep: kept });
    else if (phase.step === 'password') void connect({ password: secret, keep: kept });
  }

  /** « Relancer » vient d'être cliqué : le terminal rouvert reprend le clavier, même sous le graphe. */
  let relaunched = false;

  function reconnect() {
    relaunched = true;
    window.clearTimeout(retryTimer);
    attempt = {};
    term?.reset();
    void connect();
  }

  onMount(() => {
    const css = getComputedStyle(document.documentElement);
    const v = (name: string) => css.getPropertyValue(name).trim();
    // Les 16 couleurs ANSI, dans l'ordre de xterm.js : 0 à 7, puis leurs versions vives (charte, 2.6).
    const ANSI = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'] as const;
    const bright = (name: string) => `bright${name[0].toUpperCase()}${name.slice(1)}`;
    const theme = () => ({
      background: v('--term-bg'),
      foreground: v('--term-ink'),
      cursor: v('--accent'),
      selectionBackground: v('--sel'),
      // La marge des résultats de recherche, sans trait sur son bord.
      overviewRulerBorder: v('--term-bg'),
      ...Object.fromEntries(ANSI.flatMap((name, i) => [[name, v(`--ansi-${i}`)], [bright(name), v(`--ansi-${i + 8}`)]])),
    });

    const terminal = new Terminal({
      // La recherche surligne ses résultats par des décorations, une API encore « proposée ».
      allowProposedApi: true,
      overviewRuler: { width: 8 },
      fontFamily: v('--f-mono'),
      fontSize: parseFloat(v('--fs-term')),
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
      theme: theme(),
      // Filet de sécurité : un texte d'une couleur choisie par le programme (256 couleurs, couleurs
      // vraies, ou le noir et le blanc qui servent de fonds) est éclairci ou foncé jusqu'à 4,5:1 sur
      // le fond de sa cellule.
      minimumContrastRatio: 4.5,
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    const searchAddon = new SearchAddon();
    terminal.loadAddon(searchAddon);
    searchAddon.onDidChangeResults((e) => (results = { index: e.resultIndex, count: e.resultCount }));
    search = searchAddon;
    if (termEl) terminal.open(termEl);
    term = terminal;
    fit = fitAddon;

    terminal.onData((data) => {
      if (sessionId === null) return;
      if (!lastKey) lastKey = performance.now();
      typed = true;
      void invoke(API.write, { id: sessionId, data }).catch(() => {});
    });
    // Le pseudo-terminal ne suit qu'une fois le geste fini : pendant qu'on glisse une poignée, le
    // shell redessinerait son invite à chaque image.
    let resizeTimer: number | undefined;
    terminal.onResize(({ cols, rows }) => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        if (sessionId !== null) void invoke(API.resize, { id: sessionId, cols, rows }).catch(() => {});
      }, 120);
    });

    const observer = new ResizeObserver(() => {
      // Onglet masqué : la taille est nulle, on ne recalcule rien.
      if (termEl && termEl.offsetParent !== null && termEl.clientWidth > 0) fitAddon.fit();
    });
    if (termEl) observer.observe(termEl);

    // Le terminal lit ses couleurs dans les jetons : il les relit quand le thème change.
    const stopTheme = $effect.root(() => {
      $effect(() => {
        void appTheme.key;
        terminal.options.theme = theme();
      });
    });

    const unlisten = listen<Closed>(API.closed, (event) => {
      if (event.payload.id !== sessionId) return;
      sessionId = null;
      info = null;
      const { exitStatus, reason, ending, signal } = event.payload;
      // Terminal local : un shell qu'on quitte (`exit`, Ctrl + D) ferme son onglet, quel que soit son
      // code de sortie, qui n'est que celui de la dernière commande. L'onglet reste, avec « Relancer »,
      // quand le shell est arrêté par un signal, ou s'arrête en erreur avant qu'on y ait rien tapé.
      if (local) {
        session.localId = undefined;
        journalEnded();
        if (!signal && (exitStatus === 0 || typed)) return onclose();
        session.status = 'closed';
        const how = signal ? ` (signal${NB}: ${signal})` : exitStatus === null ? '' : ` (code de sortie ${exitStatus})`;
        phase = {
          step: 'closed',
          message: `Le shell s’est arrêté${how}. Relance-le, ou ferme l’onglet.`,
          brief: `Shell arrêté${how}.`,
        };
        return;
      }
      // Seule une connexion perdue se rouvre d'elle-même, et seulement si le réglage le permet.
      if (ending === 'lost' && prefs.sshReconnect) {
        term?.write('\r\n');
        scheduleRetry(1, reason ?? '');
        return;
      }
      journalEnded(ending === 'lost');
      session.status = 'closed';
      phase =
        ending === 'lost'
          ? { step: 'closed', lost: true, message: reason ? explain(reason) : 'La connexion au serveur a été coupée.' }
          : {
              step: 'closed',
              message: reason
                ? `Connexion interrompue\u00a0: ${reason}`
                : exitStatus === null || exitStatus === 0
                  ? 'Le serveur a fermé la session.'
                  : `Session terminée (code de sortie ${exitStatus}).`,
            };
    });

    void connect();

    return () => {
      observer.disconnect();
      window.clearTimeout(resizeTimer);
      stopTheme();
      void unlisten.then((stop) => stop());
      window.clearTimeout(retryTimer);
      if (sessionId !== null) void invoke(API.close, { id: sessionId }).catch(() => {});
      terminal.dispose();
    };
  });

  $effect(() => {
    // À l'affichage de l'onglet : la taille du terminal a pu changer pendant qu'il était masqué.
    if (!active) return;
    const connected = phase.step === 'connected';
    requestAnimationFrame(() => {
      fit?.fit();
      if (connected && autofocus) term?.focus();
    });
  });
</script>

<div class="session" bind:this={rootEl}>
  {#if !bare}
  <header>
    {#if local}
      <!-- Un terminal local : l'ordinateur, comme partout où « local » est dit (charte, section 7). -->
      <span class="what"><Icon name="laptop" size={14} /> <b>Terminal local</b></span>
    {:else}
      {#if session.status === 'connecting'}
        <!-- La connexion s'ouvre : l'anneau d'attente tient la place du point, comme sur l'onglet.
             « connexion… » le dit à côté : l'anneau lui-même n'est pas lu. -->
        <span class="spinner" aria-hidden="true"></span>
      {:else}
        <span class="dot s-{session.status}" role="img" aria-label={STATUS[session.status]} title={STATUS[session.status]}></span>
      {/if}
      <b class="mono">{target}</b>
    {/if}
    <!-- Les mots de l'en-tête cèdent la place avant les boutons, coupés par une ellipse (9.15) : un
         seul texte, séparé par des espaces cadratins, pour que l'ellipse se dessine. -->
    <span class="meta" title={metaTitle}
      >{#each metaWords as word, i (i)}{#if i}{'\u2003'}{/if}{word.before ?? ''}<span class:mono={word.mono}>{word.text}</span>{/each}</span
    >
    {#if session.journal}
      <span class="journal" title={session.journal.path}>{recording ? 'journal en cours' : 'journal enregistré'}</span>
    {/if}
    {#if phase.step === 'connected' && (local || info)}
      <button
        class="btn ghost small toggle"
        type="button"
        aria-pressed={recording}
        title={recording ? 'Arrête l’enregistrement\u00a0: le fichier reste.' : 'Enregistre ce qui est affiché et la suite, dans un fichier texte.'}
        onclick={() => void toggleJournal()}
      >
        Enregistrer le journal
      </button>
    {/if}
    {#if !local && phase.step === 'connected' && info}
      <button class="btn small" type="button" onclick={() => sessionId !== null && void invoke(API.close, { id: sessionId })}>Déconnecter</button>
    {/if}
  </header>
  {/if}

  {#if session.searching}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="search" role="search" aria-label="Chercher dans le terminal" onkeydown={onBarKey}>
      <label class="filter" class:invalid>
        <Icon name="search" size={14} />
        <input
          bind:this={searchEl}
          type="search"
          placeholder="Chercher dans le terminal"
          aria-label="Chercher dans le terminal"
          aria-invalid={invalid ? 'true' : undefined}
          aria-describedby="search-count-{session.key}"
          spellcheck="false"
          autocomplete="off"
          bind:value={query}
          oninput={() => find('next', true)}
          onkeydown={onFieldKey}
        />
      </label>
      {#if invalid}
        <span class="count bad" id="search-count-{session.key}" role="alert">Expression non valide</span>
      {:else}
        <span class="count" id="search-count-{session.key}" role="status">{countText}</span>
      {/if}
      <button class="btn ghost small" type="button" aria-label="Résultat précédent" title="Résultat précédent (Maj + Entrée)" disabled={!results?.count} onclick={() => find('previous')}>
        <Icon name="up" size={14} />
      </button>
      <button class="btn ghost small" type="button" aria-label="Résultat suivant" title="Résultat suivant (Entrée)" disabled={!results?.count} onclick={() => find('next')}>
        <Icon name="down" size={14} />
      </button>
      <button class="btn ghost small toggle" type="button" aria-pressed={caseSensitive} title="Respecter la casse" aria-label="Respecter la casse" onclick={() => (caseSensitive = !caseSensitive)}>Aa</button>
      <button class="btn ghost small toggle mono" type="button" aria-pressed={regex} title="Expression régulière" aria-label="Expression régulière" onclick={() => (regex = !regex)}>.*</button>
      <button class="btn ghost small close" type="button" aria-label="Fermer la recherche" title="Fermer la recherche (Échap)" onclick={closeSearch}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <div class="stage">
    <div class="term" bind:this={termEl}></div>

    {#if phase.step !== 'connected' && !strip}
      <div class="veil">
        <div class="card" class:bad={phase.step === 'hostKeyChanged'}>
          {#if phase.step === 'connecting'}
            <span class="spinner wait" aria-hidden="true"></span>
            <p class="lead">
              {#if local}Ouverture du terminal local…{:else}{retry ? 'Reconnexion' : 'Connexion'} à <b class="mono">{target}</b>…{/if}
            </p>
          {:else if phase.step === 'hostKey'}
            <h3><Icon name="warning" /> Serveur inconnu</h3>
            <p>
              <b class="mono">{host.hostname}</b> n’est pas dans <code>known_hosts</code>. Vérifie que cette empreinte
              {phase.algorithm} est bien celle du serveur avant de lui faire confiance :
            </p>
            <p class="print mono">{phase.fingerprint}</p>
            <div class="row">
              <button class="btn primary" type="button" onclick={() => void connect({ accept: phase.step === 'hostKey' ? phase.fingerprint : undefined, remember: true })}>
                Faire confiance et mémoriser
              </button>
              <button class="btn" type="button" onclick={() => void connect({ accept: phase.step === 'hostKey' ? phase.fingerprint : undefined, remember: false })}>
                Faire confiance cette fois
              </button>
              <button class="btn ghost" type="button" onclick={onclose}>Annuler</button>
            </div>
          {:else if phase.step === 'hostKeyChanged'}
            <h3><Icon name="warning" /> L’empreinte du serveur a changé</h3>
            <p>
              L’empreinte présentée par <b class="mono">{host.hostname}</b> ne correspond plus à celle enregistrée à la ligne
              {phase.line} de <code>known_hosts</code>. La connexion est bloquée : le serveur a peut-être été réinstallé, ou
              quelqu’un s’interpose entre toi et lui.
            </p>
            <p class="print mono">{phase.fingerprint}</p>
            <p class="muted">Si le changement est légitime, retire l’ancienne ligne de <code>known_hosts</code>, puis reconnecte-toi.</p>
            <div class="row">
              <button class="btn" type="button" onclick={reconnect}>Réessayer</button>
              <button class="btn ghost" type="button" onclick={onclose}>Fermer l’onglet</button>
            </div>
          {:else if phase.step === 'passphrase' || phase.step === 'password'}
            <form onsubmit={submitSecret}>
              <h3>{phase.step === 'passphrase' ? 'Phrase secrète de la clé' : `Mot de passe de ${host.user}`}</h3>
              <p class="muted">
                {#if phase.step === 'passphrase'}
                  La clé <span class="mono">{phase.path}</span> est protégée.
                {:else}
                  Pour <span class="mono">{target}</span>.
                {/if}
              </p>
              <!-- Secret refusé : le champ passe en erreur, son message dessous (charte, section 9.3). -->
              <div class="secret">
                <input
                  class="field"
                  class:invalid={!!phase.error}
                  type="password"
                  autocomplete="off"
                  bind:this={secretEl}
                  aria-label={phase.step === 'passphrase' ? 'Phrase secrète' : 'Mot de passe'}
                  aria-invalid={phase.error ? 'true' : undefined}
                  aria-describedby={phase.error ? `secret-error-${session.key}` : undefined}
                  bind:value={secret}
                />
                {#if phase.error}<p class="err small" id="secret-error-{session.key}" role="alert">{phase.error}</p>{/if}
              </div>
              <!-- Retenir le secret : proposé si le trousseau du système répond ; sinon on le dit. -->
              {#if keyring}
                <label class="keep">
                  <input type="checkbox" bind:checked={keep} />
                  <span>Retenir dans le trousseau du système</span>
                </label>
              {:else if keyring === false}
                <p class="muted">{NO_KEYRING[phase.step]}</p>
              {/if}
              <div class="row">
                <button class="btn primary" type="submit" disabled={!secret}>Se connecter</button>
                <button class="btn ghost" type="button" onclick={onclose}>Annuler</button>
              </div>
            </form>
          {:else if phase.step === 'lost'}
            <h3><Icon name="warning" /> Connexion perdue</h3>
            <!-- Le compte à rebours change chaque seconde : il n'est pas lu, seule l'annonce l'est. -->
            <p aria-hidden="true">{waitText}</p>
            <p class="spoken" role="status">{announce}</p>
            <!-- La ligne du motif garde sa place : les boutons ne bougent pas d'une tentative à l'autre. -->
            <p class="muted reason">{phase.reason || NB}</p>
            <div class="row">
              <button class="btn primary" type="button" bind:this={retryButton} onclick={retryNow}><Icon name="refresh" size={14} /> Reconnecter maintenant</button>
              <button class="btn ghost" type="button" onclick={stopRetrying}>Arrêter</button>
            </div>
          {:else}
            <h3>
              {#if local}Shell arrêté{:else}{phase.step === 'error'
                  ? 'Connexion impossible'
                  : phase.lost
                    ? 'Connexion perdue'
                    : 'Session terminée'}{/if}
            </h3>
            <!-- Un échec est annoncé aussitôt : le focus part sur le bouton, le motif ne serait pas lu. -->
            <p class:err={phase.step === 'error'} role={phase.step === 'error' ? 'alert' : undefined}>{phase.message}</p>
            <div class="row">
              <!-- Un terminal local n'est jamais « reconnecté » : il est relancé. -->
              <button class="btn primary" type="button" bind:this={endButton} onclick={reconnect}><Icon name="refresh" size={14} /> {local ? 'Relancer' : 'Reconnecter'}</button>
              <button class="btn ghost" type="button" onclick={onclose}>Fermer l’onglet</button>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  {#if strip && phase.step !== 'connected'}
    <!-- Sous le graphe, ni voile ni carte : une bande d'une ligne, qui laisse lire les dernières
         lignes de sortie, celles qui disent pourquoi le shell s'est arrêté (charte, section 9.20). -->
    <div class="strip">
      {#if phase.step === 'connecting'}
        <span class="spinner" aria-hidden="true"></span>
        <span class="said" role="status">Ouverture du terminal local…</span>
      {:else if phase.step === 'closed' || phase.step === 'error'}
        {@const said = phase.step === 'closed' ? (phase.brief ?? phase.message) : phase.message}
        <span class="said" class:bad={phase.step === 'error'} role="alert" title={said}>{said}</span>
        <button class="btn primary small" type="button" bind:this={endButton} onclick={reconnect}>Relancer</button>
        <button class="btn ghost small" type="button" onclick={onclose}>Fermer l’onglet</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .session {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  /* Barre de recherche : une bande sous l'en-tête, qui pousse le terminal sans cacher son texte. */
  .search {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line-soft);
    background: var(--panel);
  }
  .filter {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 240px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .filter:hover:not(:focus-within) {
    border-color: color-mix(in srgb, var(--muted) 50%, var(--line));
  }
  .filter:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .filter.invalid,
  .filter.invalid:focus-within {
    border-color: var(--bad);
  }
  .filter.invalid:focus-within {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--bad) 25%, transparent);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
    padding: 5px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  /* Assez large pour « Expression non valide » : les boutons ne bougent pas quand le texte change. */
  .count {
    min-width: 124px;
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .count.bad {
    color: var(--bad);
  }
  /* Bascule : un bouton fantôme, enfoncé en --accent-soft bordé d'accent (charte, section 9.1). */
  .toggle[aria-pressed='true'],
  .toggle[aria-pressed='true']:hover {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .close {
    margin-left: auto;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    /* Hauteur fixe : celle d'un en-tête avec son bouton « Déconnecter ». Le terminal ne saute pas
       d'un onglet à l'autre. */
    height: 35px;
    padding: 5px 12px;
    border-bottom: 1px solid var(--line-soft);
    color: var(--muted);
    font-size: var(--fs-label);
    white-space: nowrap;
    overflow: hidden;
  }
  header b {
    color: var(--ink);
  }
  .meta {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  header .btn,
  .journal {
    flex: none;
  }
  /* L'en-tête parle en --muted ; ses boutons gardent le texte d'un bouton (9.1). */
  header .btn {
    color: var(--ink);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .dot.s-connected {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 25%, transparent);
  }
  .dot.s-reconnecting {
    background: var(--warn);
    animation: blink var(--t-wait) var(--ease) infinite;
  }
  header > .spinner {
    margin: -2px;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .stage {
    flex: 1;
    position: relative;
    min-height: 0;
    background: var(--term-bg);
  }
  .term {
    position: absolute;
    inset: 8px 4px 4px 10px;
  }
  /* Terminal d'un dépôt : l'état du shell tient en une ligne, sous le terminal. */
  .strip {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-top: 1px solid var(--line-soft);
    background: var(--panel);
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .said {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .said.bad {
    color: var(--bad);
  }
  .veil {
    position: absolute;
    inset: 0;
    /* Dans un terminal bas (sous le graphe), le panneau défile plutôt que de déborder ; ses marges
       automatiques le centrent sans rogner son haut quand il dépasse. */
    display: flex;
    padding: 24px;
    overflow: auto;
    background: color-mix(in srgb, var(--term-bg) 82%, transparent);
    backdrop-filter: blur(1.5px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .card {
    margin: auto;
    /* 560 px : les trois boutons de l’empreinte tiennent sur une ligne. */
    width: min(560px, 100%);
    display: grid;
    gap: 10px;
    padding: 16px 20px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    animation: pop-in var(--t-med) var(--ease);
  }
  .card.bad {
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  .card form {
    display: grid;
    gap: 10px;
  }
  /* La case et son texte sont la cible : pas plus larges que ce texte. */
  .keep {
    justify-self: start;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .lead {
    text-align: center;
  }
  .muted {
    color: var(--muted);
  }
  .err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .err.small {
    font-size: var(--fs-small);
  }
  /* Le champ d'un secret et, dessous, le message de son refus. */
  .secret {
    display: grid;
    gap: 4px;
  }
  .print {
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: var(--fs-code);
    user-select: all;
  }
  code {
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .wait {
    justify-self: center;
  }
  /* Hors de l'écran, mais lu : l'annonce d'une tentative de reconnexion. */
  .spoken {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .what {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
