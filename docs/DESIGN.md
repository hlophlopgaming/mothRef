# MothRef design

Visual identity and UI layout agreed with the maintainer on 2026-10-24. This file
is the source of truth for what the redesign looks like; ROADMAP.md says when
each part ships. Mock-ups live in two private Claude Design canvases owned by the
maintainer ("MothRef — UX redesign", screens 1–14, and "MothRef — logo").

---

## 1. Principle

Every action has one home. The board is in Select mode by default; one bottom
toolbar replaces the top toolbar and the left rail; everything about the
selection lives in the selection bar and the context menu; everything about the
app lives in the settings window. Nothing technical is shown unless the user
opens Settings › For developers.

---

## 2. Identity: «Album»

MothRef looks like an artist's sketchbook: paper, graphite and the hand-drawn
moth line. The sage-and-white palette the project started with becomes a system.

### Colors

| Token | Name | HEX | Role |
| --- | --- | --- | --- |
| `paper` | Paper | `#F3EFE4` | Light-theme background and canvas |
| `graphite` | Graphite | `#23241F` | Text, primary button, dark-theme background |
| `sage` | Sage | `#A6C47A` | Accent: selection, active tool, logo underline. Not for text on paper (too little contrast) |
| `moss` | Moss | `#55702F` | Sage for text and thin lines on paper |
| `eraser` | Eraser | `#E9B7A8` | Rare warm accent: labels, calm warnings |

### Dark theme palette (from exploration C, maintainer 2026-10-24)

The dark theme takes its colors from the «Pixel» exploration; its pixel font and
hard-edged styling are not used.

| Token | Name | HEX | Role in dark theme |
| --- | --- | --- | --- |
| `screen` | Screen | `#1E2420` | Background and canvas |
| `ink` | Ink | `#1A1A1A` | Deepest surfaces, shadows, text on sage |
| `bone` | Bone | `#E8E4D6` | Text and icons |
| `sage` | Sage | `#A6C47A` | Accent: selection, active tool (shared with the light theme) |
| `wing` | Wing | `#E58FAE` | Rare warm accent: labels, calm warnings (dark counterpart of Eraser) |

Raised surfaces and borders are derived from `screen` (lighter steps of the same
green-grey). Exact derived shades are set when the theme-token module exists;
every token needs a light and a dark value.

### Type

- **Bricolage Grotesque** — logo and large headings only (OFL).
- **Noto Sans** — the whole interface (already bundled in `crates/moth-ui/fonts`).
- Monospace for numbers and shortcuts: Hack (already bundled).

### Themes

- Light «paper» theme is the default; dark theme in the palette above for night work.
- Selection, focus and the active tool use `sage`; text links and thin accents on
  paper use `moss`.

### Logo and icon

Final mark, app icon and README logo are on hold (maintainer, 2026-10-25).

- **Mark:** the maintainer's hand-drawn moth (big black eyes, ω smile, folded
  paws holding a pencil, feathered antennae) in graphite line. The first round of
  vector redraws was rejected; the final mark is still open (section 6).
- **Wordmark:** "MothRef" in Bricolage Grotesque Bold with a hand-drawn sage
  underline.
- **App icon:** paper tile with the moth face. The existing 64 px pixel moth
  (`mothIcon/`) may stay as the favicon and README badge.

---

## 3. Screens

| # | Screen | Decision | Milestone |
| --- | --- | --- | --- |
| 1 | Board | One floating bottom toolbar: tool group, Insert ▾, undo/redo, fit, zoom "− 25% +". A small "Saved · 9 images" chip replaces the status bar. Minimap with silhouettes | M2 |
| 2 | Adding images | No modal question: arrange with the last method, fit the camera, toast "Added 9 images · Packed ▾ · Undo" | M2 |
| 3 | Selection | One bounding box with four handles; selection bar above it; grouped context menu with right-aligned shortcuts | M2–M3 |
| 4 | Library | One primary "New board" and "Open file…"; stack colors picked from a palette; pin icon on pinned boards and in the sidebar; import tile for `.pur`/`.bee` | M5 |
| 5 | Empty board | "Drop images here", full list of formats, "Choose files…" and "Paste" | M2 |
| 6 | Command palette | Search field focused on open; finds commands, boards and settings rows; grouped results; "/" not "Slash" | M3 |
| 7, 13 | Settings | Own window on Ctrl/⌘+, with icons per section and search; adapter and memory details only here | M2.5–M3 |
| 8 | Drawing | Tool group in the bottom toolbar; a pen strip with icons (pen, marker, highlighter, tape, stamp, eraser), widths, colors, eyedropper; Esc back to Select | M4 |
| 9 | Insert and import | "Insert ▾" menu with icons (images, clipboard, PDF, note, heading, to-do, group, column, nested board, line, link, swatch, palette); banner for opened `.pur` boards with "Save as .moth" | M3–M6 |
| 10 | Selection bar per type | Image, several items, GIF (player on the image itself), note (formatting), group, PDF page, review status | M4–M15 |
| 11 | Compact window and overlay | Window bar slides in on hover, Tab or a corner tap; overlay HUD with opacity, click-through and exit (grab handle dropped, 2026-10-29) | M3 |
| 12, 12b | Drawing sessions | Full-screen session and a mini window over the editor that keeps the current image's aspect ratio while resized | M4 |
| 14 | Groups | Ctrl/⌘+G: backdrop plus an inline title field focused at once; clicking the title selects the whole group; dragging the title moves it; layers in a slide-out drawer | M4 |
| 15 | Window modes | Collect ⇄ View with `Tab`, Over with `Ctrl/⌘+T` | M3 |
| 16 | View | Images only; window bar with a "View" popover for opacity and background | M3 |
| 17 | Over | Glass background, three opacity controls, click-through | M3 |

---

## 4. Interaction decisions

- **Window modes (screens 15–17).** One board, three window modes; a mode changes
  only the chrome and the window, never the content.
  - *Collect* — Miro-like: bottom toolbar, Insert, notes, groups, layers drawer, tabs.
  - *View* — PureRef-like: images only, frameless window, nothing drawn over the
    images. A floating, rounded window bar slides in at the top centre when the
    pointer reaches the top edge (also while a control in it has focus or a menu
    is open): drag grip, board switcher, a "View" popover (image opacity, board
    background solid / translucent / glass, whole-window opacity, "images only"),
    pin ▾, overlay and minimize / maximize / close. Below 640 px the View popover,
    pin choices and overlay fold into "⋯" so the bar stays one row. Touch opens
    it with "Window controls" in the context menu (long press). There is no
    permanent corner button (maintainer, 2026-10-28). Editing works through the
    mouse, the context menu and keys.
  - *Collect* window controls (pin ▾, minimize, maximize/restore, close) fade in
    when the pointer is over the top bar, a control has focus, or after a tap on
    the bar's free space; they keep their place while hidden so nothing shifts.
    The free space drags the window; double-click maximizes. Tabs are rounded
    chips with the close cross inside; "+" adds a board, middle click closes,
    italics mark boards never saved to a file.
  - *Several windows.* Dragging a tab out of the strip opens it in a window of
    its own; the tab menu moves it to a new or another window or gathers all
    tabs. Each window has its own tabs, mode, pin and share of video memory.
    Wayland cannot tell which window a drag was released over, so there the
    way back is the tab menu; Windows, macOS and X11 also accept a drop on
    another window's tab strip.
  - *Over* — window above another app: the same controls as the View popover,
    plus click-through; the separate Over controller is the way back out of
    click-through (the grab handle was dropped, 2026-10-29).
  - Keys: `Tab` toggles Collect ⇄ View (ignored in text fields); `Ctrl/⌘+T`
    toggles Over; clicking a group title then `Tab` views only that group;
    `Ctrl/⌘+Shift+F` resizes the window to the selection. A window narrower than
    about 600 px switches to View by itself; the mode is remembered per board.
  - Opacity is a secondary feature: it lives in the View popover and the Over
    panel, never in its own floating window. Number keys `1`…`0` set the selected
    image's opacity with the Select tool (pen width with the drawing tools);
    `Alt+1`…`0` set the whole window.
  - Glass needs a platform blur capability (capability flag); where the OS cannot
    blur, Glass falls back to a translucent tint and says so.
- **Looking at one image closely.** Implemented and kept: double-click an image
  (or select it and press Space) fits the camera to it, double-click again goes
  back; dragging a corner handle scales the image itself. Today the double-click
  works only with the Select tool (section 5). Planned in M3: "overlay
  selection" pops selected images into their own borderless windows.

- **Groups (M4).** Select images, press Ctrl/⌘+G: a tinted backdrop appears under
  them and a title field opens immediately (Enter keeps it, Esc leaves it
  untitled). Clicking the title selects the whole group and shows group actions
  (enter, ungroup, backdrop color). Dragging the title moves the group; an image
  dropped on the backdrop joins it.
- **Layers (M4).** The hierarchy tree is a specialised tool, so it lives in a
  drawer hidden behind a "Layers" tab on the right edge, not in a permanent panel.
- **GIF (M4).** The player sits on the image: scrubber, play/pause, previous and
  next frame, frame counter, step size 1, 2, 3 or 4 frames, speed. The selection
  bar keeps only "Grab frame", "Frame to new image" and "…".
- **Drawing sessions (M4).** Full screen or a mini window pinned over the
  editor; the mini window resizes from its corner and always keeps the current
  image's aspect ratio, re-flowing for the next image from its top-left corner.
- **Insert menu (M6).** Milanote-style elements are created from "Insert ▾" in the
  bottom toolbar instead of a separate left toolbar.
- **Icons everywhere.** Every menu item, tool and settings section has an icon in
  addition to its label.

---

## 5. Defects found in the 0.1.0-alpha review (2026-10-24)

- "→" and "▸" render as "?" (missing glyphs).
- Command palette: field not focused, window jumps while filtering, "Slash" label,
  no Settings command.
- Empty board says "PNG / JPEG" although WebP, GIF and BMP are supported.
- Minimap silhouettes are retained by maintainer decision on 2026-10-25; zoom buttons read "+ 100% −";
  handles drawn on every selected image.
- The two-item "Commands" menu and "Help" take about 90 px at the top.
- The separate Hand/Pan tool has been removed by maintainer request. Select stays
  active; Space-drag, middle-drag and two-contact touch pan move the camera.

---

## 6. Open questions

- Many images added at once: maintainer confirmed on 2026-10-25 that the default
  remains "ask above 5 images". Automatic arrangement is opt-in; its feedback
  should provide the method and Undo. This supersedes the no-question default
  pictured in screen 2.
- Performance modes: the mock-up shows four; decision 0014 and M2.5 have five.
  Merge "Cache on disk" and "No cache" into "Least memory", or keep all five?
- The Shapes button in the bottom toolbar is not recognisable: labels under the
  tool icons, or a corner mark for "has a submenu"?
- Final moth mark: maintainer redraws the sketch, an illustrator does, or another
  round of exploration.
