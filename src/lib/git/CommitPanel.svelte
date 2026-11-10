<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import FileList from './FileList.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo, commit }: { repo: Repo; commit: CommitRow } = $props();

  const fullDateFmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' });
  const detail = $derived(repo.detail);
  const body = $derived(detail ? detail.message.split('\n').slice(1).join('\n').trim() : '');

  async function copySha() {
    try {
      await navigator.clipboard.writeText(commit.sha);
      toasts.ok('SHA copié');
    } catch {
      /* presse-papiers refusé : le SHA reste sélectionnable dans le panneau */
    }
  }
</script>

<span class="label">Commit</span>
<p class="title">{commit.subject}</p>
{#if body}<p class="message">{body}</p>{/if}
<dl>
  <dt>SHA</dt>
  <dd class="sha">
    <span class="mono sha-full">{commit.sha}</span>
    <button class="copy" type="button" title="Copier le SHA" aria-label="Copier le SHA" onclick={copySha}><Icon name="copy" size={14} /></button>
  </dd>
  <dt>Auteur</dt>
  <dd>{commit.author}</dd>
  <dt>Date</dt>
  <dd>{fullDateFmt.format(commit.time * 1000)}</dd>
  <dt>Parent{commit.parents.length > 1 ? 's' : ''}</dt>
  <dd class="mono">{commit.parents.length ? commit.parents.map((p) => p.slice(0, 7)).join(', ') : 'aucun (premier commit)'}</dd>
  {#if repo.hasRemote}
    <dt>Emplacement</dt>
    <dd>{commit.localOnly ? 'Local uniquement, pas encore poussé' : 'Présent sur le dépôt distant'}</dd>
  {/if}
  {#if commit.refs.length}
    <dt>Références</dt>
    <dd>{commit.refs.map((r) => r.name).join(', ')}</dd>
  {/if}
</dl>

{#if repo.detailError}
  <p class="err">{repo.detailError}</p>
{:else if detail}
  <span class="label">
    Fichiers modifiés · {detail.files.length}{commit.parents.length > 1 ? ' (par rapport au premier parent)' : ''}
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
    <p class="muted">Ce commit ne modifie aucun fichier.</p>
  {/if}
{:else}
  <div class="skeleton" aria-label="Chargement des fichiers">
    <span></span><span></span><span></span>
  </div>
{/if}

<style>
  p {
    margin: 0;
  }
  .title {
    font-size: 14.5px;
    font-weight: 650;
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
    font-size: 12px;
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
    font-size: 11.5px;
    user-select: all;
  }
  .copy {
    flex: none;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    margin-top: -3px;
    border: 0;
    border-radius: 5px;
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
    transform: scale(0.9);
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
  /* Lignes fantômes pendant que git liste les fichiers. */
  .skeleton {
    display: grid;
    gap: 8px;
    width: 100%;
  }
  .skeleton span {
    height: 14px;
    border-radius: 4px;
    background: linear-gradient(90deg, var(--hover) 25%, var(--line-soft) 50%, var(--hover) 75%);
    background-size: 200% 100%;
    animation: shimmer 1.2s linear infinite;
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
</style>
