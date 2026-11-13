<script lang="ts">
  import { slide } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import { discardFiles } from './actions';
  import FileList from './FileList.svelte';
  import type { Repo } from './repo.svelte';
  import type { FileChange } from './types';
  import { motion } from '../ui/motion';

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
  const subjectLength = $derived(message.split('\n')[0].length);
  const headPushed = $derived(repo.hasRemote && !!repo.head && !repo.head.localOnly);
  const plural = (n: number) => (n === 1 ? '' : 's');

  // A file in both lists is only partly staged.
  const partial = $derived(new Set(status.staged.filter((f) => unstaged.some((u) => u.path === f.path)).map((f) => f.path)));

  // What is missing to commit: said on screen, rather than left to guess in front of a grayed-out button.
  const blocker = $derived.by(() => {
    if (busy) return '';
    if (conflicts.length) return 'Resolve the conflicts first.';
    if (!status.staged.length && !repo.amend) return 'Nothing is staged: stage a file or some lines.';
    if (!message.trim()) return 'Write a commit message.';
    return '';
  });
  // Ctrl + Enter while committing is not possible: the reason is brought forward, until it changes.
  let nudged = $state(false);
  $effect(() => {
    void blocker;
    nudged = false;
  });

  const active = $derived(repo.openFile?.key ?? null);
  const activeIn = (prefix: string) => (active?.startsWith(prefix) ? active.slice(2) : null);

  // During a rebase, git swaps the roles: “ours” is the base, “theirs” the commit being replayed.
  const sides = $derived(
    status.operation === 'rebase'
      ? { ours: 'Keep the base version', theirs: 'Keep your change' }
      : { ours: 'Keep your version', theirs: 'Keep the incoming version' },
  );

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !(e.ctrlKey || e.metaKey)) return;
    e.preventDefault();
    if (canCommit) void repo.commit();
    else if (blocker) nudged = true;
  }
</script>

<div class="work">
  {#if conflicts.length}
    <section class="conflicts" transition:slide={{ duration: motion.med }}>
      <header>
        <span class="label bad"><Icon name="warning" size={14} /> In conflict · {conflicts.length}</span>
      </header>
      <div class="list">
        <FileList
          files={conflicts}
          active={repo.conflictFile?.path ?? null}
          disabled={busy}
          onopen={(f) => repo.openConflict(f)}
          actions={[
            { label: sides.ours, icon: 'laptop', run: (f: FileChange) => void repo.resolve(f, 'ours') },
            { label: sides.theirs, icon: 'cloud', run: (f: FileChange) => void repo.resolve(f, 'theirs') },
            { label: 'Mark as resolved (after fixing it in your editor)', icon: 'check', run: (f: FileChange) => void repo.stage([f]) },
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
          {partial}
          active={activeIn('u:')}
          disabled={busy}
          onopen={(f) => repo.openWorkDiff(f, false)}
          actions={[
            { label: 'Discard changes…', icon: 'undo', danger: true, run: (f: FileChange) => void discardFiles(repo, [f]) },
            { label: 'Stage', icon: 'plus', run: (f: FileChange) => void repo.stage([f]) },
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
          {partial}
          active={activeIn('s:')}
          disabled={busy}
          onopen={(f) => repo.openWorkDiff(f, true)}
          actions={[{ label: 'Unstage', icon: 'minus', run: (f: FileChange) => void repo.unstage([f]) }]}
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
        This commit is already on the remote: after amending it, you will have to force push.
      </p>
    {/if}
    <label class="label" for="commit-message">Commit message</label>
    <textarea
      id="commit-message"
      class="field"
      rows="4"
      placeholder="Summary on the first line, details after"
      bind:value={repo.message}
      onkeydown={onKey}
      disabled={busy}
    ></textarea>
    {#if blocker}
      <!-- Ctrl + Enter refused: an icon is added to the color, and the text, inserted again, is announced again. -->
      <p id="commit-blocker" class="why small" class:nudged aria-live="polite">
        {#key nudged}
          {#if nudged}<Icon name="warning" size={14} />{/if}
          <span>{blocker}</span>
        {/key}
      </p>
    {/if}
    <div class="send">
      <small class="muted" class:long={subjectLength > 72} title="Length of the first line: past 72 characters, many tools cut it.">
        {subjectLength} / 72
      </small>
      <button
        class="btn primary"
        type="button"
        disabled={!canCommit}
        aria-describedby={blocker ? 'commit-blocker' : undefined}
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
    </div>
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
  .warn {
    padding: 6px 8px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
  }
  .amend {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  textarea {
    resize: vertical;
    min-height: 64px;
    max-height: 220px;
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  .why {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--muted);
    transition: color var(--t-fast) var(--ease);
  }
  .why.nudged {
    color: var(--warn);
  }
  .send {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .send small {
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
  }
  .send small.long {
    color: var(--warn);
  }
</style>
