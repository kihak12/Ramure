# Answer key

What the change in [`change/`](change) hides, written before any review was run. How to use it
is in the [README](README.md).

- **P** is `change/CleanupPanel.svelte`, reviewed as `src/lib/git/CleanupPanel.svelte`.
- **T** is `change/cleanup.ts`, reviewed as `src/lib/git/cleanup.ts`.
- The number in brackets is the section of the [design guidelines](../../../docs/design-guidelines.md)
  that holds the rule.

## Planted violations (25)

### Interface text

| | Where | What |
|---|---|---|
| T1 | T:16 | "Oups" and an exclamation mark in an error, which does not say what to do (12). |
| T2 | T:6 | The plural does not agree: "1 branches sélectionnées." (12). |
| T3 | T:11, T:16 | An ordinary space before "?" and "!" instead of a non-breaking space (12). |
| T4 | T:11 | The confirmation of a destructive action says neither what is lost nor whether it can be recovered (9.10, 12). |
| T5 | P:71 | "Fetch" instead of "Récupérer" (12). |
| T6 | P:78 | "Cochez": "vous" instead of "tu" (12). |
| T7 | P:120 | An emoji, "⚠️" (7). |

### Colour

| | Where | What |
|---|---|---|
| C1 | P:69, P:124 | Two primary buttons in one view (2.3, 9.1). |
| C2 | P:170 | The accent used as decoration, on the title (2.3). |
| C3 | P:330 | `--ok` decorating the border of the footer (2.3). |
| C4 | P:267 | The hover of a row uses `--sel` instead of `--hover` (9, 9.6). |
| C5 | P:105 | "Merged" and "not merged" told by colour alone (11, 9.6). |

### Shapes and surfaces

| | Where | What |
|---|---|---|
| S1 | P:151 | A shadow on a card (1, 6, 9.8). |
| S2 | P:303-310 | A card inside a card, `.resume` (6). |
| S3 | P:227 | An 8 px button inside a 6 px field; what can be clicked is 6 px (5). |
| S4 | P:273-274 | A 12 px checkbox instead of 15 px (9.3). |
| S5 | P:336-350 | A dialog without the `--scrim` veil (6, 9.10). |

### Typography

| | Where | What |
|---|---|---|
| Y1 | P:294-297 | A SHA that is not in the code font (3.1). |
| Y2 | P:172-177 | A counter at 11.5 px instead of 10.5 px, without tabular figures (9.5, 3.4). |

### Motion

| | Where | What |
|---|---|---|
| M1 | P:157 | An entrance offset of 48 px, above the 10 px maximum (8). |

### States, keyboard, dialog

| | Where | What |
|---|---|---|
| A1 | P:332-335 | Disabled at 60% with the default cursor, instead of 45% and "not allowed" (9). |
| A2 | P:124 | `tabindex="1"` breaks the tab order (11). |
| A3 | P:125 | An icon at 15 px, outside 14, 16 and 18 (7). |
| A4 | P:135-136 | In the dialog, the action comes before "Annuler" (9.10). |
| A5 | P:135 | In the dialog, focus goes to the destructive button on opening (9.10). |

## Decoys (12)

Compliant code that looks like a violation. None of these may appear under "Violations".

| | Where | Why it is compliant |
|---|---|---|
| D1 | P:250-255 | `.liste:focus-visible { outline: none }`: replaced by the 1.5 px inner ring on the active row (9.6). |
| D2 | P:215 | `.filtre input { outline: none }`: a field shows focus by its border and its halo (9.3). |
| D3 | P:178-186 | `.tourne` loops, with a linear curve: it is the waiting indicator, shown only while loading (8). |
| D4 | P:172-177 | `.compte` is a pill: a counter informs, it cannot be clicked (5, 9.5). |
| D5 | P:218-220 | `--faint` on the hint of an empty field (2.1). |
| D6 | P:247 | `border-right: 1px solid var(--line-soft)`: a separator inside a card (6). |
| D7 | P:116 | "HEAD" in capitals: the term is kept as it is (12). |
| D8 | P:73, P:84 | The buttons made of an icon alone have a tooltip and an accessible name (7). |
| D9 | P:221-232 | `.effacer` is exactly 24 × 24 px (4.3). |
| D10 | P:108 | "1 commit", "2 commits": the plural agrees (12). |
| D11 | P:120, P:318-324 | The error has `role="alert"`, in `--bad` on `--bad-soft` (11, 2.3). |
| D12 | P:336-350 | A border and `--shadow-pop` on the dialog: it floats (6). |

## Left to judgment

Not scored, whichever part they are reported under.

- The filter field has a hint and an accessible name, but no visible label (9.3).
- "Supprimer la sélection" opens a dialog and does not end with "…" (9.9 is about menus).
- The dialog is written by hand although `src/lib/ui/Dialog.svelte` exists (14).
- The state marker is a circle (5 reserves the circle for three uses).
- The "refresh" icon is used to fetch although `Icon.svelte` has a "fetch" icon (7).
- The active row is not brought back into range when the filter shortens the list, and is not
  scrolled into view (11).
- A small button is under 24 px high: the cause is `.btn.small` in `src/app.css`, outside the
  change (4.3).
