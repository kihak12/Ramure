# Ramure design guidelines

The reference for Ramure's design. It sets the rules of the chosen direction, **Clairière**. How
that direction was chosen is told in [`design/directions.md`](design/directions.md).

- **Status:** version 1, under review.
- **Scope:** the whole interface, Git and SSH modules.
- **Ground rule:** when the code and the guidelines disagree, the guidelines are right. A rule
  changes in the guidelines first, through a pull request, and in the code second.

The tokens in the code follow these guidelines (issue #5) and the interface has the shape of
Clairière (issue #10). The screen-by-screen review, rule by rule, is issue #6.

The interface is in French, so the labels quoted in this document are in French.

| Dark | Light |
|---|---|
| ![Clairière, dark theme](design/directions/clairiere-graphe-sombre.png) | ![Clairière, light theme](design/directions/clairiere-graphe-clair.png) |

## Contents

1. [Principles](#1-principles)
2. [Colours](#2-colours)
3. [Typography](#3-typography)
4. [Spacing and dimensions](#4-spacing-and-dimensions)
5. [Shapes](#5-shapes)
6. [Surfaces and shadows](#6-surfaces-and-shadows)
7. [Icons](#7-icons)
8. [Motion](#8-motion)
9. [Components](#9-components)
10. [Commit graph](#10-commit-graph)
11. [Accessibility](#11-accessibility)
12. [Tone of the interface text](#12-tone-of-the-interface-text)
13. [Logo](#13-logo)
14. [Changing the guidelines](#14-changing-the-guidelines)

## 1. Principles

What the interface favours:

1. **Content first.** The graph, the diff and the terminal fill the screen. Everything else
   steps back: no decoration on working screens.
2. **Colour means something.** A bright colour stands for a branch, a state or the main action.
   Nothing else.
3. **Surfaces rather than lines.** Areas are separated by their background and by space. Lines
   are only used inside a card.
4. **One shape, one role.** A slightly rounded rectangle can be clicked. A pill or a circle
   informs.
5. **Dense and readable.** A lot of information on screen, but no text under 10.5 px and no
   target under 24 px.
6. **Calm.** A movement is brief and explains a change. Nothing moves without an action from the
   user.

What it forbids:

- a colour hard-coded in a component;
- a border or a shadow around a card;
- a coloured bar on the edge of a card or of a row, to mark a selection or a state;
- a pill for an element that can be clicked;
- an element that changes size when it is clicked, other than a button;
- a decorative gradient;
- text in all capitals;
- a looping animation, apart from the two cases of [section 8](#8-motion).

## 2. Colours

Every colour goes through a token, defined in `src/app.css`. The values below are those of the
default theme, **Ardoise**.

This holds for every token in these guidelines: colours, text sizes and weights, radii and
durations. `src/app.css` is the only file that writes those values, and `npm run check` rejects a
component that hard-codes one.

### 2.1 Tokens

| Token | Role | Light | Dark |
|---|---|---|---|
| `--side` | Window background: rail, tabs, toolbar, status bar, space between the cards. | `#eaecf1` | `#0c0d11` |
| `--panel` | Background of the cards and of the active tab. | `#ffffff` | `#181b21` |
| `--bg` | Recessed background, inside a card: input fields. | `#f6f7f9` | `#111318` |
| `--raised` | Background of buttons and of what floats: menus, dialogs, notifications. | `#ffffff` | `#20242c` |
| `--line` | Border of buttons, fields and floating elements. | `#d9dce3` | `#2b303a` |
| `--line-soft` | Discreet separator inside a card. | `#e7e9ee` | `#21252d` |
| `--ink` | Body text. | `#171a21` | `#e6e9ef` |
| `--muted` | Secondary text: captions, metadata, labels. | `#5c6370` | `#959ca9` |
| `--faint` | Hint in an empty field, disabled element, decorative marker. | `#8f95a1` | `#676e7b` |
| `--hover` | Background of a hovered element. | `#eef0f4` | `#1e222a` |
| `--sel` | Background of a selected element. | `#e1ecfb` | `#1d3252` |
| `--accent` | Main action, focus, active element, link. | `#2b6fdb` | `#6aa5f8` |
| `--accent-soft` | Accent-tinted background: hovered menu item, active module in the rail, pressed view button of the toolbar. | `#e4eefc` | `#182b45` |
| `--on-accent` | Text placed on the accent or on a branch colour. | `#ffffff` | `#08131f` |
| `--ok` / `--ok-soft` | Success, added line. | `#1f7a45` / `#e2f3e8` | `#5cc48d` / `#12301f` |
| `--warn` / `--warn-soft` | Warning, pending operation. | `#8f5708` / `#fbf0dc` | `#f0a94f` / `#3a2b14` |
| `--bad` / `--bad-soft` | Error, destructive action, removed line. | `#b42318` / `#fbe5e2` | `#f2776d` / `#3a1715` |
| `--term-bg` / `--term-ink` | Background and text of the terminal. | `#fbfbfc` / `#1c1f27` | `#0b0c10` / `#d5dae3` |
| `--scrim` | Veil behind a dialog. | `rgb(18 22 30 / 0.32)` | `rgb(0 0 0 / 0.55)` |

### 2.2 Branch colours

Eight colours, reserved for the graph. A branch takes the colour of its column: column 0 for
`--l0`, column 1 for `--l1`, and so on, starting again at `--l0` after `--l7`.

| Token | Light | Dark |
|---|---|---|
| `--l0` | `#2b6fdb` | `#6aa5f8` |
| `--l1` | `#c2410c` | `#f2844e` |
| `--l2` | `#1b7f53` | `#55c68b` |
| `--l3` | `#8447c7` | `#b98aeb` |
| `--l4` | `#8a6500` | `#e8be45` |
| `--l5` | `#c33a6f` | `#ee79a2` |
| `--l6` | `#0f808a` | `#3fc4cc` |
| `--l7` | `#5d6b17` | `#b0c25a` |

- `--l0` is the accent colour: the first column and the accent echo each other.
- Outside the graph, these colours are only used for a marker chosen by the user, such as the
  colour of an SSH host.

### 2.3 Rules of use

- No colour is hard-coded in a component. An intermediate shade is made with `color-mix()`
  between two tokens.
- **The accent** stands for one thing at a time: the main button of a view or of a dialog, the
  focus, the active element. Never two main buttons side by side.
- **State colours** (`--ok`, `--warn`, `--bad`) only say the state. They do not decorate.
- **`--faint`** never carries information that is needed: its contrast is too low for text that
  has to be read.
- A state-tinted background uses the `-soft` token, or 13 to 15% of the colour over the
  background.

### 2.4 Minimum contrasts

Every theme must meet them, in light and in dark.

| Pair | Minimum |
|---|---|
| `--ink` on `--panel`, `--bg`, `--sel`, `--hover` | 7:1 |
| `--ink` in a message capsule or a branch label | 7:1 |
| `--muted` on `--panel`, `--side`, `--sel` | 4.5:1 |
| `--on-accent` on `--accent` and on each branch colour | 4.5:1 |
| `--ok`, `--warn`, `--bad` on `--panel` and `--bg` | 4.5:1 |
| `--accent` and each branch colour on `--panel` | 3:1 |

### 2.5 Colour themes

- The application follows the system's light or dark setting.
- A colour theme redefines the tokens of sections 2.1 and 2.2, in light and in dark, and nothing
  else: no shape, no font, no size, no shadow. It does not change the colours of the logo either
  (section 13).
- Four themes are planned: Ardoise (the default), Encre, Sous-bois and Braise. Their values are
  in [`design/themes.css`](design/themes.css); implementing them is issue #9.

## 3. Typography

### 3.1 Fonts

| Use | Font | Token |
|---|---|---|
| Interface | DM Sans (variable) | `--f-ui` |
| Code | DM Mono | `--f-mono` |

- Both fonts are embedded in the application. No font is loaded from the network.
- **The code font** is for what gets copied or typed in a terminal: SHAs, git commands, file
  content (diff, conflict editor), the terminal, the name of the current branch in the toolbar.
- In a list of files, the path is in the interface font: the folder in `--muted`, the file name
  in `--ink`.
- DM Mono stops at weight 500. Emphasised code text is rendered at 500; the browser does not
  synthesise a fake bold.

### 3.2 Sizes

| Token | Size | Use |
|---|---|---|
| `--fs-micro` | 10.5 px | Counters, badges, shortcut keys. |
| `--fs-small` | 11.5 px | Captions, toolbar labels, status bar, graph labels, metadata. |
| `--fs-label` | 12 px | Field labels, column headers, section titles, secondary columns of a list (author, date). |
| `--fs-body` | 13.5 px | Text, list rows, buttons, menus. |
| `--fs-title` | 15 px | Title of a dialog or of a panel, input of the command palette. |
| `--fs-display` | 24 px | Start page only. |
| `--fs-code` | 12 px | Diff, conflict editor, SHAs. |
| `--fs-term` | 13 px | Terminal: a whole number, so that the character grid stays sharp. |

- No other size. No text under 10.5 px.
- Line height: 1.35 for the interface.

### 3.3 Weights

Two weights, no more.

| Token | Weight | Use |
|---|---|---|
| `--fw-regular` | 400 | Body text. |
| `--fw-strong` | 650 | Titles, labels, main button, current element (active tab, current branch), label of a local branch. |

### 3.4 Rules

- A capital at the start of a sentence, and that is all: no title in capitals, no spaced
  capitals.
- Numbers that line up or that change (counters, dates, columns) use tabular figures
  (`font-variant-numeric: tabular-nums`).
- Text that is too long is cut on the right with an ellipsis, and its full value appears in a
  tooltip. The title of a notification is the one exception (section 9.11).

## 4. Spacing and dimensions

### 4.1 Scale

Margins and gaps take their values from this scale: **2, 4, 6, 8, 10, 12, 16, 20, 24 px**.

| Gap | Value |
|---|---|
| Between two cards, and between the cards and the edge of the window | 8 px |
| Inner margin of a card | 12 px |
| Between an icon and its text | 6 px |
| Between two buttons | 8 px |
| Between a label and its field | 6 px |
| Between two fields of a form | 12 px |

### 4.2 Screen structure

From left to right and from top to bottom:

| Area | Dimension | Background |
|---|---|---|
| Module rail (Git, SSH) | 48 px wide | `--side` |
| Tab bar | tabs 30 px high | `--side` |
| Toolbar | full width | `--side` |
| Three cards: sidebar, centre, detail | 8 px apart; the two side cards can be resized (9.16) and hidden (9.17) | `--panel` |
| Status bar | 26 px high | `--side` |

- No line separates these areas: the background and the space are enough.
- The window is at least 1000 × 600 px; no area may overflow at that size.
- When a side card is hidden, the gap of its handle goes with it: the centre card sits 8 px from
  the edge of the window.

### 4.3 Heights

| Element | Height |
|---|---|
| Graph row | 34 px |
| Sidebar row | 28 px |
| Row of a file list | 24 px |
| Button | 30 px; 24 px for a small button |
| Field | 32 px |
| Branch label | 22 px |
| Clickable target, at least | 24 × 24 px |

## 5. Shapes

| Token | Radius | For |
|---|---|---|
| `--radius-xs` | 4 px | Small markers: shortcut key, file status letter, highlight inside a diff line. |
| `--radius-s` | 6 px | What can be clicked: buttons, fields, branch and tag labels, menu items, checkboxes. |
| `--radius` | 8 px | Selectable rows, tabs, notifications, rail buttons. |
| `--radius-l` | 12 px | What floats: dialogs, menus, command palette. |
| `--radius-card` | 14 px | Cards. |
| `--radius-pill` | 999 px | What informs: counters, the capsule of a commit message. |

- **Everything that can be clicked is a slightly rounded rectangle.** The pill is reserved for
  what informs without being clicked.
- The circle is reserved for the dot of a commit, the badge of a toolbar button, the icon of a
  notification and the waiting ring.
- An element placed inside another never has a larger radius than the one that contains it.

## 6. Surfaces and shadows

The interface has four levels. Each is recognised by its background.

| Level | Element | Background | Border | Shadow |
|---|---|---|---|---|
| 0 | Window | `--side` | none | none |
| 1 | Card | `--panel` | none | none |
| 2 | Button, field (inside a card) | `--raised`, `--bg` | 1 px `--line` | `--shadow-soft` for a button |
| 3 | Menu, dialog, notification, palette | `--raised` | 1 px `--line` | `--shadow-pop` |

| Token | Light | Dark |
|---|---|---|
| `--shadow-soft` | `0 1px 2px rgb(18 22 30 / 0.06)` | `0 1px 2px rgb(0 0 0 / 0.3)` |
| `--shadow-pop` | `0 18px 44px -14px rgb(18 22 30 / 0.3), 0 2px 6px rgb(18 22 30 / 0.08)` | `0 22px 50px -14px rgb(0 0 0 / 0.7), 0 2px 8px rgb(0 0 0 / 0.4)` |

- A shadow says "this floats". A card never has one.
- No card inside a card. Inside a card, things are separated by space or by a `--line-soft`
  line.
- Shadows are neutral: they do not take the tint of the theme.
- A dialog sits on the `--scrim` veil, with a 2 px blur.

## 7. Icons

- One set only, the application's own (`src/lib/ui/Icon.svelte`). No emoji, no icon from
  another library.
- Drawing: a 16 × 16 grid, a 1.4 stroke with no fill, rounded ends and corners, the colour of
  the text (`currentColor`).

| Size | Use |
|---|---|
| 14 px | In a label, a counter or a dense row. |
| 16 px | By default: menus, buttons, lists. |
| 18 px | Toolbar and rail. |

- One notion, one icon, everywhere: the computer for "local", the cloud for "remote", the tag
  for a tag.
- A button made of an icon alone has a tooltip and an accessible name (`aria-label`). An icon
  that accompanies text is hidden from screen readers (`aria-hidden`).
- An icon never carries a state on its own: it is backed by text or by a tooltip.

## 8. Motion

| Token | Value | Use |
|---|---|---|
| `--ease` | `cubic-bezier(0.2, 0.8, 0.2, 1)` | The only curve in the application. Only a continuous rotation or a continuous sweep is linear. |
| `--t-fast` | 110 ms | Hover, press, focus, colour change, disappearance. |
| `--t-med` | 200 ms | Appearance of a menu or a dialog, folding of a section. |
| `--t-slow` | 340 ms | Long move: the rail marker, reordering of a list. |
| `--t-step` | 14 ms | Delay between two elements of a staggered entrance. |
| `--t-spin` | 800 ms | One turn of the loading indicator. |
| `--t-wait` | 1.1 s | One cycle of a waiting bar or dot. |
| `--t-halo` | 2.4 s | One cycle of the halo of the current commit. |

Transitions written in script read the same tokens, through `src/lib/ui/motion.ts`.

**What is animated.** Opacity, position (`transform`) and colours. An appearance comes from
close by: an offset of 6 to 10 px at most.

**The press effect.** A button shrinks to 97% while it is clicked. This applies to buttons,
toolbar buttons and rail buttons, and to nothing else.

**What is not animated.**

- Branch and tag labels, list rows, tabs and menu items never change size, neither on click nor
  on hover.
- The scrolling of the graph, of the diff and of the terminal.
- The move of the selection from the keyboard: it is immediate.
- The width of a card that is hidden or shown: the graph would be laid out again at every frame.
- The text of a diff or of a terminal.

**The only two loops.** The waiting indicator, for as long as the operation or the wait lasts: a
spinning icon, a spinning ring, a bar, a blinking dot. The halo of the current commit in the
graph: a ring that widens and fades, in 2.4 s.

The waiting ring is 12 px, in `--accent` or in the colour of the text around it, and it appears
where the user asked for the operation: on the tab of the repository, on the label or the row of
the branch being switched to, in the status bar. It is the shared class `.spinner` of
`src/app.css`, whose `--spinner` property gives it another colour. Words always say the state
beside it, so the ring itself is hidden from screen readers.

**When a repository is opened** or after a full reload, the rows of the graph come in one after
the other, once. Never on scroll.

**Reduced motion.** When the system asks for less animation (`prefers-reduced-motion`),
durations drop to 1 ms and loops stop. No information may depend on an animation.

## 9. Components

States are the same everywhere:

| State | Rendering |
|---|---|
| Hover | `--hover` background, in `--t-fast`. |
| Keyboard focus | A 2 px outline in `--accent`, offset by 1 px. See the exceptions for fields and lists. |
| Selection | `--sel` background. |
| Disabled | 45% opacity, "not allowed" cursor, no reaction on hover. |

### 9.1 Buttons

| Variant | Rendering | Use |
|---|---|---|
| Standard | `--raised` background, `--line` border, `--shadow-soft` shadow. | Ordinary action. |
| Primary | `--accent` background, `--on-accent` text, weight 650. | The expected action. Only one per view or per dialog. |
| Danger | `--bad` background, `--on-accent` text, weight 650. | Confirming a destructive action, in a dialog. |
| Ghost | No background and no border at rest. | Secondary action, in a header or a row. |
| Small | 24 px high, 12 px text. | In a card header or a row. |

- 6 px corners. Inner margins: 5 px at the top and bottom, 12 px on the sides.
- Hover: `--hover` background and a stronger border; a primary or danger button gets 10%
  lighter.
- Press: shrinks to 97%.
- The label is a verb in the infinitive, with its object when the context does not give it:
  "Créer la branche" (create the branch), not "OK".
- A disabled button whose reason is not plain to see says why: a sentence in `--muted` at
  11.5 px next to it, repeated in its tooltip. If the user tries the action all the same, the
  sentence turns to `--warn` and takes the warning icon.

### 9.2 Toolbar

- Each action is an 18 px icon above an 11.5 px label, at least 58 px wide.
- At rest, no background and no border: the button sits on the window background. On hover, a
  `--hover` background and 6 px corners.
- A counter (commits to pull, to push) is a round badge in `--accent`, at the top right of the
  icon.
- During an operation, the icon spins and the button is disabled.
- The repository and the current branch are on the left, the branch in the code font. Each has
  a fixed width, 140 px and 200 px, so that the actions keep their place whatever the names. The
  two widths only yield to a narrow window, down to 70 px. The full name is always in a tooltip.
- Two view buttons close the toolbar, one at each end, each above the card it shows and hides:
  the branch list on the left, the detail panel on the right (9.17). They are an 18 px icon
  alone, in a 30 × 30 px button.
  - Pressed means that the card is shown: an `--accent-soft` background and the icon in
    `--accent`, like the active module in the rail. Released, the icon is in `--muted`.
  - The tint is not alone to say the state: the card itself is there or not, right under the
    button, the tooltip names what a click will do, and `aria-pressed` says it to screen readers.
  - Hover: a `--hover` background and the icon in `--ink`; on a pressed button, the tint gets a
    little stronger (12% of `--accent` over `--accent-soft`). Disabled, when no repository is
    open: 45%.

### 9.3 Fields

- `--bg` background, `--line` border, 6 px corners, inner margins of 6 px and 9 px.
- Hover: a stronger border.
- Focus: an `--accent` border and a 3 px halo in `--accent` at 25%. No outline on top of it.
- Error: a `--bad` border, and an 11.5 px message in `--bad` under the field.
- The label is above the field, in 12 px, weight 650, colour `--muted`. A hint inside the empty
  field never replaces the label.
- A checkbox is 15 px, checked in `--accent`, and its text can be clicked too.

### 9.4 Branch and tag labels

The labels of the "Branche / Tag" column of the graph. They are buttons, and have the shape of
buttons.

| Type | Rendering |
|---|---|
| Local branch | Background: 24% of the branch colour over `--panel`. Border: the same colour at 55%. `--ink` text, weight 650. "Computer" icon. |
| Current branch | Solid background in the branch colour, surrounded by a ring of the same colour. `--on-accent` text, weight 650, preceded by a check mark. |
| Remote branch | The same background as a local branch. Dashed border in the branch colour, weight 400. "Cloud" icon. |
| Tag | The same background as a local branch. Solid border in the branch colour, weight 400. "Tag" icon. |

- A rectangle with 6 px corners, 22 px high, 11.5 px text. **Never a pill.**
- Local branches, remote branches and tags share one background. The icon, the border and the
  weight tell them apart, never the tint.
- **A label never changes size**: not on click, not on hover, not when a branch is dragged onto
  it.
- Hover: the tint of the background gets stronger, 30% at most: beyond that, `--ink` falls
  under 7:1 in the dark theme. The solid background of the current branch moves 10% towards
  `--ink`, darker in light and lighter in dark, so that its text gains contrast in both themes.
- The icons of a label are 14 px.
- Drop target: a 2 px outline in `--accent`, offset by 2 px.
- A local branch and its remote branch on the same commit share one label, with both icons.
- When a commit carries several labels, the first is shown and the others are counted in a
  "+N" pill.
- Double-click: switch to the branch. Right-click: the branch menu.
- During the switch, the waiting ring takes the place of the icon of the label, centred in the
  same 14 px box.

### 9.5 Counters

- A pill on a `--sel` background, 10.5 px text in tabular figures.
- A counter informs: it cannot be clicked.

### 9.6 Rows and lists

Sidebar, file lists, graph, palette results.

- Hover: a rounded `--hover` background under the row, 8 px corners.
- Selection: the same background, in `--sel`. In the graph, it is inset 6 px from the edges of
  the card.
- **No coloured bar on the edge** to mark the selection.
- Keyboard focus: a 1.5 px inner ring in `--accent` on the selected row. **Never an outline
  around the whole list.** Focus stays on the list, which handles the arrow keys; if no row is
  selected when it receives focus, the first one becomes selected.
- The actions of a row (stage, discard) appear on hover and on focus, on the right.
- The state of a file is a letter on a tinted background (M, A, D, R), never a colour alone.
- A file that is only partly staged carries the words "en partie" after its name, in `--muted`
  at 11.5 px, in both lists.

### 9.7 Tabs

- The active tab is a small card: `--panel` background, 8 px corners, no frame and no line.
- An inactive tab is `--muted` text on the window background; on hover, a `--hover` background.
- 30 px high, at least 90 px wide. The name is cut with an ellipsis.
- The close cross is visible on the active tab, and on hover on the others.
- The "+" button follows the last tab.

### 9.8 Cards

- `--panel` background, 14 px corners, no border and no shadow.
- A card scrolls inside itself; its content never goes past its corners.
- The title of a card or of a section is in 12 px, weight 650.
- A column header is separated from the content by a `--line-soft` line.

### 9.9 Context menus

- Floating: `--raised` background, `--line` border, 12 px corners, `--shadow-pop` shadow.
  Between 220 and 380 px wide.
- An item: a 16 px icon in `--muted` in an 18 px column, then the text. 6 px corners.
- Hover and focus: `--accent-soft` background.
- A destructive action is in `--bad`, last, after a separator. Its hover is in `--bad-soft`.
- A label ends with "…" when the action opens a dialog.
- Keyboard: arrows to move, Enter to choose, Escape to close.
- Appears in `--t-fast`, from the point of the click.

### 9.10 Dialogs

- To confirm a destructive action, to enter a few values, or to offer a way through when an
  action would be refused ("Remiser et basculer"). Never for plain information: that is the role
  of a notification.
- Floating: `--raised` background, `--line` border, 12 px corners, `--shadow-pop` shadow, 460 px
  wide at most.
- A 15 px title, weight 650: the name of the action ("Nouvelle branche", new branch).
  Explanatory text in `--muted`.
- A warning is a box on a `--warn-soft` background, with an icon.
- The buttons are at the bottom right: "Annuler" (cancel) first, the action second.
- Enter confirms, Escape cancels. A click on the veil cancels too.
- On opening, focus goes to the first field. For a destructive action, it goes to "Annuler".
- A destructive action says what will be lost, and whether it can be recovered.

### 9.11 Notifications

- At the bottom, in the centre of the window, 40 px from the edge. Five at most, stacked.
  480 px wide at most.
- Floating: `--raised` background, `--line` border, 8 px corners, `--shadow-pop` shadow.
- The state is carried by an icon in a tinted circle (success, warning, error), not by a
  coloured bar on the edge.
- A short title, then the detail. The output of git is shown as it is, in the code font at
  11.5 px.
- In the title, a word of more than 36 characters, such as the name of a branch or a path, is
  cut inside: its first 24 characters and its last 10 stay, around an ellipsis. The full title
  is in a tooltip, and it is what a screen reader reads. This is the one place where a text is
  not cut on the right. A title that is still too long runs over a second line.

| Type | Duration |
|---|---|
| Success | 4.5 s |
| Warning | 9 s |
| Error | Stays until it is closed. |

### 9.12 Command palette

- Floating, centred, 11% from the top of the window, 640 px wide at most.
- A 15 px input. Results as rows (9.6), with the shortcut of each command on the right.
- A shortcut key is drawn like a key: 10.5 px, 4 px corners, a thicker bottom edge.

### 9.13 Status bar

- 26 px high, 11.5 px text in `--muted`, on the window background.
- On the left, the branch and the last git command, in the code font. On the right, the
  timings.
- During an operation, the right side starts with the waiting ring and a few words:
  "Basculement sur main…", or "Opération en cours…".

### 9.14 Diff

- Code font, 12 px. Line numbers in `--muted`.
- Added line: a background at 14% of `--ok`, a "+" sign in `--ok`. Removed line: a background at
  13% of `--bad`, a "−" sign in `--bad`.
- The sign is always there: colour alone never says whether a line is added or removed.
- A line picked to be staged on its own has a stronger tint and its sign in bold. No coloured
  bar on its edge.
- From the keyboard, the lines behave like a list (9.6): ↑ and ↓ move from one changed line to
  the next, Home and End go to the first and to the last, with the inner ring on the line; Space
  picks the line, or lets go of it; Shift with any of these keys extends the choice; Enter
  stages what is picked, or the whole hunk when nothing is. A line of the diff is a band across
  the card: its ring has square corners.
- When only one half of a modified line is picked, the removed line or the added one, a warning
  says so under the lines: a box on a `--warn-soft` background, with the warning icon and a
  button that picks the other half. It sits under the lines so that they do not move while they
  are being picked. Two lines count as one modified line when they look alike; it holds for both
  layouts, and when lines are taken out of the index.

### 9.15 Terminal

- `--term-bg` background, `--term-ink` text, code font at 13 px.
- Terminal tabs follow the rule for tabs (9.7).

### 9.16 Resize handle

Between two cards whose width can be set: in the view of a repository, on either side of the
centre card.

- The handle takes up the 8 px gap between the cards. At rest it cannot be seen: the window
  background already separates the cards.
- Hover: a 2 px vertical marker in `--faint`, and the resize cursor.
- During the drag and on focus: the same marker, in `--accent`. No frame around the handle.
- Keyboard: left and right arrows in steps of 16 px, Home and End for the minimum and maximum
  width, Enter for the default width.
- Double-click: the default width.
- Each card has a minimum and a maximum width, and the centre card always keeps 360 px. The
  chosen widths are restored at the next launch.
- The handle is only 8 px wide: it is the one exception to the 24 px target, and the keyboard
  makes up for it.
- A hidden card has no handle (9.17).

### 9.17 Hiding a side card

In the view of a repository, the branch list and the detail panel can each be hidden, to read
the graph on the whole width.

- Three ways, always: the view button of the toolbar (9.2), a command of the palette, and a
  shortcut, Ctrl + Shift + B for the branch list and Ctrl + Shift + E for the detail panel. A
  command of the palette hides or shows both at once.
- The tooltip of a view button names what a click does, with the shortcut: "Masquer la liste des
  branches (Ctrl + Maj + B)". Its accessible name does not change: "Liste des branches",
  "Panneau de détail".
- Hiding is immediate, and so is showing (8). The card comes back at the width it had.
- **Hiding is the user's choice: only a request to see the detail undoes it.** The detail panel
  comes back on "Voir les modifications en cours", on "Voir les fichiers" in the banner of a
  stopped operation, on a click on the "WIP" row, and on Enter or a double-click on a commit. It does not come back when the selection moves, by a click or from
  the keyboard, nor when an operation stops on its own. Nothing brings the branch list back but
  its own commands.
- The choice is shared by all the tabs and restored at the next launch, like the widths.
- What a hidden card holds is out of the tab order and out of the reach of screen readers. If
  the focus was in the card, it moves to the graph.

## 10. Commit graph

### 10.1 Columns

In order: Branche / Tag (220 px), Graphe (420 px at most), Message, Auteur, Date, and SHA when
the width allows it.

The message comes first when room is short. When it would be less than about 200 px wide, the
"Branche / Tag" column gives its width back to it, down to 164 px.

### 10.2 Drawing

| Parameter | Value |
|---|---|
| Height of a row | 34 px |
| Width of a branch column | 24 px |
| Radius of a commit | 10 px |
| Radius of a merge commit | 5 px |
| Thickness of a line | 3 px |
| Band behind a commit | 24 px high, 15% of the branch colour |

- **A commit** is a round dot in the colour of its branch, with the initial of its author in
  `--on-accent`. A ring in the colour of the card sets it apart from the lines.
- **A merge commit** is a smaller point, with no initial.
- **Lines** are straight as long as the branch keeps its column. A change of column is a soft
  curve, with no corner.
- **The message** sits in a capsule: a background at 15% of the branch colour, rounded like a
  pill on the right, as wide as the text. A band of the same tint links it to the dot.
- **A label** is linked to its commit by a thin line in the branch colour.
- **The current commit** carries a halo (section 8), and the label of its branch is solid.
- **Changes in progress** take the first row: an empty circle, the words `// WIP` in the code
  font inside a dashed frame, and the number of changed files.

### 10.3 Rules

- The colour of a commit, of its line, of its capsule and of its label is the same.
- Hover and selection follow the rule for rows (9.6).
- The graph is not animated on scroll.
- No text on the lines, no gradient, no more than eight colours.
- The "Classique" style, offered in the settings, keeps points and more compact curves. It
  follows the same colours and the same selection rules.

## 11. Accessibility

- **Contrasts.** Those of section 2.4, checked in light and in dark.
- **Visible focus.** Every element that takes keyboard focus shows it. An outline is never
  removed without another marker replacing it.
- **Keyboard.** Every action is possible without a mouse. The tab order follows the visual
  order. Escape closes the topmost floating element.
- **Targets.** 24 × 24 px at least.
- **Never colour alone.** A state is backed by an icon, a sign or text: "+" and "−" in a diff, a
  letter for the state of a file, an icon for local and remote.
- **Reduced motion.** See section 8.
- **Screen readers.** An icon on its own has an accessible name. An error is announced at once
  (`role="alert"`), other notifications without interrupting (`role="status"`). Menus and the
  palette declare their role.
- **Both themes.** A screen is not finished until it has been checked in light and in dark.

## 12. Tone of the interface text

The interface is written in French. These rules apply to that French text.

- **Address the user as "tu"**, in the imperative, in short sentences: "Indexe les fichiers à
  inclure dans le commit."
- **Buttons** are verbs in the infinitive: "Créer la branche", "Tout indexer". Never "OK" or
  "Oui".
- **Titles** name the action: "Nouvelle branche".
- **An error** says what happened, then what to do. The output of git follows, as it is. No
  "Oups", no exclamation mark, no blame.
- **A destructive action** says what will be lost, and whether it can be recovered.
- **The git commands** that are run are always shown as they are, in the status bar and in the
  command log.

Git vocabulary is translated when a French word exists and is understood. The English term is
recalled in brackets where it helps.

| Git | In the interface |
|---|---|
| fetch | Récupérer |
| pull | Tirer |
| push | Pousser |
| stash, stash pop | Remiser, dépiler; une remise |
| stage, unstage | Indexer, retirer |
| merge | Fusionner |
| rebase | Rebaser |
| checkout, switch | Basculer |
| remote | Distant |
| revert | Annuler (revert) |
| commit, tag, cherry-pick, reset, HEAD | Kept as they are. |

French typography:

- French quotation marks « », a curly apostrophe ’, an ellipsis as a single character …;
- a non-breaking space before ":", ";", "?" and "!";
- plurals agree: "1 commit", "10 commits";
- a short date is written "9 nov. 2026", a full date "lundi 9 novembre 2026 à 23:35".

## 13. Logo

Ramure's logo is a deer's head drawn in a single stroke. The muzzle is a commit dot; two lines
leave it, run along the head, spread apart and become the antlers. It shows a deer, and a commit
from which two branches start. How it was chosen is told in [`design/logo.md`](design/logo.md).

![The logo, the icon, the symbol and their sizes](logo/planche.png)

### 13.1 Versions

| Version | File | Use |
|---|---|---|
| Icon | [`logo/ramure-icone.svg`](logo/ramure-icone.svg) | The application: taskbar, window, start page. The symbol in white on a blue tile. |
| Icon, small sizes | [`logo/ramure-icone-petit.svg`](logo/ramure-icone-petit.svg) | The same, under 40 px. The drawing is simplified: one tine per side, a thicker stroke. |
| Logo | [`logo/ramure-logo.svg`](logo/ramure-logo.svg), [`logo/ramure-logo-sombre.svg`](logo/ramure-logo-sombre.svg) | The icon followed by the name. The "sombre" version has the name in a light colour, for a dark background. |
| Symbol | [`logo/ramure-symbole.svg`](logo/ramure-symbole.svg), [`logo/ramure-symbole-petit.svg`](logo/ramure-symbole-petit.svg) | The drawing alone, in a single colour, when the tile does not fit. |

### 13.2 Colours

| Token | Value | Role |
|---|---|---|
| `--brand` | `#2b6fdb` | The tile. |
| `--on-brand` | `#ffffff` | The symbol on the tile. |

- These two colours are fixed: they change neither in the dark theme nor with the colour theme.
  The icon is the same everywhere.
- The name is in `#171a21` on a light background, in `#e6e9ef` on a dark one.
- The symbol alone takes the colour of the text, the blue of the tile, or white. No other.

### 13.3 Construction

- **The tile** is a square whose corners are rounded by 22% of its side. The symbol is centred
  in it, with a margin of 16% of the side (12% for the simplified drawing).
- **The name** is "Ramure", in DM Sans, weight 650, with a tracking of −0.01 em. Its cap height
  is 45% of the height of the tile, and it is centred on the tile.
- **The gap** between the tile and the name is a quarter of the height of the tile.
- In the files, the name is converted to outlines: it displays without the font being installed.

### 13.4 Sizes and clear space

- The full drawing from 40 px up; the simplified drawing from 16 to 39 px; nothing under 16 px.
- The logo with the name is not used under 40 px high.
- Around the logo, a margin of at least a quarter of the height of the tile stays empty.

### 13.5 What is forbidden

- Distorting the logo, slanting it or rotating it.
- Changing the colour of the tile or of the symbol; adding a gradient, a shadow or an outline.
- Redrawing the symbol: adding or removing tines, changing the thickness of the stroke.
- Using the full drawing under 40 px.
- Placing the symbol alone on a photograph or a busy background: that is what the tile is for.
- Writing the name in another font, or changing its gap with the tile.

### 13.6 In the code

- `src/lib/ui/Logo.svelte` displays the symbol. It reads the files in `docs/logo`, which are the
  only source of the drawing.
- The icons in `src-tauri/icons` are derived from the same files by
  `scripts/generer-icones.cjs`. That script only needs to be run again if the drawing changes.

## 14. Changing the guidelines

- A rule changes through a pull request that edits this file. The code follows, in the same pull
  request or in the next one.
- Before creating a component, look for the one that already exists. If a new one is needed, it
  goes into section 9 with its states.
- A new token goes into the relevant section, with its role and its two values, then into
  `src/app.css`.
- `npm run check` rejects a colour, a text size, a weight, a radius or a duration hard-coded in
  a component.
- Every pull request that touches the interface is reviewed against these guidelines.
