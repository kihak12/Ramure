# Trial of the `ux-designer` subagent

Screenshots of a real screen of Ramure, a task, and a grid to read what the `ux-designer`
subagent ([`.claude/agents/ux-designer.md`](../../agents/ux-designer.md)) returns. It served to
choose its model, and it serves again when its definition, its model or its effort level changes.

There is no answer key here: a proposal is a choice, not a fact. The grid checks what can be
checked, which is whether the subagent sees the screen as it is, stays in its role, and finds
the friction a person finds.

## What is in this folder

[`screens/`](screens) holds eight screenshots of the Git module, 1440 × 880 px, on a repository
with two modified files. Four steps of one task, each in light and in dark:

| Screenshot | What it shows |
| --- | --- |
| `01-graph-*` | The repository open, nothing selected. |
| `02-changes-*` | After a click on the "// WIP" row of the graph. |
| `03-diff-*` | After a click on `src/main.rs` in the "Unstaged" list. |
| `04-line-picked-*` | After a click on the added line of the diff, then the commit message typed. |

They were taken on November 14, 2026, on the English interface, with
[`scripts/screen.sh`](../../../scripts/screen.sh), which runs the application on a virtual
display, and `xdotool` for the clicks. The repository is the one that
[`scripts/demo-repo.sh`](../../../scripts/demo-repo.sh) builds. If the screenshots are taken
again, bring the request and the reference below in step with what they show.

## Running it

1. Copy the screenshots where a real request would put them, and keep the copies out of Git:

   ```bash
   mkdir -p captures && cp .claude/tests/ux-designer/screens/*.png captures/
   echo 'captures/' >> .git/info/exclude
   ```

2. Ask the `ux-designer` subagent, with this request and nothing else. To compare two models,
   give the model and the effort level in the call.

   ```text
   Look at this screen and propose what would make it better.

   Screen: the Git module, while the user prepares a commit.
   Task: commit only part of the changes, here a single line of `src/main.rs`, and leave the
   rest aside.

   Screenshots of the application, each in the light and the dark theme, in a window of
   1440 × 880 px:
   - captures/01-graph-light.png, captures/01-graph-dark.png: the repository open, with two
     modified files and nothing selected.
   - captures/02-changes-light.png, captures/02-changes-dark.png: after a click on the
     "// WIP" row of the graph.
   - captures/03-diff-light.png, captures/03-diff-dark.png: after a click on `src/main.rs` in
     the "Unstaged" list.
   - captures/04-line-picked-light.png, captures/04-line-picked-dark.png: after a click on the
     added line of the diff, then the commit message typed.
   ```

3. Read the answer with the grid below.

4. Remove the copies:

   ```bash
   rm -r captures
   sed -i '\#^captures/$#d' .git/info/exclude
   ```

A run is void if the answer read anything under `.claude/tests/`: its "Looked at" line lists what
it read.

## The grid

| Measure | How |
| --- | --- |
| Main friction | Whether the proposals include the main friction of the reference below, and at which rank. |
| False observations | What a "Finding" says of a screenshot or of the code that is not true. Check each against the screenshot or the file. The target is 0. |
| Already there | Proposals for something the application already does. The target is 0. |
| Outside the frame | Proposals presented as a rule that is broken, and proposals that go against the guidelines without saying "touches the guidelines". The target is 0. |
| Form | Six proposals at most, each with its tag and its four parts, one change per proposal. |
| Cost | The duration and the tokens of the run. |

## The reference

Written before the first runs, from the screenshots.

**The main friction.** On `04-line-picked-*`, a line is picked and a message is typed, but
"Commit" stays disabled. Its hint says "Nothing is staged: stage a file or some lines." without
saying that the picked line is not staged yet. The step that is missing, "Stage 1 line", is at
the top of the center card, far from the commit box at the bottom right.

**Already there**, so not to be proposed as new: Ctrl + Enter to commit, Shift + click for a
range of lines, "Stage hunk", staging and discarding a file from its row on hover, amending
the last commit, the unified and side-by-side views.

**Settled by the guidelines**, so not to be proposed against without saying so: the git commands
shown in the status bar and the word "commit" kept as it is (section 12), no border or shadow on
a card (sections 1 and 6), one primary button per view (section 9.1).

## Runs so far

All on November 10, 2026, on the French screenshots of then, one run per row, with the
definition as it was first written. The labels below are given in English.

| Model, effort | Proposals | Main friction | False observations | Already there | Outside the frame | Duration | Tokens |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Sonnet, `high` | 6 | first, 8/10 | 0 | 0 | 0 | 1 min 22 | 75,853 |
| Opus, `high` | 4 | first, 8/10 | 0 | 0 | 0 | 2 min 58 | 92,635 |
| Haiku 5.5, `xhigh` | 5 | second, 6/10 | 0 | 0 | 0 | 5 min 22 | 159,815 |

- **What all three proposed:** saying why "Commit" is disabled, and marking a file that is
  only partly staged. All three also noticed that a chosen line shows "✓" in place of its sign,
  which can be read as "already staged".
- **What one alone proposed.** Opus: a warning when only one half of a replaced line is chosen,
  since the commit would then hold the old line and the new one. Sonnet: showing the chosen
  lines and their action in the right card, and choosing lines from the keyboard. Haiku:
  staging a line in one click from the line itself, and letting the two lists take the height
  of their content.
- **Observations.** Every observation that was checked against the screenshots and the code was
  right, and each run said which of its proposals came from the code alone. One run doubted
  what it saw of the hint of the commit field in the dark theme; a crop of the screenshot shows
  it was right.
- **Role.** No run presented a proposal as a broken rule. Departures from the guidelines went
  under "Seen in passing": four lines for Sonnet, seven for Opus, nine for Haiku.
- **The choice.** The grid does not separate Sonnet from Opus. Opus kept fewer proposals and
  went further into what the commit would contain; Sonnet was twice as fast. The definition
  names Sonnet at `high`, and the call can name another model for a screen that deserves it.
- One run per row: this trial shows that the three models can do the work, not which one is
  better by a small margin.

## What it sees depends on what the screenshots show

Two more runs, the same day, on the graph and the task "find where the branches are, then
switch to one". Both with Sonnet at `high`, after a step was added to the method: put real
content in the screen, since screenshots are often taken on tidy data.

| Screenshots | What the answer said of long branch names |
| --- | --- |
| The repository of the trial, with short names | Nothing was cut on screen. The fixed width of the label column, 164 px, came up only as a caution inside another proposal. |
| A repository with names like `feature/PROJ-1482-…`, and the notification of a switch | A proposal on the names cut in the sidebar, in the labels and in the toolbar, and one on the notification, whose title runs over two lines and whose body repeats the name. It also measured that the toolbar shifts by 150 px when the name of the branch changes. |

The added step is not enough on its own: the subagent reports what it is shown. Give it
screenshots taken on real data, with long names, many rows and the states that go wrong.
