# Answer key

What the change in [`change/`](change) hides, written before any review was run, and rewritten
for the English interface in #151. How to use it is in the [README](README.md).

- **P** is `change/CleanupPanel.svelte`, reviewed as `src/lib/git/CleanupPanel.svelte`.
- **T** is `change/cleanup.ts`, reviewed as `src/lib/git/cleanup.ts`.
- **Rule** is the section of the [design guidelines](../../../docs/design-guidelines.md) and the
  text that a review should quote, in italics here.
- **Sev.** is the severity, from 1 to 10, that the table of the definition of the subagent
  ([`.claude/agents/design-reviewer.md`](../../agents/design-reviewer.md)) gives to the rule: 8
  to 10 is "blocking", 4 to 7 "major", 1 to 3 "minor".

## Planted violations (25)

Five are "blocking", fifteen "major", five "minor".

C1 names two buttons, and S5 three departures of one dialog. A review may list such a case as one
finding or as several: it counts as one planted violation found, and the extra findings are
neither errors nor decoys.

### Interface text

| | Where | What | Rule | Sev. |
|---|---|---|---|---|
| T1 | T:16 | "Oops, the deletion failed!": "Oops" and an exclamation mark, and the error says neither what to do nor what git answered. | § 12: *An error says what happened, then what to do. The output of git follows, as it is. No "Oops", no exclamation mark, no blame.* | 5 |
| T2 | T:6 | The plural does not agree: "1 branches selected." when one branch is checked. | § 12, typography: *plurals agree: "1 commit", "10 commits"* | 1 |
| T3 | P:78 | A period inside the closing quotation mark around a label: "then confirm with “Delete.”" | § 12, typography: *a period or a comma goes outside the closing quotation mark unless it belongs to what is quoted: what is quoted is a label, a name or a value, read as it is (“Reconnect”, “.pub”), and a period inside would seem to be part of it* | 1 |
| T4 | T:11, P:130-131 | The confirmation of a destructive action is its title alone, "Delete 2 branches?": it says neither what will be lost nor whether it can be recovered. | § 9.10 and § 12: *A destructive action says what will be lost, and whether it can be recovered.* | 10 |
| T5 | P:71 | "Retrieve", and "Retrieving…" while it runs, instead of git's word: "Fetch", "Fetching…". | § 12: *Git vocabulary is used as git uses it: the interface says what git says*; its table gives, for "fetch, pull, push": *Fetch, Pull, Push; a fetch, a pull.* | 5 |
| T6 | P:125 | Title Case on a button: "Delete Selection…". | § 12: *Sentence case everywhere, buttons, titles, menus, column headers and settings included: a capital for the first word and for proper names only (Ramure, Git, Slate). "Create branch", not "Create Branch"* | 5 |
| T7 | P:120 | An emoji, "⚠️", before the error. | § 7: *One set only, the application's own (`src/lib/ui/Icon.svelte`). No emoji, no icon from another library.* | 5 |

### Color

| | Where | What | Rule | Sev. |
|---|---|---|---|---|
| C1 | P:69, P:124 | Two primary buttons in one card: "Retrieve" in the header and "Delete Selection…" in the footer. Neither should be primary: an action in the header of a card is a small standard button, and the entry that opens the confirmation of a destructive action is ghost danger. | § 2.3: *Never two main buttons in one surface.* § 9.1, "Hierarchy", the primary: *One per surface at most* and *It is never destructive*; *In the header, a row or the foot of a card, the actions of the card are small standard buttons*; *Ghost danger is the entry that opens that dialog* | 6 |
| C2 | P:169 | The accent used as decoration, on the title of the panel. | § 2.3: *The accent stands for one thing at a time: the main button of a surface (9.1, "Hierarchy"), the focus, the active element, and the changes not yet committed* | 5 |
| C3 | P:330 | `--ok` decorating the border of the footer, 2 px wide. | § 2.3: *State colors (`--ok`, `--warn`, `--bad`) only say the state. They do not decorate.* | 5 |
| C4 | P:266 | The hover of a row uses `--sel` instead of `--hover`: a hovered row cannot be told from a checked one. | § 9.6: *Hover: a rounded `--hover` background under the row, 8 px corners.* § 2.3: *`--sel`, more saturated, so that the selection stays apart from the hover* | 4 |
| C5 | P:105, P:277-286 | "Merged" and "not merged" are told by the color of a dot alone, `--ok` or `--warn`, with no text and no accessible name. | § 11: *Never color alone. A state is backed by an icon, a sign or text* | 8 |

### Shapes and surfaces

| | Where | What | Rule | Sev. |
|---|---|---|---|---|
| S1 | P:150 | A shadow on a card. | § 1, what it forbids: *a border or a shadow around a card*; § 6: *A shadow says "this floats". A card never has one.*; § 9.8: *`--panel` background, 14 px corners, no border and no shadow.* | 7 |
| S2 | P:303-310 | A card inside a card: the summary, `.summary`, takes the background, the corners and the inner margin of a card. | § 6: *No card inside a card.* | 7 |
| S3 | P:226 | The clear button has 8 px corners inside a field of 6 px; what can be clicked takes 6 px. | § 5: *An element placed inside another never has a larger radius than the one that contains it.*; its table: `--radius-s`, 6 px, *What can be clicked: buttons, fields, branch and tag labels, menu items.* | 2 |
| S4 | P:271-276 | The checkbox is given back to the system (`appearance: auto`) at 12 px: not the 15 px square of the application, and checked in the system's blue, not in `--accent`. | § 9.3: *A checkbox is drawn by the application, never by the system, so that it reads the same in both themes.*; *Unchecked: a 15 px square with 4 px corners*; *Checked: the `--accent` background and border* | 4 |
| S5 | P:129-138, P:336-350 | The confirmation is a dialog written by hand, which departs from 9.10: no `--scrim` veil behind it, and it sits at the bottom right of the panel instead of the center of the window. `src/lib/ui/Dialog.svelte` exists. | § 6: *A dialog sits on the `--scrim` veil, with a 2 px blur.*; § 9.10: *Place. Centered across the window, and anchored 11% from its top*; § 14: *Before creating a component, look for the one that already exists.* | 6 |

### Typography

| | Where | What | Rule | Sev. |
|---|---|---|---|---|
| Y1 | P:294-297 | A SHA that is not in the code font. | § 3.1: *The code font is for what gets copied or typed in a terminal: SHAs, git commands, ...* | 4 |
| Y2 | P:171-176 | A counter at 11.5 px instead of 10.5 px, without tabular figures, which writes the shared `.chip` of `src/app.css` again. | § 9.5: *A pill on a `--sel` background, 10.5 px text in tabular figures.*; § 3.4: *Numbers that line up or that change (counters, dates, columns) use tabular figures*; § 14: *Before creating a component, look for the one that already exists.* | 6 |

### Motion

| | Where | What | Rule | Sev. |
|---|---|---|---|---|
| M1 | P:156 | An entrance offset of 48 px. | § 8: *An appearance comes from close by: an offset of 6 to 10 px at most.* | 3 |

### States, keyboard, dialog

| | Where | What | Rule | Sev. |
|---|---|---|---|---|
| A1 | P:332-335 | Disabled at 60% with the default cursor. | § 9, the states: Disabled, *45% opacity, "not allowed" cursor, no reaction on hover.* | 4 |
| A2 | P:124 | `tabindex="1"`: "Delete Selection…" comes first in the tab order, before the header and the list. | § 11: *The tab order follows the visual order.* (also § 9.1, "Keyboard") | 9 |
| A3 | P:125 | An icon at 15 px, outside the sizes of section 7: 14, 16, 18 and 20 px. | § 7, the table of sizes: 16 px *By default: menus, buttons, lists.* | 2 |
| A4 | P:134-135 | In the confirmation of a destructive action, "Delete" comes before "Cancel". | § 9.1, "Hierarchy": *The last line of a dialog, from left to right: the way out, then a secondary action if there is one, then the primary or the danger button, last, all at the right.*; § 9.10: *The buttons are on the last line, in the order of 9.1 ("Hierarchy"): "Cancel", then the action, at the right.* | 10 |
| A5 | P:134 | In the confirmation, the code sends the focus to "Delete", the destructive button, on opening (`autofocus`). | § 9.10: *On opening, focus goes to the first field. For a destructive action, it goes to "Cancel".*; § 9.1, "Keyboard": *for a destructive action the focus starts on the way out, so Enter leaves.* | 10 |

## Decoys (12)

Compliant code that looks like a violation. None of these may appear under "Violations".

| | Where | Why it is compliant | Rule |
|---|---|---|---|
| D1 | P:248-254 | `.list:focus-visible { outline: none }`: replaced by the 1.5 px inner ring on the active row. | § 9.6: *Keyboard focus: a 1.5 px inner ring in `--accent` on the selected row. Never an outline around the whole list.*; § 11: *An outline is never removed without another marker replacing it.* |
| D2 | P:214 | `.filter input { outline: none }`: the field shows the focus by its border and its halo (P:204-207). | § 9.3: *Focus: an `--accent` border and a 3 px halo in `--accent` at 25%. No outline on top of it.* |
| D3 | P:177-185 | `.spin` loops, on a linear curve: it is the waiting indicator, shown only while the fetch runs. | § 8: *The only two loops. The waiting indicator, for as long as the operation or the wait lasts: a spinning icon, ...*; the `--ease` row: *Only a continuous rotation or a continuous sweep is linear.* |
| D4 | P:171-176 | `.count` is a pill: a counter informs, it cannot be clicked. | § 5: *The pill is reserved for what informs without being clicked.*; § 9.5: *A counter informs: it cannot be clicked.* |
| D5 | P:217-219 | `--faint` on the hint of an empty field. | § 2.1, `--faint`: *Hint in an empty field, disabled element, decorative marker.*; § 9.3: *The hint is in `--faint`* |
| D6 | P:246 | `border-right: 1px solid var(--line-soft)`: a separator inside a card. | § 6: *Inside a card, things are separated by space or by a `--line-soft` line* |
| D7 | P:116 | "HEAD" in capitals: git's term, kept as it is. | § 12, the table of git vocabulary: *Kept as they are, in lower case except HEAD.*; § 3.4: *The abbreviations of git and of the system keep theirs: HEAD, SHA, WIP, SSH, PM.* |
| D8 | P:73, P:84 | The buttons made of an icon alone have a tooltip and an accessible name. | § 7: *A button made of an icon alone has a tooltip and an accessible name (`aria-label`).* |
| D9 | P:220-231 | `.clear` is exactly 24 × 24 px. | § 4.3: *Clickable target, at least*, 24 × 24 px; § 9.1: *a button made of an icon alone is at least 24 × 24 px.* |
| D10 | P:108 | "1 commit", "2 commits": the plural agrees, and "0 commits" would too. | § 12, typography: *plurals agree: "1 commit", "10 commits"* |
| D11 | P:120, P:318-324 | The error has `role="alert"`, in `--bad` on `--bad-soft`, with the 8 px corners of a tinted box. | § 11: *An error is announced at once (`role="alert"`)*; § 2.4: `--bad` on `--bad-soft`, 4.5:1; § 5, `--radius`: *a tinted box inside a card or a dialog* |
| D12 | P:345, P:348 | A border and `--shadow-pop` on the dialog: it floats. | § 6, level 3: menu, dialog, `--raised`, 1 px `--line`, `--shadow-pop`; § 9.10: *Floating: `--raised` background, `--line` border, 12 px corners, `--shadow-pop` shadow.* |

## What the screenshots show (6)

What only the rendering tells, with the measures taken in the page
([`screens/measures.json`](screens/measures.json)). A review that is given the screenshots should
report each of these, under whichever part.

| | Screenshot | What it shows |
|---|---|---|
| E1 | `04-narrow-*` | At 440 px the list is crushed: the summary keeps its 200 px and a row is 175 px wide, the 11 names are cut down to a letter or to nothing, and 7 commit counts wrap onto two lines that spill over the rows around them. |
| E2 | `02-confirmation-*` | The confirmation covers the bottom of the summary and the right end of rows 7 to 10, their SHAs and their commit counts, and nothing is dimmed behind it. The keyboard focus stays outside the box: its ring is on "Delete Selection…". |
| E3 | `01-rest-*`, `04-narrow-*` | The hovered row, the fifth, is not checked and has the same background as the checked rows. |
| E4 | `*-dark` | The summary stands out as a card only in the dark theme: in the light theme `--raised` and `--panel` are both white. |
| E5 | `03-error-*` | The error appears above the footer and pushes it down: "Close" and "Delete Selection…" are 44 px lower than in `01-rest-*` and `02-confirmation-*`. |
| E6 | `03-error-*` | "Retrieving…", in progress, and "Delete Selection…", with nothing selected, are disabled at 60%: they still read as buttons that can be clicked. |

What the screenshots do not show, and a review must not claim:

- At 760 px, only the long name is cut, with an ellipsis, and no count wraps.
- The filter field is 32.2 px high and the rows are 28 px high.
- The small buttons of the header, "Retrieve" and the close button, are 24 px high, as 4.3 asks
  of a small button; the close button is 32 × 24 px. The buttons of the footer are 30.2 px high.
- "Retrieving…" is 39 px wider than "Retrieve" (104.7 px against 65.9 px) and keeps its right
  edge: the counter moves to the left, which 9.1 allows ("a label that changes while it works
  ("Cloning…") may shift what stands to its left").
- The confirmation does not reach the footer: its buttons stay visible.
- The page does not overflow, at either width.

E2 corrects what the code suggests for A5. The code asks for the focus on "Delete", which is the
violation. In the page the focus does not move at all, because Svelte only honors `autofocus`
when nothing else has the focus.

## Left to judgment

Not scored, whichever part they are reported under.

- The filter field has a hint and an accessible name, but no visible label: 9.3 says that "a hint
  inside the empty field never replaces the label", and also describes a filter field by its
  magnifier, its hint and its accessible name, which this one has.
- The filter field has no hover and writes `.field` again with other inner margins (9.3, 14).
- The state marker is a circle (5 reserves the circle for a list of uses, which does not name a
  state dot).
- The "refresh" icon spins in "Retrieving…" although `Icon.svelte` has a "fetch" icon (7, "One
  notion, one icon"), and 9.1 names the waiting ring or a progress bar for a wait that can last,
  where 8 allows a spinning icon.
- The active row is not brought back into range when the filter shortens the list, and is not
  scrolled into view (9.6, 11).
- The buttons of the footer are full size, where 9.1 makes the actions in the foot of a card
  small standard buttons, and they read in the order of a dialog, the way out first; 9.1 says
  that "the line of a panel, a strip or a banner reads the other way", and this panel is a card.
- "Delete Selection…", disabled while nothing is checked, does not say why beside it (9.1, "A
  disabled button whose reason is not plain to see says why"): the help text and the summary
  say it.
- E5 against 9.1, "The button just pressed does not move": the button pressed last, "Delete",
  goes away with its dialog, and the error pushes down the button that opened it.
- The title of the panel, "Merged branches", names what it lists, and its accessible name, "Clean
  up branches", the action (12, "Titles name the action").
