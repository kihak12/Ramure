# Review test for the `designer` subagent

A change that breaks the design guidelines on purpose, with a written answer key. It measures
what the `designer` subagent ([`.claude/agents/designer.md`](../../agents/designer.md)) finds,
what it misses and what it reports wrongly.

Replay it when the definition of the subagent, its model or its effort level changes, and when
the guidelines change a rule the answer key relies on.

## What is in this folder

| File | Content |
| --- | --- |
| [`change/CleanupPanel.svelte`](change/CleanupPanel.svelte), [`change/cleanup.ts`](change/cleanup.ts) | The change to review: a "Nettoyer les branches" panel and its interface text. It holds 25 planted violations and 12 decoys, which are compliant code that looks like a violation. |
| [`answer-key.md`](answer-key.md) | The planted violations, the decoys, and the cases left to judgment. |

The change hard-codes no value, so `npm run check` passes on it, with a single warning from
Svelte about a positive `tabindex`: the test is about what the automatic check cannot see. The
two files are kept out of `src/`, so nothing builds or checks them where they are.

## Running it

1. Copy the change where a real one would be, and keep the copies out of Git:

   ```bash
   cp .claude/tests/designer/change/* src/lib/git/
   printf 'src/lib/git/CleanupPanel.svelte\nsrc/lib/git/cleanup.ts\n' >> .git/info/exclude
   ```

2. Ask the `designer` subagent for a review, with this request and nothing else. The request
   must not say that this is a test, and must not name this folder.

   ```text
   Relis ce changement contre la charte graphique.

   Le changement ajoute un panneau « Nettoyer les branches » au module Git : il liste les
   branches locales déjà fusionnées et propose de les supprimer.

   Fichiers modifiés (nouveaux tous les deux) :
   - src/lib/git/CleanupPanel.svelte
   - src/lib/git/cleanup.ts
   ```

3. Score the review against the answer key, as described below.

4. Remove the copies:

   ```bash
   rm src/lib/git/CleanupPanel.svelte src/lib/git/cleanup.ts
   sed -i '\#^src/lib/git/CleanupPanel.svelte$#d;\#^src/lib/git/cleanup.ts$#d' .git/info/exclude
   ```

Three things to know:

- A session that is already open can keep the definition it loaded when it started. After a
  change to the definition, run the test from a new session. Or ask a general-purpose agent to
  read `.claude/agents/designer.md` and to follow it with reading tools only: that is how the
  last two runs below were made.
- To compare two models, give the model and the effort level in the call. A result does not say
  which model produced it.
- A run is void if the review read anything under `.claude/tests/`. Its "Relu" line lists what
  it read.

## Scoring

| Measure | How |
| --- | --- |
| Found | Planted violations listed under "Violations", out of 25. One that is listed under "Dérives" counts as missed. |
| Decoys reported | Decoys listed under "Violations", out of 12. The target is 0. |
| Quotations | Check a few: the section, the text of the rule, the file and the line. |
| Sorting | A violation is in the change and rests on a rule that applies without doubt. A departure in a file that was not changed is a drift. A finding appears once. |
| Cost | The duration and the tokens of the run. |

A violation that the answer key does not list is not an error in itself: check it against the
guidelines. The cases the key leaves to judgment are not scored.

## Runs so far

All on 10 November 2026, one run per row.

| Definition | Model, effort | Found | Decoys reported | Violations | Drifts | Duration | Tokens |
| --- | --- | --- | --- | --- | --- | --- | --- |
| First version | Sonnet, `high` | 25 / 25 | 0 / 12 | 26 | 7 | 1 min 34 | 51,774 |
| First version | Haiku 5.5, `xhigh` | 25 / 25 | 0 / 12 | 30 | 13 | 6 min 27 | 133,958 |
| With "Violation or drift" | Sonnet, `high` | 25 / 25 | 0 / 12 | 28 | 7 | 2 min 27 | 131,014 |
| With "Violation or drift" | Haiku 5.5, `xhigh` | 25 / 25 | 0 / 12 | 26 | 14 | 6 min 40 | 213,000 |

- The first two rows called the `designer` subagent. The last two went through a general-purpose
  agent that read the definition: it carries a longer prompt of its own, so its tokens and its
  duration do not compare with the first two rows.
- With the first version, the Haiku run listed as a violation a departure in `src/app.css`,
  which the change had not touched, and listed one finding both as a violation and as a drift.
  With the "Violation or drift" section, neither happened again: what lies outside the change
  came back as a drift that starts with "Hors du changement :".
- In all four runs, the quotations that were checked were right.
- One run per row: a difference of a few points between two rows is noise.

## Keeping it up to date

- The answer key quotes sections of the guidelines. When one of those rules changes, update the
  key. If a planted violation becomes compliant, replace it with another.
- The line numbers in the key are those of the files in `change/`: keep them in step when a file
  changes.
- Do not add the planted cases to the search leads of the definition: the test would then
  measure the leads, not the review.
