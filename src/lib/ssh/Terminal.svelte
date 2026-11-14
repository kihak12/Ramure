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
  import { ssh, STATUS, type Session } from './ssh.svelte';
  import TunnelStrip from './TunnelStrip.svelte';
  import { focusWithRing } from '../ui/focus';
  import { toasts } from '../ui/toasts.svelte';
  import { keyringAvailable, NO_KEYRING } from './secrets';
  import { explain } from './reasons';
  import { newJournal, resumed, shown, type JournalOpen } from './journal';
  import { addressOf, tunnelLabel, type Closed, type Connected, type SshError } from './types';

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
    /** No header: the panel that holds it says itself what the terminal is (guidelines, 9.20). */
    bare?: boolean;
    /** The terminal takes the keyboard when its tab shows. Under the graph, never on its own. */
    autofocus?: boolean;
    /** Called on every output of the shell: the repository is read again once it goes quiet. */
    onoutput?: () => void;
  } = $props();

  /** What the session waits for from the user, or its state. */
  type Phase =
    | { step: 'connecting' }
    | { step: 'connected' }
    | { step: 'hostKey'; fingerprint: string; algorithm: string }
    | { step: 'hostKeyChanged'; fingerprint: string; line: number }
    | { step: 'passphrase'; path: string; error: string }
    | { step: 'password'; error: string }
    | { step: 'error'; message: string }
    /** `brief`: the same in one line, for the strip of a repository terminal. */
    | { step: 'closed'; message: string; lost?: boolean; brief?: string }
    /** Connection lost: attempt `attempt` (1 to 5) starts at the instant `at`. */
    | { step: 'lost'; attempt: number; at: number; reason: string };

  // Delays between two reconnection attempts, in seconds (guidelines, section 9.15).
  const RETRIES = [2, 5, 10, 20, 30];

  /** What is known already for this attempt: nothing is kept once the session is open. */
  interface Attempt {
    accept?: string;
    remember?: boolean;
    keyPath?: string;
    passphrase?: string;
    password?: string;
    /** Keep the typed secret in the system keyring, once it has worked. */
    keep?: boolean;
  }

  let phase = $state<Phase>({ step: 'connecting' });
  // The system keyring: when it responds, the panel that asks for a secret offers to keep it (9.15).
  let keyring = $state<boolean | null>(null);
  let keep = $state(false);
  // Asked as soon as a session opens, so that the panel asking for a secret shows in one piece, and
  // again when it shows: the keyring may have come or gone in the meantime.
  $effect(() => {
    if (session.kind !== 'ssh') return;
    void phase.step;
    if (phase.step !== 'connecting' && phase.step !== 'passphrase' && phase.step !== 'password') return;
    void keyringAvailable().then((answer) => (keyring = answer));
  });
  // A secret is asked for: its field takes the keyboard as soon as the tab is on screen. The
  // `autofocus` attribute is not enough: it only acts when nothing else has the focus, and the row of
  // the host that was just double-clicked still has it.
  let secretEl = $state<HTMLInputElement>();
  $effect(() => {
    if ((phase.step !== 'passphrase' && phase.step !== 'password') || !active) return;
    void tick().then(() => secretEl?.focus());
  });
  // Automatic reconnection: the current attempt (0 when there is none), its timer, and the time,
  // refreshed every second for the countdown.
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
    const which = `${phase.attempt} of ${RETRIES.length}`;
    return seconds > 0 ? `Next attempt in ${seconds} s, ${which}.` : `Attempt ${which}…`;
  });
  // What a screen reader hears: once per attempt, not every second.
  const announce = $derived.by(() => {
    if (phase.step !== 'lost') return '';
    const delay = RETRIES[phase.attempt - 1];
    return `Connection lost. Attempt ${phase.attempt} of ${RETRIES.length} in ${delay} second${delay === 1 ? '' : 's'}.`;
  });
  let retryButton = $state<HTMLButtonElement>();
  let endButton = $state<HTMLButtonElement>();
  // An end panel appears: the focus goes to its main button, which the keyboard could not reach
  // otherwise, since the terminal keeps the Tab key (guidelines, section 9.15).
  let rootEl = $state<HTMLElement>();
  $effect(() => {
    if (phase.step !== 'closed' && phase.step !== 'error') return;
    // Under the graph, the terminal never takes the keyboard on its own (section 9.20): the button
    // takes it only if the keyboard was already in this terminal.
    if (bare && !untrack(() => rootEl?.contains(document.activeElement))) return;
    void tick().then(() => focusWithRing(endButton));
  });
  // The card appears: the focus goes to "Reconnect now". The terminal, cut off, is of no use any
  // more, and it keeps the Tab key for itself: without this, the keyboard could not reach the card.
  $effect(() => {
    if (phase.step !== 'lost' || phase.attempt !== 1) return;
    void tick().then(() => focusWithRing(retryButton));
  });

  /** Schedules attempt `attempt`, or stops after the last one. */
  function scheduleRetry(attempt: number, reason: string) {
    window.clearTimeout(retryTimer);
    if (attempt > RETRIES.length) {
      retry = 0;
      journalEnded(true);
      ssh.tunnelsClose(session, true);
      session.status = 'closed';
      phase = { step: 'closed', lost: true, message: `All ${RETRIES.length} reconnection attempts failed${reason ? `: ${explain(reason)}` : '.'}` };
      return;
    }
    retry = attempt;
    session.status = 'reconnecting';
    const delay = RETRIES[attempt - 1] * 1000;
    now = Date.now();
    phase = { step: 'lost', attempt, at: now + delay, reason: explain(reason) };
    retryTimer = window.setTimeout(() => void connect({}, attempt), delay);
  }

  /** "Reconnect now": the planned attempt starts without waiting. */
  function retryNow() {
    if (phase.step !== 'lost') return;
    window.clearTimeout(retryTimer);
    void connect({}, phase.attempt);
  }

  /** "Stop": no more attempts, the tab waits for a click. */
  function stopRetrying() {
    window.clearTimeout(retryTimer);
    retry = 0;
    journalEnded(true);
    ssh.tunnelsClose(session, true);
    session.status = 'closed';
    phase = { step: 'closed', lost: true, message: 'Automatic reconnection stopped.' };
  }
  let info = $state.raw<Connected | null>(null);
  let echo = $state<number | null>(null);
  let secret = $state('');

  let termEl = $state<HTMLDivElement>();
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let search: SearchAddon | undefined;

  // --- Search in the terminal (guidelines, section 9.15) -------------------------------------
  const MATCH = 0.3; // share of the accent in the background of a result, as in check-contrasts.mjs
  let query = $state('');
  let caseSensitive = $state(false);
  let regex = $state(false);
  /** Results of the last search; `index` is -1 beyond 1,000 highlighted results. */
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
  // Numbers in the American format, "1,234" (guidelines, section 12).
  const numbers = new Intl.NumberFormat('en-US');
  const countText = $derived(
    !query || !results
        ? ''
        : results.count === 0
          ? 'No results'
          : results.index < 0
            ? `${numbers.format(results.count)} result${results.count === 1 ? '' : 's'}`
            : `${numbers.format(results.index + 1)} of ${numbers.format(results.count)}`,
  );

  /** `#rrggbb` mixed at `p` over `#rrggbb`: the terminal wants a solid color, not `color-mix()`. */
  function blend(hex: string, p: number, over: string) {
    const channels = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
    const [a, b] = [channels(hex), channels(over)];
    return `#${a.map((v, i) => Math.round(v * p + b[i] * (1 - p)).toString(16).padStart(2, '0')).join('')}`;
  }

  /** The colors of the results, read from the tokens when searching: they follow the theme.
   *  A result is the accent at 30% over the background of the terminal (guidelines, section 9.15). */
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

  /** Escape closes the bar, wherever the key is pressed: the field, a button or a toggle. */
  function onBarKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    closeSearch();
  }

  // Opening asked for (shortcut, palette), even with the bar already open: the field takes the focus
  // and its text is selected.
  $effect(() => {
    void session.searchAsk;
    if (!session.searching) return;
    void tick().then(() => {
      searchEl?.focus();
      searchEl?.select();
    });
  });

  // The options change: the search runs again in place.
  $effect(() => {
    void caseSensitive;
    void regex;
    if (session.searching && query) find('next', true);
  });
  // Outside reactivity: read in the callbacks of the terminal, on every keystroke.
  let sessionId: number | null = null;
  /** Something was typed in this shell: a stop on an error is then not a failure to start. */
  let typed = false;
  let lastKey = 0;
  let echoes: number[] = [];
  let attempt: Attempt = {};

  const host = $derived(session.host);
  // A local terminal talks to the Rust core through its own commands; the rest of the component is
  // shared. The kind of a tab does not change: reading it once is enough.
  // svelte-ignore state_referenced_locally
  const local = session.kind === 'local';
  // The terminal of a repository says the state of its shell in a strip, not on a veil.
  const strip = $derived(bare && local);
  const API = local
    ? { write: 'local_write', resize: 'local_resize', close: 'local_close', closed: 'local-closed' }
    : { write: 'ssh_write', resize: 'ssh_resize', close: 'ssh_close', closed: 'ssh-closed' };
  /** What the core says of an open local terminal: its shell and its starting folder. */
  let opened = $state<{ shell: string; cwd: string } | null>(null);

  // --- Log (guidelines, 9.15, "The log of a session") -----------------------------------------
  const recording = $derived(!!session.journal?.live);
  /** The key of the log in the core: that of the current session. */
  const journalKey = () => `${local ? 'local' : 'ssh'}:${sessionId}`;
  const stem = () => (local ? 'local-terminal' : host.alias || host.hostname);
  const title = () => (local ? 'Log of the local terminal' : `Log of ${host.alias || host.hostname}`);

  /** The log to open with this connection: that of the tab as long as it has not been stopped,
   *  with a resume line if it has been used already; or a new one, for a host that records every
   *  session. */
  async function journalToOpen(): Promise<JournalOpen | null> {
    if (session.journalStopped) return null;
    const previous = session.journal;
    // A lost connection that comes back goes on in its file, after a resume line.
    if (previous?.resumable) return { path: previous.path, header: previous.header, note: previous.opened ? resumed() : null };
    // A new session: the header no longer speaks of the log of the previous one.
    session.journal = null;
    // Otherwise, it is a new session: a new file, for a host that records each one.
    if (local || !host.logSessions) return null;
    session.journal = { ...(await newJournal(stem(), title())), live: false, opened: false, resumable: true };
    return { path: session.journal.path, header: session.journal.header, note: null };
  }

  /** The core has opened the log asked for, or says why it could not: the session opens all the
   *  same. The tab says "recording" only once the file is open. */
  function journalOpened(asked: JournalOpen | null, failed: string | null | undefined) {
    if (!asked || !session.journal) return;
    if (failed) {
      session.journal = null;
      toasts.warn('The log could not be opened', failed);
    } else {
      session.journal.live = true;
      session.journal.opened = true;
    }
  }

  /** The session has ended: the core has closed the file, the header says where it is. After a
   *  lost connection, "Reconnect" will resume it; after a normal end, it will not. */
  function journalEnded(lost = false) {
    if (!lost) session.journalStopped = false; // the next session records again
    if (!session.journal) return;
    session.journal.live = false;
    session.journal.resumable = lost && session.journal.resumable;
  }

  /** The words of the header, after the name: they give way to the buttons, cut with an
   *  ellipsis, and the tooltip gives them whole (guidelines, 3.4 and 9.15). */
  type Word = { text: string; mono?: boolean; before?: string };
  const metaWords = $derived.by((): Word[] => {
    // The shell and its starting folder in the code font (guidelines, 9.15).
    if (local) return opened ? [{ text: opened.shell, mono: true }, { before: 'opened in ', text: opened.cwd, mono: true }] : [];
    if (phase.step === 'connected' && info) {
      const words: Word[] = [
        { text: info.knownHost ? 'known server' : 'fingerprint accepted for this session' },
        { text: info.authMethod },
        { text: `connected in ${Math.round(info.connectMs)} ms` },
      ];
      if (echo !== null) words.push({ text: `echo ${Math.round(echo)} ms` });
      return words;
    }
    if (phase.step === 'connecting') return [{ text: session.status === 'reconnecting' ? 'reconnecting…' : 'connecting…' }];
    return [];
  });
  const metaTitle = $derived(metaWords.map((w) => `${w.before ?? ''}${w.text}`).join(' · '));

  /** "Record log": starts, with what the terminal shows already, or stops. */
  async function toggleJournal() {
    if (sessionId === null) return;
    if (session.journal?.live) {
      const path = await invoke<string | null>('journal_stop', { key: journalKey() }).catch(() => null);
      session.journal.live = false;
      session.journalStopped = true;
      toasts.ok('Log saved', path ?? session.journal.path);
      return;
    }
    try {
      const fresh = await newJournal(stem(), title());
      const path = await invoke<string>('journal_start', { key: journalKey(), open: { ...fresh, backlog: term ? shown(term) : null } });
      session.journal = { ...fresh, path, live: true, opened: true, resumable: true };
      session.journalStopped = false;
    } catch (e) {
      toasts.warn('The log could not be opened', typeof e === 'string' ? e : String(e));
    }
  }

  // --- Tunnels (guidelines, 9.15, "Tunnels"): the session's state is in `ssh`, the strip in TunnelStrip.
  const tunnels = $derived(local ? [] : ssh.tunnelsOf(session));
  /** The toggle is there while the group of the session holds its tunnels, connected or waiting. */
  const tunnelsLive = $derived(!!session.tunnels?.live);
  const tunnelsFailed = $derived(tunnels.filter((row) => row.state.state === 'failed'));
  // With a failure, the tooltip of the toggle says why.
  const tunnelsTitle = $derived(
    tunnelsFailed.length
      ? tunnelsFailed.map((row) => `${tunnelsFailed.length > 1 ? `${tunnelLabel(row.tunnel)}: ` : ''}${row.state.error ?? 'Failed'}`).join('\n')
      : 'Show or hide tunnels',
  );

  // Toggle asked for by the palette or a shortcut.
  $effect(() => {
    if (!session.journalAsk) return;
    untrack(() => void toggleJournal());
  });

  /** Starts the shell of this machine in a pseudo-terminal. */
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
      // The core says what failed and, when the shell could not start, what to check.
      phase = { step: 'error', message: typeof e === 'string' ? e : String(e) };
    }
  }
  const target = $derived(addressOf(host.user, host.hostname, host.port));

  function median(values: number[]) {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)];
  }

  function normalize(e: unknown): SshError {
    if (e && typeof e === 'object' && 'kind' in e) return e as SshError;
    return { kind: 'other', message: typeof e === 'string' ? e : String(e) };
  }

  /** Opens the session. `auto`: number of the automatic reconnection attempt, 0 otherwise. */
  async function connect(next: Attempt = {}, auto = 0) {
    if (!term || !fit) return;
    if (local) return openLocal();
    retry = auto;
    attempt = { ...attempt, ...next };
    if (host.proxyJump) {
      phase = { step: 'error', message: 'This host goes through a bastion (ProxyJump). Connecting through a bastion is not supported yet.' };
      session.status = 'closed';
      return;
    }
    if (!host.user) {
      phase = { step: 'error', message: 'This host has no user: edit the host to add one.' };
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
        ? // Without a typed password, the core takes the one the keyring keeps, or asks for it.
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
          // The tunnels belong to the group of the tab, which outlives a connection.
          group: session.tunnels?.group ?? '',
        },
        onData,
      });
      journalOpened(journal, connected.journalFailed);
      info = connected;
      sessionId = connected.id;
      phase = { step: 'connected' };
      session.status = 'connected';
      // Secrets do not outlive the opening of the session.
      attempt = {};
      if (connected.learnFailed) toasts.warn('The fingerprint could not be remembered', connected.learnFailed);
      if (connected.keepFailed) toasts.warn('The secret could not be kept', `The system keyring refused it: ${connected.keepFailed}`);
      retry = 0;
      // A first connection starts the tunnels; a lost one that comes back finds them in the core.
      ssh.tunnelsStart(session);
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
              ? 'This passphrase does not unlock the key.'
              : problem.refused === 'retained'
                ? 'The remembered passphrase no longer unlocks the key. It has been forgotten.'
                : '',
        };
      } else if (problem.kind === 'needPassword') {
        // Nothing kept, or what was kept has been refused: the series of attempts stops, the tab
        // waits for the user.
        retry = 0;
        attempt.password = undefined;
        phase = { step: 'password', error: problem.refused ? 'The server refused the remembered password. It has been forgotten.' : '' };
      } else if (problem.kind === 'auth' && host.auth === 'password') {
        attempt.password = undefined;
        phase = { step: 'password', error: problem.message };
      } else if (auto) {
        // The server still does not answer: the next attempt is scheduled.
        scheduleRetry(auto + 1, problem.message);
      } else {
        // The reason the system gives, said in plain words when it is known (guidelines, section 9.15).
        phase = { step: 'error', message: explain(problem.message) };
        // Nothing is retried any more: the ports of the tunnels are freed.
        ssh.tunnelsClose(session, true);
      }
    }
  }

  function submitSecret(e: SubmitEvent) {
    e.preventDefault();
    // Without a keyring, nothing is offered: nothing is kept.
    const kept = keep && keyring === true;
    if (phase.step === 'passphrase') void connect({ keyPath: phase.path, passphrase: secret, keep: kept });
    else if (phase.step === 'password') void connect({ password: secret, keep: kept });
  }

  /** "Restart" has just been clicked: the reopened terminal takes the keyboard back, even under the graph. */
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
    // The 16 ANSI colors, in the order of xterm.js: 0 to 7, then their bright versions (guidelines, 2.6).
    const ANSI = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'] as const;
    const bright = (name: string) => `bright${name[0].toUpperCase()}${name.slice(1)}`;
    const theme = () => ({
      background: v('--term-bg'),
      foreground: v('--term-ink'),
      cursor: v('--accent'),
      selectionBackground: v('--sel'),
      // The margin of the search results, without a line on its edge.
      overviewRulerBorder: v('--term-bg'),
      ...Object.fromEntries(ANSI.flatMap((name, i) => [[name, v(`--ansi-${i}`)], [bright(name), v(`--ansi-${i + 8}`)]])),
    });

    const terminal = new Terminal({
      // The search highlights its results with decorations, an API still "proposed".
      allowProposedApi: true,
      overviewRuler: { width: 8 },
      fontFamily: v('--f-mono'),
      fontSize: parseFloat(v('--fs-term')),
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
      theme: theme(),
      // Safety net: a text in a color chosen by the program (256 colors, true colors, or the black
      // and white that serve as backgrounds) is lightened or darkened up to 4.5:1 over the background
      // of its cell.
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
    // The pseudo-terminal follows only once the gesture is over: while a handle is dragged, the
    // shell would redraw its prompt on every frame.
    let resizeTimer: number | undefined;
    terminal.onResize(({ cols, rows }) => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        if (sessionId !== null) void invoke(API.resize, { id: sessionId, cols, rows }).catch(() => {});
      }, 120);
    });

    const observer = new ResizeObserver(() => {
      // Hidden tab: the size is zero, nothing is computed again.
      if (termEl && termEl.offsetParent !== null && termEl.clientWidth > 0) fitAddon.fit();
    });
    if (termEl) observer.observe(termEl);

    // The terminal reads its colors from the tokens: it reads them again when the theme changes.
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
      // Local terminal: a shell that is left (`exit`, Ctrl + D) closes its tab, whatever its exit
      // code, which is only that of the last command. The tab stays, with "Restart", when the shell is
      // stopped by a signal, or stops on an error before anything was typed in it.
      if (local) {
        session.localId = undefined;
        journalEnded();
        if (!signal && (exitStatus === 0 || typed)) return onclose();
        session.status = 'closed';
        const how = signal ? ` (signal: ${signal})` : exitStatus === null ? '' : ` (exit code ${exitStatus})`;
        phase = {
          step: 'closed',
          message: `The shell stopped${how}. Restart it, or close the tab.`,
          brief: `Shell stopped${how}.`,
        };
        return;
      }
      // Only a lost connection opens again on its own, and only if the setting allows it.
      if (ending === 'lost' && prefs.sshReconnect) {
        term?.write('\r\n');
        scheduleRetry(1, reason ?? '');
        return;
      }
      journalEnded(ending === 'lost');
      ssh.tunnelsClose(session, ending === 'lost');
      session.status = 'closed';
      phase =
        ending === 'lost'
          ? { step: 'closed', lost: true, message: reason ? explain(reason) : 'The connection to the server was cut.' }
          : {
              step: 'closed',
              message: reason
                ? `Connection interrupted: ${reason}`
                : exitStatus === null || exitStatus === 0
                  ? 'The server closed the session.'
                  : `Session ended (exit code ${exitStatus}).`,
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
      if (!local) ssh.tunnelsClose(session);
      terminal.dispose();
    };
  });

  $effect(() => {
    // When the tab shows: the size of the terminal may have changed while it was hidden.
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
      <!-- A local terminal: the computer, as everywhere "local" is said (guidelines, section 7). -->
      <span class="what"><Icon name="laptop" size={14} /> <b>Local terminal</b></span>
    {:else}
      {#if session.status === 'connecting'}
        <!-- The connection is opening: the waiting ring takes the place of the dot, as on the tab.
             "connecting…" says it beside it: the ring itself is not read. -->
        <span class="spinner" aria-hidden="true"></span>
      {:else}
        <span class="dot s-{session.status}" role="img" aria-label={STATUS[session.status]} title={STATUS[session.status]}></span>
      {/if}
      <b class="mono">{target}</b>
    {/if}
    <!-- The words of the header give way before the buttons, cut with an ellipsis (9.15): a single
         text, separated by em spaces, so that the ellipsis is drawn. -->
    <span class="meta" title={metaTitle}
      >{#each metaWords as word, i (i)}{#if i}{'\u2003'}{/if}{word.before ?? ''}<span class:mono={word.mono}>{word.text}</span>{/each}</span
    >
    {#if session.journal}
      <span class="journal" title={session.journal.path}>{recording ? 'recording log' : 'log saved'}</span>
    {/if}
    {#if tunnelsLive && session.tunnels}
      {@const group = session.tunnels}
      <!-- "Tunnels · 2" shows and hides the strip; a failure gives it the warning icon. With no
           tunnel yet it reads "Tunnels", so that a first one can be added with the mouse. -->
      <!-- One text node: a span would be a flex item of the button, and lose its spaces. -->
      <button class="btn ghost small toggle tally" type="button" aria-pressed={group.shown} title={tunnelsTitle} onclick={() => (group.shown = !group.shown)}>
        {#if tunnelsFailed.length}<span class="failed"><Icon name="warning" size={14} /></span>{/if}{tunnels.length ? `Tunnels · ${tunnels.length}` : 'Tunnels'}
      </button>
    {/if}
    {#if phase.step === 'connected' && (local || info)}
      <button
        class="btn ghost small toggle"
        type="button"
        aria-pressed={recording}
        title={recording ? 'Stop recording: the file stays.' : 'Record what is shown, and what follows, in a text file.'}
        onclick={() => void toggleJournal()}
      >
        Record log
      </button>
    {/if}
    {#if !local && phase.step === 'connected' && info}
      <button class="btn small" type="button" onclick={() => sessionId !== null && void invoke(API.close, { id: sessionId })}>Disconnect</button>
    {/if}
  </header>
  {/if}

  {#if tunnelsLive && session.tunnels?.shown}
    <TunnelStrip {session} />
  {/if}

  {#if session.searching}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="search" role="search" aria-label="Search the terminal" onkeydown={onBarKey}>
      <label class="filter" class:invalid>
        <Icon name="search" size={14} />
        <input
          bind:this={searchEl}
          type="search"
          placeholder="Search the terminal"
          aria-label="Search the terminal"
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
        <span class="count bad" id="search-count-{session.key}" role="alert">Invalid expression</span>
      {:else}
        <span class="count" id="search-count-{session.key}" role="status">{countText}</span>
      {/if}
      <button class="btn ghost small" type="button" aria-label="Previous result" title="Previous result (Shift + Enter)" disabled={!results?.count} onclick={() => find('previous')}>
        <Icon name="up" size={14} />
      </button>
      <button class="btn ghost small" type="button" aria-label="Next result" title="Next result (Enter)" disabled={!results?.count} onclick={() => find('next')}>
        <Icon name="down" size={14} />
      </button>
      <button class="btn ghost small toggle" type="button" aria-pressed={caseSensitive} title="Match case" aria-label="Match case" onclick={() => (caseSensitive = !caseSensitive)}>Aa</button>
      <button class="btn ghost small toggle mono" type="button" aria-pressed={regex} title="Regular expression" aria-label="Regular expression" onclick={() => (regex = !regex)}>.*</button>
      <button class="btn ghost small close" type="button" aria-label="Close search" title="Close search (Escape)" onclick={closeSearch}>
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
              {#if local}Opening local terminal…{:else}{retry ? 'Reconnecting' : 'Connecting'} to <b class="mono">{target}</b>…{/if}
            </p>
          {:else if phase.step === 'hostKey'}
            <h3><Icon name="warning" /> Unknown server</h3>
            <p>
              <b class="mono">{host.hostname}</b> is not in <code>known_hosts</code>. Check that this {phase.algorithm}
              fingerprint is the server’s before you trust it:
            </p>
            <p class="print mono">{phase.fingerprint}</p>
            <div class="row">
              <button class="btn primary" type="button" onclick={() => void connect({ accept: phase.step === 'hostKey' ? phase.fingerprint : undefined, remember: true })}>
                Trust and remember
              </button>
              <button class="btn" type="button" onclick={() => void connect({ accept: phase.step === 'hostKey' ? phase.fingerprint : undefined, remember: false })}>
                Trust this time
              </button>
              <button class="btn ghost" type="button" onclick={onclose}>Cancel</button>
            </div>
          {:else if phase.step === 'hostKeyChanged'}
            <h3><Icon name="warning" /> Server fingerprint changed</h3>
            <p>
              The fingerprint presented by <b class="mono">{host.hostname}</b> no longer matches the one saved on line
              {phase.line} of <code>known_hosts</code>. The connection is blocked: the server may have been reinstalled, or
              someone may be intercepting the connection.
            </p>
            <p class="print mono">{phase.fingerprint}</p>
            <p class="muted">If the change is legitimate, remove the old line from <code>known_hosts</code>, then reconnect.</p>
            <div class="row">
              <button class="btn" type="button" onclick={reconnect}>Try again</button>
              <button class="btn ghost" type="button" onclick={onclose}>Close tab</button>
            </div>
          {:else if phase.step === 'passphrase' || phase.step === 'password'}
            <form onsubmit={submitSecret}>
              <h3>{phase.step === 'passphrase' ? 'Key passphrase' : `Password for ${host.user}`}</h3>
              <p class="muted">
                {#if phase.step === 'passphrase'}
                  The key <span class="mono">{phase.path}</span> is protected.
                {:else}
                  For <span class="mono">{target}</span>.
                {/if}
              </p>
              <!-- Secret refused: the field turns to its error state, its message under it (guidelines, section 9.3). -->
              <div class="secret">
                <input
                  class="field"
                  class:invalid={!!phase.error}
                  type="password"
                  autocomplete="off"
                  bind:this={secretEl}
                  aria-label={phase.step === 'passphrase' ? 'Passphrase' : 'Password'}
                  aria-invalid={phase.error ? 'true' : undefined}
                  aria-describedby={phase.error ? `secret-error-${session.key}` : undefined}
                  bind:value={secret}
                />
                {#if phase.error}<p class="err small" id="secret-error-{session.key}" role="alert">{phase.error}</p>{/if}
              </div>
              <!-- Keeping the secret: offered when the system keyring responds; otherwise that is said. -->
              {#if keyring}
                <label class="keep">
                  <input type="checkbox" bind:checked={keep} />
                  <span>Remember in the system keyring</span>
                </label>
              {:else if keyring === false}
                <p class="muted">{NO_KEYRING[phase.step]}</p>
              {/if}
              <div class="row">
                <button class="btn primary" type="submit" disabled={!secret}>Connect</button>
                <button class="btn ghost" type="button" onclick={onclose}>Cancel</button>
              </div>
            </form>
          {:else if phase.step === 'lost'}
            <h3><Icon name="warning" /> Connection lost</h3>
            <!-- The countdown changes every second: it is not read, only the announcement is. -->
            <p aria-hidden="true">{waitText}</p>
            <p class="spoken" role="status">{announce}</p>
            <!-- The line of the reason keeps its place: the buttons do not move from one attempt to the
                 next. Empty, it holds a non-breaking space, which keeps its height. -->
            <p class="muted reason">{phase.reason || '\u00a0'}</p>
            <!-- With tunnels, a line of its own says so, from the first attempt on (guidelines, 9.15). -->
            {#if ssh.tunnelsPaused(session)}<p class="muted">{ssh.tunnelsPaused(session)}</p>{/if}
            <div class="row">
              <button class="btn primary" type="button" bind:this={retryButton} onclick={retryNow}><Icon name="refresh" size={14} /> Reconnect now</button>
              <button class="btn ghost" type="button" onclick={stopRetrying}>Stop</button>
            </div>
          {:else}
            <h3>
              {#if local}Shell stopped{:else}{phase.step === 'error'
                  ? 'Connection failed'
                  : phase.lost
                    ? 'Connection lost'
                    : 'Session ended'}{/if}
            </h3>
            <!-- A failure is announced at once: the focus goes to the button, and the reason would not be read. -->
            <p class:err={phase.step === 'error'} role={phase.step === 'error' ? 'alert' : undefined}>{phase.message}</p>
            <div class="row">
              <!-- A local terminal is never "reconnected": it is restarted. -->
              <button class="btn primary" type="button" bind:this={endButton} onclick={reconnect}><Icon name="refresh" size={14} /> {local ? 'Restart' : 'Reconnect'}</button>
              <button class="btn ghost" type="button" onclick={onclose}>Close tab</button>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  {#if strip && phase.step !== 'connected'}
    <!-- Under the graph, neither veil nor card: a strip one line high, which leaves the last lines
         of output readable, those that say why the shell stopped (guidelines, section 9.20). -->
    <div class="strip">
      {#if phase.step === 'connecting'}
        <span class="spinner" aria-hidden="true"></span>
        <span class="said" role="status">Opening local terminal…</span>
      {:else if phase.step === 'closed' || phase.step === 'error'}
        {@const said = phase.step === 'closed' ? (phase.brief ?? phase.message) : phase.message}
        <span class="said" class:bad={phase.step === 'error'} role="alert" title={said}>{said}</span>
        <button class="btn primary small" type="button" bind:this={endButton} onclick={reconnect}>Restart</button>
        <button class="btn ghost small" type="button" onclick={onclose}>Close tab</button>
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
  /* Search bar: a strip under the header, which pushes the terminal down without hiding its text. */
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
  /* Wide enough for "Invalid expression": the buttons do not move when the text changes. */
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
  /* Toggle: a ghost button, pressed in --accent-soft with an accent border (guidelines, section 9.1). */
  .toggle[aria-pressed='true'],
  .toggle[aria-pressed='true']:hover {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .close {
    margin-left: auto;
  }
  /* The count of the tunnels toggle, in tabular figures. */
  .tally {
    font-variant-numeric: tabular-nums;
  }
  .failed {
    display: grid;
    color: var(--bad);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    /* Fixed height: that of a header with its "Disconnect" button. The terminal does not jump from
       one tab to the next. */
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
  /* The header speaks in --muted; its buttons keep the text of a button (9.1). */
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
  /* Terminal of a repository: the state of the shell fits in one line, under the terminal. */
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
    /* In a low terminal (under the graph), the panel scrolls rather than overflowing; its automatic
       margins center it without cutting its top when it is taller. */
    display: flex;
    padding: 24px;
    overflow: auto;
    background: color-mix(in srgb, var(--term-bg) 82%, transparent);
    backdrop-filter: blur(1.5px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .card {
    margin: auto;
    /* 560 px: the three buttons of the fingerprint fit on one line. */
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
  /* The checkbox and its text are the target: no wider than that text. */
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
  /* The field of a secret and, under it, the message of its refusal. */
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
  /* Off screen, but read: the announcement of a reconnection attempt. */
  .what {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
