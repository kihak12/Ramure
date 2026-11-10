# Review test for the `design-reviewer` subagent

A change that breaks the design guidelines on purpose, with a written answer key. It measures
what the `design-reviewer` subagent
([`.claude/agents/design-reviewer.md`](../../agents/design-reviewer.md)) finds, what it misses,
what it reports wrongly, how it grades what it finds, and what it sees on screenshots.

Replay it when the definition of the subagent, its model or its effort level changes, and when
the guidelines change a rule the answer key relies on.

## What is in this folder

| File | Content |
| --- | --- |
| [`change/CleanupPanel.svelte`](change/CleanupPanel.svelte), [`change/cleanup.ts`](change/cleanup.ts) | The change to review: a "Nettoyer les branches" panel and its interface text. It holds 25 planted violations and 12 decoys, which are compliant code that looks like a violation. |
| [`screens/`](screens) | Eight screenshots of the panel, mounted on its own with sample branches: four states, each in light and in dark. `mesures.json` holds the measures taken in the page for each screenshot. |
| [`capture/`](capture) | The page and the script that take the screenshots. |
| [`answer-key.md`](answer-key.md) | The planted violations with their severity, the decoys, what the screenshots show, and the cases left to judgment. |

The change hard-codes no value, so `npm run check` passes on it, with a single warning from
Svelte about a positive `tabindex`: the test is about what the automatic check cannot see. The
two files are kept out of `src/`, so nothing builds or checks them where they are.

## Running it

1. Copy the change and the screenshots where a real change would put them, and keep the copies
   out of Git:

   ```bash
   cp .claude/tests/design-reviewer/change/* src/lib/git/
   mkdir -p captures && cp .claude/tests/design-reviewer/screens/*.png captures/
   printf 'src/lib/git/CleanupPanel.svelte\nsrc/lib/git/cleanup.ts\ncaptures/\n' >> .git/info/exclude
   ```

2. Ask the `design-reviewer` subagent for a review, with this request and nothing else. The
   request must not say that this is a test, and must not name this folder. To test the review
   of the code alone, leave out the last paragraph.

   ```text
   Relis ce changement contre la charte graphique.

   Le changement ajoute un panneau « Nettoyer les branches » au module Git : il liste les
   branches locales déjà fusionnées et propose de les supprimer.

   Fichiers modifiés (nouveaux tous les deux) :
   - src/lib/git/CleanupPanel.svelte
   - src/lib/git/cleanup.ts

   Captures d'écran du panneau, monté seul avec des branches d'exemple, chacune en thème clair
   et en thème sombre :
   - captures/01-repos-clair.png, captures/01-repos-sombre.png : fenêtre de 760 px de large.
     Deux lignes cochées au clavier, le focus dans la liste, la souris sur la cinquième ligne.
   - captures/02-confirmation-clair.png, captures/02-confirmation-sombre.png : le même état,
     après Entrée sur « Supprimer la sélection ».
   - captures/03-erreur-clair.png, captures/03-erreur-sombre.png : après une suppression qui
     échoue, puis un clic sur « Fetch », qui reste en cours.
   - captures/04-etroit-clair.png, captures/04-etroit-sombre.png : le premier état, dans une
     fenêtre de 440 px de large.
   ```

3. Score the review against the answer key, as described below.

4. Remove the copies:

   ```bash
   rm -r src/lib/git/CleanupPanel.svelte src/lib/git/cleanup.ts captures
   sed -i '\#^src/lib/git/CleanupPanel.svelte$#d;\#^src/lib/git/cleanup.ts$#d;\#^captures/$#d' .git/info/exclude
   ```

Three things to know:

- A session that is already open can keep the definition it loaded when it started. After a
  change to the definition, run the test from a new session. Or ask a general-purpose agent to
  read `.claude/agents/design-reviewer.md` and to follow it with reading tools only: that is how
  every run below was made, except the first two.
- To compare two models, give the model and the effort level in the call. A result does not say
  which model produced it.
- A run is void if the review read anything under `.claude/tests/`. Its "Relu" line lists what
  it read.

## Scoring

| Measure | How |
| --- | --- |
| Found | Planted violations listed under "Violations", out of 25. One that is listed under "Dérives" counts as missed. |
| Decoys reported | Decoys listed under "Violations", out of 12. The target is 0. |
| Severity | Planted violations that carry the score of the answer key, out of 25. With two runs, the number that have the same score in both, and the number that are within one point: the second measure does not depend on the key. |
| Screen | Facts of "What the screenshots show" that the review reports, under whichever part, out of 6. And what it says of the screen that the measures contradict: the target is nothing. |
| Quotations | Check a few: the section, the text of the rule, the file and the line. |
| Sorting | A violation is in the change and rests on a rule that applies without doubt. A departure in a file that was not changed is a drift. A finding appears once. |
| Cost | The duration and the tokens of the run. |

A violation that the answer key does not list is not an error in itself: check it against the
guidelines. The cases the key leaves to judgment are not scored.

## Runs so far

All on 10 November 2026, one run per row.

### The code alone

| Definition | Model, effort | Found | Decoys reported | Violations | Drifts | Duration | Tokens |
| --- | --- | --- | --- | --- | --- | --- | --- |
| First version | Sonnet, `high` | 25 / 25 | 0 / 12 | 26 | 7 | 1 min 34 | 51,774 |
| First version | Haiku 5.5, `xhigh` | 25 / 25 | 0 / 12 | 30 | 13 | 6 min 27 | 133,958 |
| With "Violation or drift" | Sonnet, `high` | 25 / 25 | 0 / 12 | 28 | 7 | 2 min 27 | 131,014 |
| With "Violation or drift" | Haiku 5.5, `xhigh` | 25 / 25 | 0 / 12 | 26 | 14 | 6 min 40 | 213,000 |

- The first two rows called the subagent. The others went through a general-purpose agent that
  read the definition: it carries a longer prompt of its own, so its tokens and its duration do
  not compare with the first two rows.
- With the first version, the Haiku run listed as a violation a departure in `src/app.css`,
  which the change had not touched, and listed one finding both as a violation and as a drift.
  With the "Violation or drift" section, neither happened again: what lies outside the change
  came back as a drift that starts with "Hors du changement :".

### With the screenshots and the severity

All four runs are Sonnet at `high`, two with each scale. "Same in both" compares the two runs of
a scale with each other.

| Severity | Found | Decoys reported | As in the key | Same in both | Screen | Violations | Drifts | Duration | Tokens |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Three levels | 25 / 25 | 0 / 12 | 25 / 25 | 25 / 25 | 5 / 6 | 27 | 9 | 3 min 17 | 156,327 |
| Three levels | 25 / 25 | 0 / 12 | 25 / 25 | | 5 / 6 | 28 | 8 | 4 min 08 | 165,617 |
| Score from 1 to 10 | 25 / 25 | 0 / 12 | 24 / 25 | 24 / 25 | 5 / 6 | 29 | 8 | 4 min 00 | 162,010 |
| Score from 1 to 10 | 25 / 25 | 0 / 12 | 25 / 25 | | 5 / 6 | 28 | 6 | 4 min 23 | 169,653 |

- **Severity, three levels.** The two runs gave the same level to each of the 25 planted
  violations. The key first put A4, the order of the buttons in the dialog, one level lower;
  both runs read it as a safeguard of a destructive action, and the key was aligned on them.
- **Severity, from 1 to 10.** The two runs gave the same score to 24 of the 25, and scores one
  point apart to the last, S4, the checkbox: 3 in one run, 4 in the other. The key was aligned
  on two points after these runs. Y2, the counter, went from 2 to 6: both runs scored it as a
  shared class written again, which is what the table says. S4 went from 2 to 4, the score of
  a checked state that is not rendered in `--accent`. Against the key as first written, each
  run had 23 of 25.
- **Screen.** Every run opened the eight screenshots. All four reported the crushed list at
  440 px, the hover that cannot be told from the selection, the summary that is a card in dark
  only, the missing veil, the focus that stays outside the confirmation, and the small buttons.
  One also saw that "Fetch" grows by 20 px when its icon appears, which the measures confirm.
- No run said that the disabled buttons still read as active (E6): all kept to the opacity
  written in the code.
- The runs made small mistakes about a screenshot, none of which changes a violation: the row
  cut at the bottom of the list counted as the ninth when it is the tenth, the SHA said to
  wrap at 440 px when only the commit counts do, the confirmation said to cover rows 6 to 8
  when it covers rows 6 to 9.
- The focus in the confirmation: the screenshot shows it on "Supprimer la sélection", the code
  asks for it on "Supprimer". Three runs put that under "À vérifier à l'écran"; the last one
  explained it, rightly, by an `autofocus` that is not applied to an element inserted later.
- One run listed the close button, 20 px high, as a violation, and its cause, `.btn.small` in
  `src/app.css`, as a drift: the same finding in two parts.

In every run, the quotations that were checked were right. One run per row: a difference of a
few points between two rows is noise.

## Taking the screenshots again

When a file of `change/` or `src/app.css` changes, the screenshots are out of date. With the
change copied into `src/lib/git/` (step 1 above):

```bash
node .claude/tests/design-reviewer/capture/capture.cjs .claude/tests/design-reviewer/screens
```

The script starts the development server, mounts the panel in a page of its own and takes the
eight screenshots with their measures. It needs Playwright and its Chromium, which are not
dependencies of the project. Then update "What the screenshots show" in the answer key from
`screens/mesures.json`.

## Keeping it up to date

- The answer key quotes sections of the guidelines. When one of those rules changes, update the
  key. If a planted violation becomes compliant, replace it with another.
- The line numbers in the key are those of the files in `change/`: keep them in step when a file
  changes.
- Do not add the planted cases to the search leads of the definition, and do not use one as the
  example of its review: the test would then measure the leads, not the review.
