<script lang="ts">
  import { tick } from 'svelte';
  import { veil } from '../ui/veil';
  import { flip } from 'svelte/animate';
  import Icon from '../ui/Icon.svelte';
  import Select from '../ui/Select.svelte';
  import type { Repo } from './repo.svelte';
  import type { TodoAction, TodoItem } from './types';
  import { motion } from '../ui/motion';

  let { repo }: { repo: Repo } = $props();

  interface Item {
    sha: string;
    subject: string;
    action: TodoAction;
    message: string;
  }

  const ACTIONS: { value: TodoAction; label: string; hint: string }[] = [
    { value: 'pick', label: 'Pick', hint: 'The commit is replayed as it is' },
    { value: 'reword', label: 'Reword', hint: 'The commit is replayed with a new message' },
    { value: 'squash', label: 'Squash', hint: 'Melded into the commit above; the two messages are combined' },
    { value: 'fixup', label: 'Fixup', hint: 'Melded into the commit above, whose message alone is kept' },
    { value: 'drop', label: 'Drop', hint: 'The commit is removed from the branch' },
  ];

  let element = $state<HTMLDialogElement>();
  let items = $state<Item[]>([]);
  let dragged = $state(-1);

  const plan = $derived(repo.rebasePlan);
  const firstKept = $derived(items.find((i) => i.action !== 'drop'));
  const problem = $derived.by(() => {
    if (!firstKept) return 'Every commit would be dropped: use a reset instead.';
    if (firstKept.action === 'squash' || firstKept.action === 'fixup') {
      return 'The first commit kept cannot be squashed or fixed up: there is no commit above it.';
    }
    if (items.some((i) => i.action === 'reword' && !i.message.trim())) return 'A commit to reword has an empty message.';
    return '';
  });
  const changed = $derived(
    !!plan && (items.some((i) => i.action !== 'pick') || items.some((item, n) => item.sha !== plan.commits[n]?.sha)),
  );

  $effect(() => {
    if (!element) return;
    if (plan && !element.open) {
      items = plan.commits.map((c) => ({ sha: c.sha, subject: c.subject, action: 'pick', message: c.message }));
      element.showModal();
      // The rows appear after the opening: the focus then goes to the first list (guidelines, 9.10).
      void tick().then(() => element?.querySelector<HTMLElement>('select')?.focus());
    } else if (!plan && element.open) {
      element.close();
    }
  });

  function move(from: number, to: number) {
    if (to < 0 || to >= items.length || from === to) return;
    const next = [...items];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    items = next;
  }

  const cancel = () => (repo.rebasePlan = null);

  async function run() {
    if (!plan || problem) return;
    const todo: TodoItem[] = items.map((i) => ({
      action: i.action,
      sha: i.sha,
      message: i.action === 'reword' ? i.message : undefined,
    }));
    const base = plan.base;
    repo.rebasePlan = null;
    await repo.rebaseInteractive(base, todo);
  }
</script>

<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    cancel();
  }}
  use:veil={cancel}
>
  {#if plan}
    <!-- A form: Enter starts the rebase (guidelines, 9.10), except in a message to reword. -->
    <form
      class="box"
      onsubmit={(e) => {
        e.preventDefault();
        if (!problem && changed) run();
      }}
    >
      <header>
        <h2>Interactive rebase</h2>
        <p>
          {items.length} commit{items.length === 1 ? '' : 's'} of <b class="mono">{repo.branch ?? 'HEAD'}</b>, from the oldest (at the top) to the newest (at the bottom).
          Drag a row or use the arrows to change the order.
        </p>
        {#if plan.pushed}
          <p class="warning">
            <Icon name="warning" />
            <span>Some of these commits are already on the remote: after the rebase, you will have to force push.</span>
          </p>
        {/if}
      </header>

      <ol>
        {#each items as item, n (item.sha)}
          <li
            class="a-{item.action}"
            class:dragging={dragged === n}
            animate:flip={{ duration: motion.med }}
            draggable="true"
            ondragstart={(e) => {
              dragged = n;
              e.dataTransfer?.setData('text/plain', String(n));
            }}
            ondragover={(e) => {
              e.preventDefault();
              if (dragged >= 0 && dragged !== n) {
                move(dragged, n);
                dragged = n;
              }
            }}
            ondragend={() => (dragged = -1)}
          >
            <span class="grip" title="Drag to move"><Icon name="grip" /></span>
            <div class="main">
              <div class="line">
                <span class="action">
                  <Select bind:value={item.action} title={ACTIONS.find((a) => a.value === item.action)?.hint} aria-label="Action for {item.subject}">
                    {#each ACTIONS as action (action.value)}
                      <option value={action.value}>{action.label}</option>
                    {/each}
                  </Select>
                </span>
                <span class="sha mono">{item.sha.slice(0, 7)}</span>
                <span class="subject">{item.subject}</span>
              </div>
              {#if item.action === 'reword'}
                <textarea class="field mono" rows="3" aria-label="New message for {item.subject}" bind:value={item.message}></textarea>
              {/if}
            </div>
            <span class="moves">
              <button type="button" aria-label="Move up" title="Move up" disabled={n === 0} onclick={() => move(n, n - 1)}><Icon name="up" size={14} /></button>
              <button type="button" aria-label="Move down" title="Move down" disabled={n === items.length - 1} onclick={() => move(n, n + 1)}><Icon name="down" size={14} /></button>
            </span>
          </li>
        {/each}
      </ol>

      <footer>
        <!-- The reason for a disabled “Start rebase”, or else the backup (guidelines, 9.1). -->
        <small class:bad={!!problem}>
          {problem ||
            (changed
              ? 'The current position of the branch is saved in refs/ramure/backup before the rebase.'
              : 'Change the order or the action of a commit to start the rebase.')}
        </small>
        <button class="btn" type="button" onclick={cancel}>Cancel</button>
        <button class="btn primary" type="submit" disabled={!!problem || !changed}>Start rebase</button>
      </footer>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(760px, calc(100vw - 48px));
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--ink);
    box-shadow: var(--shadow-pop);
  }
  dialog[open] {
    display: flex;
    animation: pop-in var(--t-med) var(--ease);
  }
  dialog::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(2px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .box {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: 100%;
    min-height: 0;
  }
  header {
    padding: 16px 20px 8px;
    display: grid;
    gap: 8px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    color: var(--muted);
  }
  .warning {
    display: flex;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .warning span {
    color: var(--ink);
  }
  /* Plain rows, separated by a --line line (--line-soft does not show on --raised): a row framed
     in a box would be a card in a card (guidelines, sections 6 and 9.6). */
  ol {
    list-style: none;
    margin: 0;
    padding: 4px 20px;
    overflow-y: auto;
    display: grid;
    align-content: start;
  }
  li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 6px 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  li + li {
    border-top: 1px solid var(--line);
  }
  /* The dragged row keeps its place at half opacity; the system draws it under the pointer. */
  li.dragging {
    opacity: 0.5;
  }
  /* A dropped commit stays readable: struck through, in --muted (--faint carries nothing needed, 2.3). */
  li.a-drop .subject {
    text-decoration: line-through;
    color: var(--muted);
  }
  /* A commit melded into the previous one is indented under it, by the grip and its gap: its grip
     falls under the list of the commit that receives it (guidelines, 9.6). */
  li.a-squash,
  li.a-fixup {
    margin-left: 24px;
  }
  .grip {
    display: grid;
    padding-top: 6px;
    color: var(--faint);
    cursor: grab;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 6px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  /* A fixed width: the SHAs and the subjects of the rows line up. */
  .action {
    width: 240px;
    flex: none;
  }
  .sha {
    color: var(--muted);
    font-size: var(--fs-code);
  }
  .subject {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  textarea {
    resize: vertical;
    font-size: var(--fs-code);
  }
  .moves {
    display: flex;
    gap: 2px;
    padding-top: 3px;
  }
  .moves button {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .moves button:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
  }
  .moves button:active:not(:disabled) {
    transform: scale(0.97);
  }
  .moves button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 20px 16px;
  }
  footer small {
    flex: 1;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  footer small.bad {
    color: var(--bad);
  }
</style>
