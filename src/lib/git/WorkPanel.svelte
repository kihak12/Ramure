<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { slide } from 'svelte/transition';
  import { focusBack } from '../ui/focus';
  import Icon from '../ui/Icon.svelte';
  import { discardFiles } from './actions';
  import FileList from './FileList.svelte';
  import MessageField from './MessageField.svelte';
  import type { Repo } from './repo.svelte';
  import type { FileChange } from './types';
  import { motion } from '../ui/motion';
  import { announcer } from '../ui/announce.svelte';

  let { repo }: { repo: Repo } = $props();

  const status = $derived(repo.status!);
  const busy = $derived(repo.locked);
  const conflicts = $derived(repo.conflicts);
  const unstaged = $derived(status.unstaged.filter((f) => f.kind !== 'conflict'));
  const message = $derived(repo.message);
  // An amend may change only the message: nothing staged is needed then.
  const canCommit = $derived(
    (status.staged.length > 0 || repo.amend) && message.trim().length > 0 && !busy && conflicts.length === 0,
  );
  const headPushed = $derived(repo.hasRemote && !!repo.head && !repo.head.localOnly);
  const plural = (n: number) => (n === 1 ? '' : 's');

  // A file in both lists is only partly staged.
  const partial = $derived(new Set(status.staged.filter((f) => unstaged.some((u) => u.path === f.path)).map((f) => f.path)));

  // Lines picked in the diff and not staged yet (guidelines, 9.14).
  const picked = $derived(repo.picked.length);
  const lines = (n: number) => `${n.toLocaleString('en-US')} line${plural(n)}`;
  // Something would be committed: picked lines not staged would be left out of it.
  const leftOut = $derived(
    !busy && !conflicts.length && picked && (status.staged.length > 0 || repo.amend)
      ? `${lines(picked)} picked ${picked === 1 ? 'is' : 'are'} not staged: ${picked === 1 ? 'it' : 'they'} will not be in the commit.`
      : '',
  );

  // What is missing to commit: said on screen, rather than left to guess in front of a grayed-out button.
  // Lines picked come before "Nothing is staged": staging them is the step to do.
  const blocker = $derived.by(() => {
    if (busy) return '';
    if (conflicts.length) return 'Resolve the conflicts first.';
    if (!status.staged.length && !repo.amend) return picked ? `${lines(picked)} picked, not staged yet.` : 'Nothing is staged: stage a file or some lines.';
    if (!message.trim()) return 'Write a commit message.';
    return '';
  });
  // The hint is that of the lines picked: the button that stages them goes beside it, and nowhere else.
  const pickedHint = $derived(!busy && !conflicts.length && !!picked && !status.staged.length && !repo.amend);
  const describedBy = $derived([leftOut && 'commit-left-out', blocker && 'commit-blocker'].filter(Boolean).join(' ') || undefined);

  /** "Stage N lines" of the commit box: the lines picked in the diff, as its own button does; the
   *  button gone, the keyboard goes to the message, the next step (9.14). */
  async function stagePicked() {
    await repo.applyLines(repo.picked);
    await tick();
    focusBack(document.getElementById('commit-message'));
  }
  // Ctrl + Enter while committing is not possible: the reason is brought forward, until it changes.
  let nudged = $state(false);
  /** Each refused Ctrl + Enter, said each time (11). */
  let nudges = $state(0);
  $effect(() => {
    void blocker;
    nudged = false;
  });

  const active = $derived(repo.openFile?.key ?? null);
  const activeIn = (prefix: string) => (active?.startsWith(prefix) ? active.slice(2) : null);

  // During a rebase, git swaps the roles: “ours” is the base, “theirs” the commit being replayed.
  const sides = $derived(
    status.operation === 'rebase'
      ? { ours: 'Keep upstream', theirs: 'Keep your commit' }
      : { ours: 'Keep your version', theirs: 'Keep the incoming version' },
  );

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !(e.ctrlKey || e.metaKey)) return;
    e.preventDefault();
    if (canCommit) void repo.commit();
    else if (blocker) {
      nudged = true;
      nudges++;
    }
  }

  /** The file open in the conflict editor, wherever it is: the buttons of its row wait (9.26, "One editor"). */
  const inEditor = (f: FileChange) =>
    f.path === (repo.conflictFile?.path ?? repo.editorWindow?.file) ? 'This file is open in the conflict editor.' : null;

  $effect(() => {
    if (leftOut) announcer.say(leftOut);
  });
  $effect(() => {
    void nudges;
    if (untrack(() => nudged && blocker)) announcer.say(untrack(() => blocker));
  });
</script>

<!-- data-file-lists: where the keyboard goes when a list of files empties (guidelines, 9.6). -->
<div class="work" data-file-lists>
  {#if conflicts.length}
    <section class="conflicts" transition:slide={{ duration: motion.med }}>
      <header>
        <span class="label bad"><Icon name="warning" size={14} /> In conflict · {conflicts.length}</span>
      </header>
      <div class="list">
        <FileList
          files={conflicts}
          order={1}
          active={repo.conflictFile?.path ?? null}
          disabled={busy}
          onopen={(f) => repo.openConflict(f)}
          actions={[
            { label: sides.ours, icon: 'laptop', unavailable: inEditor, run: (f: FileChange) => void repo.resolve(f, 'ours') },
            { label: sides.theirs, icon: 'cloud', unavailable: inEditor, run: (f: FileChange) => void repo.resolve(f, 'theirs') },
            { label: 'Mark as resolved (after fixing it in your editor)', icon: 'check', unavailable: inEditor, run: (f: FileChange) => void repo.stage([f]) },
          ]}
        />
      </div>
      <p class="muted small">Click a file to open the conflict editor, or keep one side directly with the buttons of its row.</p>
    </section>
  {/if}

  <section>
    <header>
      <span class="label">Unstaged · {unstaged.length}</span>
      <button class="btn small" type="button" disabled={busy || !unstaged.length} onclick={() => void repo.stage(unstaged)}>
        Stage all
      </button>
    </header>
    {#if unstaged.length}
      <div class="list">
        <FileList
          files={unstaged}
          order={2}
          {partial}
          active={activeIn('u:')}
          disabled={busy}
          onopen={(f) => repo.openWorkDiff(f, false)}
          actions={[
            { label: 'Discard changes…', icon: 'undo', danger: true, key: 'Delete', run: (f: FileChange) => discardFiles(repo, [f]) },
            { label: 'Stage', icon: 'plus', key: 'Space', run: (f: FileChange) => repo.stage([f]) },
          ]}
        />
      </div>
    {:else}
      <p class="muted">Nothing to stage.</p>
    {/if}
  </section>

  <section>
    <header>
      <span class="label">Staged · {status.staged.length}</span>
      <button class="btn small" type="button" disabled={busy || !status.staged.length} onclick={() => void repo.unstage(status.staged)}>
        Unstage all
      </button>
    </header>
    {#if status.staged.length}
      <div class="list">
        <FileList
          files={status.staged}
          order={3}
          {partial}
          active={activeIn('s:')}
          disabled={busy}
          onopen={(f) => repo.openWorkDiff(f, true)}
          actions={[{ label: 'Unstage', icon: 'minus', key: 'Space', run: (f: FileChange) => repo.unstage([f]) }]}
        />
      </div>
    {:else}
      <p class="muted">Stage the files to include in the commit. Open a modified file to stage only some of its lines.</p>
    {/if}
  </section>

  <section class="compose">
    <label class="amend">
      <input type="checkbox" checked={repo.amend} disabled={busy || !repo.head} onchange={(e) => void repo.toggleAmend(e.currentTarget.checked)} />
      <span>Amend last commit</span>
    </label>
    {#if repo.amend && headPushed}
      <p class="warn small" transition:slide={{ duration: motion.med }}>
        <Icon name="warning" size={14} />
        <span>This commit is already on the remote: after amending it, you will have to force push.</span>
      </p>
    {/if}
    <label class="label" for="commit-message">Commit message</label>
    <MessageField id="commit-message" bind:value={repo.message} onkeydown={onKey} disabled={busy}>
      {#snippet notes()}
        {#snippet stage()}
          <button class="btn ghost small" type="button" onclick={() => void stagePicked()}>Stage {lines(picked)}</button>
        {/snippet}
        {#if leftOut}
          <!-- "Commit" would leave out the lines picked: a warning, with what stages them (9.14). -->
          <div class="hint">
            <p id="commit-left-out" class="why small warned"><Icon name="warning" size={14} /><span>{leftOut}</span></p>
            {@render stage()}
          </div>
        {/if}
        {#if blocker}
          <div class="hint">
            <!-- Ctrl + Enter refused: an icon is added to the color, and the text is said again at each press. -->
            <p id="commit-blocker" class="why small" class:nudged>
              {#key nudged}
                {#if nudged}<Icon name="warning" size={14} />{/if}
                <span>{blocker}</span>
              {/key}
            </p>
            {#if pickedHint}{@render stage()}{/if}
          </div>
        {/if}
      {/snippet}
      {#snippet actions()}
      <button
        class="btn primary"
        type="button"
        disabled={!canCommit}
        aria-describedby={describedBy}
        onclick={() => void repo.commit()}
        title={blocker || 'Ctrl + Enter'}
      >
        {#if repo.amend}
          Amend last commit
        {:else if status.staged.length}
          Commit {status.staged.length} file{plural(status.staged.length)}
        {:else}
          Commit
        {/if}
      </button>
      {/snippet}
    </MessageField>
  </section>
</div>

<style>
  .work {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  section {
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 56px;
  }
  section.conflicts,
  section.compose {
    flex: 0 0 auto;
  }
  .conflicts {
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--bad-soft);
    max-height: 40%;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .label.bad {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--bad);
  }
  .list {
    overflow: auto;
    min-height: 0;
    margin: 0 -6px;
  }
  .muted {
    margin: 0;
    color: var(--muted);
  }
  .small {
    margin: 0;
    font-size: var(--fs-small);
  }
  /* A tinted box, with the warning icon, as the box of a reworded commit (guidelines, 5 and 9.25). */
  .warn {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    padding: 6px 8px;
    border-radius: var(--radius);
    background: var(--warn-soft);
  }
  .warn :global(.icon) {
    flex: none;
    margin-top: 1px;
    color: var(--warn);
  }
  .amend {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  /* A sentence that wraps keeps its icon on its first line (4.1). */
  .why {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    color: var(--muted);
    transition: color var(--t-fast) var(--ease);
  }
  .why :global(.icon) {
    flex: none;
    margin-top: 1px;
  }
  .why.nudged,
  .why.warned {
    color: var(--warn);
  }
  /* A sentence of the commit box and the button that does the step, side by side; under the
     sentence when the panel is narrow. */
  .hint {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
  }
</style>
