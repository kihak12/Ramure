<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/logo/ramure-logo-sombre.svg">
    <img src="docs/logo/ramure-logo.svg" alt="Ramure" height="72">
  </picture>
</p>

# Ramure

A graphical Git client and an SSH/SFTP client in one window, for Windows and Ubuntu.
Free software (MIT), local, no account.

**Status: milestones J1 and J2 are done, J3 has started.** The J0 prototype validated the graph
and the SSH terminal on Windows and Ubuntu. Everyday Git (J1) and advanced Git (J2) are complete.
From the SSH milestone (J3), the host book, tabbed terminals and keys are in; the rest is listed
below.

The interface is in French for now. Where it helps to find something on screen, this page gives
the French label in quotes.

## What Ramure does today

### Repositories

- **Tabs**: one repository per tab, reopened at startup. Ctrl + T opens a tab, Ctrl + W closes
  it, Ctrl + Tab moves to the next one; a middle click closes a tab too.
- **Start page** of a new tab: open a folder, clone, initialise, and the list of recent
  repositories.
- **Clone** (`git clone --progress`): live progress, can be cancelled. A clone that fails leaves
  no folder behind.
- **Resizable cards**: the branch list on the left and the detail panel on the right are resized
  by dragging the handle between them and the graph, or from the keyboard. Widths are kept from
  one launch to the next; a double-click restores the default width.
- **Cards that can be hidden**: each of the two is hidden and shown again with the button at its
  end of the toolbar, with Ctrl + Shift + B (branch list) or Ctrl + Shift + E (detail panel), or
  from the command palette, to read the graph on the whole width. A hidden detail panel only
  comes back when asked for: Enter or a double-click on a commit, a click on the "WIP" row, or
  "Voir les modifications en cours".

### Graph

- **Graph**: `git log`, lanes computed in Rust, drawn on a canvas. Only the visible rows exist in
  the page; 50,000 commits scroll smoothly.
- **Two styles**, chosen in the settings. "Ruban" (the default): a dot with the author's
  initials on each commit, soft curves, the message in a capsule of the branch colour, an
  animated halo on the current commit. "Classique": points and curves, more compact.
- **A "Branche / Tag" column** on the left of the graph, linked to its commit by a line. A
  computer icon marks the last local commit of a branch, a cloud the last remote one; on the same
  commit they share one label. Extra labels unfold on hover.
- **Sidebar**: local branches with their ahead and behind counts, remote branches, tags, stashes,
  and a filter.
- **Commit detail**: full message, changed files, the diff of each file.
- **Diff**, unified or side by side; side by side, the changed part of a line is highlighted.
- **Command palette** (Ctrl + K, or the magnifier at the bottom left): one search across actions,
  branches, tags, tabs and recent repositories.

### Work in progress

- **Index**: the "WIP" row opens the changes panel. Stage and unstage by file or all at once,
  discard after confirmation.
- **Staging by hunk or by line**: in the diff of a changed file, click the lines to stage
  (Shift + click for a range), or stage a whole hunk.
- **Commit**: message, Ctrl + Enter, and "Modifier le dernier commit" (amend).
- **Stashes**: stash with untracked files, pop, apply, drop, view the content.

### Branches, tags and history

Everything goes through a right-click on a commit or on a label, and through the toolbar.

- **Branches**: switch with a double-click, create from a commit, rename, delete locally and on
  the remote, track a remote branch.
- **Double-click on a remote branch**: switches to the local branch and brings it up to date. If
  it is behind, it is fast-forwarded; if the two have diverged, Ramure asks before rebasing.
- **Tags**: lightweight or annotated, push, delete locally and on the remote.
- **Merge, rebase, cherry-pick, revert, reset** (soft, mixed, hard).
- **Conflicts**: when an operation stops, a banner offers to continue, skip or abort. Clicking a
  conflicted file opens the **three-way editor**: both versions at the top, with the conflicting
  passage highlighted, and the result at the bottom. For each conflict, take one side or both;
  the result can still be edited by hand, and the common ancestor is shown on demand. A file
  deleted on one side, or a binary file, is resolved by keeping one side.
- **Dragging a branch onto another** (in the graph or the sidebar) offers to merge or rebase it.
- **Interactive rebase**: reorder with the mouse, reword, squash, drop commits. git launches
  Ramure itself as the "sequence editor": no external tool is needed.
- **Safety nets**: before a reset, a rebase or an interactive rebase, the position of the branch
  is kept in `refs/ramure/backup/<branch>-<date>` (`git reset --hard <that ref>` goes back to
  it). Rewriting commits that were already pushed shows a warning.

### Synchronisation

- **Fetch** ("Récupérer", `git fetch --all --prune`), **Pull** ("Tirer": fast-forward by
  default; rebase or merge as a setting), **Push** ("Pousser": a branch without an upstream is
  published automatically), and **force push** (`--force-with-lease`) after confirmation.
- **Automatic fetch** every 5 minutes, adjustable from 1 to 60 or turned off.

Authentication to remotes goes through what git already uses: a credential helper (Git Credential
Manager) or the SSH agent. Ramure does not ask for a password or a passphrase itself.

### SSH

- **Host book**: name, address, port, user, authentication (agent and default keys, key file, or
  password), group, tags, colour, startup command. It is stored in a SQLite file
  (`%APPDATA%\Ramure` or `~/.config/ramure`), with no secret in it.
- **Import of `~/.ssh/config`**: Host, HostName, User, Port, IdentityFile. A host that goes
  through a bastion (ProxyJump) is imported, but cannot be connected to yet.
- **Tabbed terminals**: a double-click on a host opens a terminal; "Reconnecter" restarts a
  session that has ended, startup command included.
- **Fingerprints**: on the first connection, the server's fingerprint is shown; accepting it
  writes it to `known_hosts`. A fingerprint that has changed blocks the connection.
- **Keys**: the list of keys in `~/.ssh`, generation of an ed25519 key with an optional
  passphrase, copy of the public key. Keys stay in `~/.ssh`.
- **A host's key**: in its form, the key file is typed or picked with "Parcourir…", which opens
  the system file dialog in `~/.ssh`.
- A password or a passphrase is asked for at connection time and is never stored.

### Shortcuts and command log

- **Configurable keyboard shortcuts**: in the settings, under "Raccourcis clavier". In a
  terminal, only shortcuts with Shift, Alt or a function key are active.
- **Command log**: every git command that was run, with its duration, from the status bar.

### Not there yet

- From milestone J3: secrets in the system keyring, a local terminal, a session log, automatic
  reconnection, terminal search and themes, export to `~/.ssh/config`, import of an existing key.
- Milestone J4: SFTP, tunnels, installers and updates.
- The interface exists in French only.

## Requirements

- Git 2.23 or later
- Node.js 20 or later
- Stable Rust (https://rustup.rs)

Windows: the "Build Tools for Visual Studio" (C++ workload) and WebView2, already present on an
up-to-date Windows 10 or 11.

Ubuntu 22.04 and 24.04:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## Run

```bash
npm install
```

```bash
npm run tauri dev
```

Optimised build, without an installer (binary in `src-tauri/target/release/`):

```bash
npm run tauri build -- --no-bundle
```

## Check

```bash
npm run check
```

This command checks the types, then rejects any colour, text size, weight, radius or duration
hard-coded in a component: those values come from the tokens in `src/app.css`. Last, it rejects
two files of `src/` whose names differ only by case, such as `Toasts.svelte` and
`toasts.svelte.ts`: on Windows an import of one loads the other, and the window stays blank.

```bash
cd src-tauri && cargo test
```

To measure how long a real repository takes to load, without opening the interface:

```bash
cd src-tauri && RAMURE_TEST_REPO=/path/to/repository cargo test real_repo -- --nocapture
```

## Layout

| Folder | Content |
| --- | --- |
| `src/` | Svelte 5 interface: `lib/git/`, `lib/ssh/`, `lib/ui/`; the design tokens are in `app.css` |
| `docs/` | The [design guidelines](docs/design-guidelines.md), the logo in `logo/`, and the design decisions in `design/` |
| `scripts/` | Checks run by `npm run check`, and the tool that derives the application icons from the logo |
| `src-tauri/src/git/` | Running git, the log parser, lane computation |
| `src-tauri/src/ssh/` | russh sessions, reading `~/.ssh/config`, the host book and keys |
| `.claude/` | For Claude Code: the `design-reviewer` subagent, which reviews interface changes against the design guidelines, and the test that measures it |

The interface touches neither the disk nor the network: it calls the commands of the Rust core.

The DM Sans and DM Mono fonts are embedded in the application. They are published under the SIL
Open Font License 1.1.

## Known limits

- Hosts that go through a bastion (`ProxyJump`) are not supported.
- The graph column is limited to about twenty lanes.
