<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import FileList from './FileList.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo, commit }: { repo: Repo; commit: CommitRow } = $props();

  const fullDateFmt = new Intl.DateTimeFormat('en-US', { dateStyle: 'full', timeStyle: 'short' });
  /** The full date, its time kept on one line with AM or PM. */
  const fullDate = (ms: number) => fullDateFmt.format(ms).replace(/ (AM|PM)$/, '\u00a0$1');
  const detail = $derived(repo.detail);
  const body = $derived(detail ? detail.message.split('\n').slice(1).join('\n').trim() : '');

  async function copySha() {
    try {
      await navigator.clipboard.writeText(commit.sha);
      toasts.ok('SHA copied');
    } catch {
      /* clipboard refused: the SHA stays selectable in the panel */
    }
  }
</script>

<header class="head">
  <span class="label">Commit</span>
  <!-- The working changes stay one click away, even while another commit is shown. -->
  {#if repo.wipCount > 0}
    <button class="btn small" type="button" onclick={() => repo.showWip()}>
      <Icon name="pencil" size={14} />
      {repo.wipText}
    </button>
  {/if}
</header>
<p class="title">{commit.subject}</p>
{#if body}<p class="message">{body}</p>{/if}
<dl>
  <dt>SHA</dt>
  <dd class="sha">
    <span class="mono sha-full">{commit.sha}</span>
    <button class="copy" type="button" title="Copy SHA" aria-label="Copy SHA" onclick={copySha}><Icon name="copy" size={14} /></button>
  </dd>
  <dt>Author</dt>
  <dd>{commit.author}</dd>
  <dt>Date</dt>
  <dd>{fullDate(commit.time * 1000)}</dd>
  <dt>Parent{commit.parents.length === 1 ? '' : 's'}</dt>
  <dd>{#if commit.parents.length}<span class="mono parents">{commit.parents.map((p) => p.slice(0, 7)).join(', ')}</span>{:else}none (first commit){/if}</dd>
  {#if repo.hasRemote}
    <dt>Location</dt>
    <dd>{commit.localOnly ? 'Local only, not pushed yet' : 'On the remote'}</dd>
  {/if}
  {#if commit.refs.length}
    <dt>Refs</dt>
    <dd>{commit.refs.map((r) => r.name).join(', ')}</dd>
  {/if}
</dl>

{#if repo.detailError}
  <p class="err" role="alert">{repo.detailError}</p>
{:else if detail}
  <span class="label">
    Changed files · {detail.files.length}{commit.parents.length > 1 ? ' (against the first parent)' : ''}
  </span>
  {#if detail.files.length}
    <div class="files">
      <FileList
        files={detail.files}
        active={repo.openFile?.key.startsWith('c:') ? repo.openFile.key.slice(2) : null}
        onopen={(f) => repo.openCommitDiff(f)}
      />
    </div>
  {:else}
    <p class="muted">This commit changes no files.</p>
  {/if}
{:else}
  <div class="skeleton" aria-label="Loading files">
    <span></span><span></span><span></span>
  </div>
{/if}

<style>
  .head {
    /* The whole width of the card: the button goes to the right edge. The height is that of a
       small button, so that the title of the commit does not move when the button appears. */
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 24px;
  }
  p {
    margin: 0;
  }
  .title {
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
    overflow-wrap: anywhere;
  }
  .message {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--muted);
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0;
    width: 100%;
  }
  dt {
    color: var(--muted);
    font-size: var(--fs-label);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .sha {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .sha-full {
    font-size: var(--fs-code);
    user-select: all;
  }
  .copy {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-top: -4px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .copy:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .copy:active {
    transform: scale(0.97);
  }
  .files {
    width: calc(100% + 12px);
    margin: -4px -6px 0;
  }
  .muted {
    color: var(--muted);
  }
  .err {
    color: var(--bad);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  /* Ghost lines while git lists the files. */
  .skeleton {
    display: grid;
    gap: 8px;
    width: 100%;
  }
  .skeleton span {
    height: 14px;
    border-radius: var(--radius-xs);
    background: linear-gradient(90deg, var(--hover) 25%, var(--line-soft) 50%, var(--hover) 75%);
    background-size: 200% 100%;
    animation: shimmer var(--t-wait) linear infinite;
  }
  .skeleton span:nth-child(2) {
    width: 70%;
  }
  .skeleton span:nth-child(3) {
    width: 85%;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  .parents {
    font-size: var(--fs-code);
  }
</style>
