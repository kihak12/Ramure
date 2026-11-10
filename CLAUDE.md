# Ramure

A graphical Git client and an SSH client in one window, for Windows and Ubuntu.
Tauri 2, Rust, Svelte 5.

## Check

- `npm run check`: types, then design tokens (no hard-coded style value in a component).
- `cd src-tauri && cargo test`.

## Design

- `docs/design-guidelines.md` is the reference. When the code and the guidelines disagree, the
  guidelines are right; a rule changes in the guidelines first.
- Whenever a component, a style, a token, an icon or a piece of interface text changes, have the
  change reviewed by the `designer` subagent before proposing it. Give it the list of changed
  files: it is read-only and cannot run `git`. Fix the violations it reports, and settle its
  drifts with whoever asked for the change.

## Conventions

- Documentation (every Markdown file) is written in English.
- The interface, code comments, commits and pull requests are in French.
- One simple issue, then a dedicated pull request.
