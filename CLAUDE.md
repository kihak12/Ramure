# Ramure

A graphical Git client and an SSH client in one window, for Windows and Ubuntu.
Tauri 2, Rust, Svelte 5.

## Check

- `npm run check`: types, then design tokens (no hard-coded style value in a component), then
  the contrasts and color distances of every color theme, then file names (no two files of
  `src/` that differ only by case: Windows would confuse them).
- `cd src-tauri && cargo test`.

## Design

- `docs/design-guidelines.md` is the reference. When the code and the guidelines disagree, the
  guidelines are right; a rule changes in the guidelines first.
- Whenever a component, a style, a token, an icon or a piece of interface text changes, have the
  change reviewed by the `design-reviewer` subagent before proposing it. Give it the list of
  changed files: it is read-only and cannot run `git`. Give it screenshots of the change too, in
  light and in dark, whenever you can take them: without them it only judges the code. Fix the
  violations it reports, the most severe first, and settle its drifts with whoever asked for
  the change.
- Before building a new screen, or when asked how a screen could be better, ask the
  `ux-designer` subagent for proposals. Give it screenshots of the screen in light and in dark,
  and the task the user does there. It proposes, and whoever asked decides: build only what
  was kept, then have it reviewed by `design-reviewer`.
- `scripts/screen.sh` takes screenshots of the application on Linux without a display: it runs
  the release build on a virtual screen, in light or in dark, and `xdotool` drives it.
- To reproduce a bug, to check a change in the running application, or to take the screenshots
  for the two subagents above, brief the `screen-tester` subagent rather than driving the screen
  yourself: the screenshots then stay out of your context. Give it the scenarios, the gesture,
  the number of tries and what to look at, not the result you expect. It returns what it saw,
  with counts and the paths of its screenshots; open one of them yourself before relying on it.

## Conventions

- Everything is written in English: the interface, the documentation (every Markdown file), code
  comments, commits, issues and pull requests.
- One simple issue, then a dedicated pull request.
