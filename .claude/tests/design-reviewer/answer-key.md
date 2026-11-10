# Answer key

What the change in [`change/`](change) hides, written before any review was run. How to use it
is in the [README](README.md).

- **P** is `change/CleanupPanel.svelte`, reviewed as `src/lib/git/CleanupPanel.svelte`.
- **T** is `change/cleanup.ts`, reviewed as `src/lib/git/cleanup.ts`.
- The number in brackets is the section of the [design guidelines](../../../docs/design-guidelines.md)
  that holds the rule.
- **Sev.** is the severity, from 1 to 10, that the table of the definition of the subagent gives
  to the rule: 8 to 10 is "bloquante", 4 to 7 "majeure", 1 to 3 "mineure".

## Planted violations (25)

Five are "bloquantes", thirteen "majeures", seven "mineures".

### Interface text

| | Where | What | Sev. |
|---|---|---|---|
| T1 | T:16 | "Oups" and an exclamation mark in an error, which does not say what to do (12). | 5 |
| T2 | T:6 | The plural does not agree: "1 branches sélectionnées." (12). | 1 |
| T3 | T:11, T:16 | An ordinary space before "?" and "!" instead of a non-breaking space (12). | 1 |
| T4 | T:11 | The confirmation of a destructive action says neither what is lost nor whether it can be recovered (9.10, 12). | 10 |
| T5 | P:71 | "Fetch" instead of "Récupérer" (12). | 5 |
| T6 | P:78 | "Cochez": "vous" instead of "tu" (12). | 5 |
| T7 | P:120 | An emoji, "⚠️" (7). | 5 |

### Colour

| | Where | What | Sev. |
|---|---|---|---|
| C1 | P:69, P:124 | Two primary buttons in one view (2.3, 9.1). | 6 |
| C2 | P:170 | The accent used as decoration, on the title (2.3). | 5 |
| C3 | P:330 | `--ok` decorating the border of the footer (2.3). | 5 |
| C4 | P:267 | The hover of a row uses `--sel` instead of `--hover` (9, 9.6). | 4 |
| C5 | P:105 | "Merged" and "not merged" told by colour alone (11, 9.6). | 8 |

### Shapes and surfaces

| | Where | What | Sev. |
|---|---|---|---|
| S1 | P:151 | A shadow on a card (1, 6, 9.8). | 7 |
| S2 | P:303-310 | A card inside a card, `.resume` (6). | 7 |
| S3 | P:227 | An 8 px button inside a 6 px field; what can be clicked is 6 px (5). | 2 |
| S4 | P:273-274 | A 12 px checkbox instead of 15 px (9.3). | 2 |
| S5 | P:336-350 | A dialog without the `--scrim` veil (6, 9.10). | 6 |

### Typography

| | Where | What | Sev. |
|---|---|---|---|
| Y1 | P:294-297 | A SHA that is not in the code font (3.1). | 4 |
| Y2 | P:172-177 | A counter at 11.5 px instead of 10.5 px, without tabular figures (9.5, 3.4). | 2 |

### Motion

| | Where | What | Sev. |
|---|---|---|---|
| M1 | P:157 | An entrance offset of 48 px, above the 10 px maximum (8). | 3 |

### States, keyboard, dialog

| | Where | What | Sev. |
|---|---|---|---|
| A1 | P:332-335 | Disabled at 60% with the default cursor, instead of 45% and "not allowed" (9). | 4 |
| A2 | P:124 | `tabindex="1"` breaks the tab order (11). | 9 |
| A3 | P:125 | An icon at 15 px, outside 14, 16 and 18 (7). | 2 |
| A4 | P:135-136 | In the dialog of a destructive action, the action comes before "Annuler" (9.10). | 10 |
| A5 | P:135 | In the dialog, the code sends focus to the destructive button on opening (9.10). | 10 |

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

## What the screenshots show (6)

What only the rendering tells, with the measures taken in the page
([`screens/mesures.json`](screens/mesures.json)). A review that is given the screenshots should
report each of these, under whichever part.

| | Screenshot | What it shows |
|---|---|---|
| E1 | `04-etroit-*` | At 440 px the list is crushed: the summary keeps its 200 px, the 11 names are cut down to a letter or to nothing, and 7 commit counts wrap onto two lines that spill over the rows around them. |
| E2 | `02-confirmation-*` | The confirmation covers the bottom of the list and of the summary, and nothing is dimmed behind it. The keyboard focus stays outside the box: its ring is on "Supprimer la sélection". |
| E3 | `01-repos-*`, `04-etroit-*` | The hovered row, the fifth, is not checked and has the same background as the checked rows. |
| E4 | `*-sombre` | The summary stands out as a card only in the dark theme: in the light theme `--raised` and `--panel` are both white. |
| E5 | all | "Fetch" is 22.2 px high and the close button 32 × 20 px, against 30.2 px for the buttons of the footer. |
| E6 | `03-erreur-*` | "Fetch", in progress, and "Supprimer la sélection", with nothing selected, are disabled at 60%: they still read as buttons that can be clicked. |

What the screenshots do not show, and a review must not claim:

- At 760 px, only the long name is cut, with an ellipsis, and no count wraps.
- The filter field is 32.2 px high and the rows are 28 px high.
- The confirmation does not reach the footer: its buttons stay visible.
- The page does not overflow, at either width.

E2 corrects what the code suggests for A5. The code asks for the focus on "Supprimer", which is
the violation. In the page the focus does not move at all, because Svelte only honours
`autofocus` when nothing else has the focus.

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
- The filter field has no hover and rewrites `.field` with other inner margins (9.3, 14).
