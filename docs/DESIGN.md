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

The dark theme is graphite-based (backgrounds derived from `#23241F`, text from
`#F3EFE4`, accent `#A6C47A`). Exact derived shades are set when the theme-token
module exists; every token needs a light and a dark value.

### Type

- **Bricolage Grotesque** — logo and large headings only (OFL).
- **Noto Sans** — the whole interface (already bundled in `crates/moth-ui/fonts`).
- Monospace for numbers and shortcuts: Hack (already bundled).

### Themes

- Light «paper» theme is the default; graphite dark theme for night work.
- Selection, focus and the active tool use `sage`; text links and thin accents on
  paper use `moss`.

### Logo and icon

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
| 1 | Board | One floating bottom toolbar: tool group, Insert ▾, undo/redo, fit, zoom "− 25% +". A small "Saved · 9 images" chip replaces the status bar. Minimap with thumbnails | M2 |
| 2 | Adding images | No modal question: arrange with the last method, fit the camera, toast "Added 9 images · Packed ▾ · Undo" | M2 |
| 3 | Selection | One bounding box with four handles; selection bar above it; grouped context menu with right-aligned shortcuts | M2–M3 |
| 4 | Library | One primary "New board" and "Open file…"; stack colors picked from a palette; pin icon on pinned boards and in the sidebar; import tile for `.pur`/`.bee` | M5 |
| 5 | Empty board | "Drop images here", full list of formats, "Choose files…" and "Paste" | M2 |
| 6 | Command palette | Search field focused on open; finds commands, boards and settings rows; grouped results; "/" not "Slash" | M3 |
| 7, 13 | Settings | Own window on Ctrl/⌘+, with icons per section and search; adapter and memory details only here | M2.5–M3 |
| 8 | Drawing | Tool group in the bottom toolbar; a pen strip with icons (pen, marker, highlighter, tape, stamp, eraser), widths, colors, eyedropper; Esc back to Select | M4 |
| 9 | Insert and import | "Insert ▾" menu with icons (images, clipboard, PDF, note, heading, to-do, group, column, nested board, line, link, swatch, palette); banner for opened `.pur` boards with "Save as .moth" | M3–M6 |
| 10 | Selection bar per type | Image, several items, GIF (player on the image itself), note (formatting), group, PDF page, review status | M4–M15 |
| 11 | Compact window and overlay | Window bar slides in on hover, Tab or a corner tap; overlay HUD with opacity, click-through, exit, and a grab handle that always takes clicks | M3 |
| 12, 12b | Drawing sessions | Full-screen session and a mini window over the editor that keeps the current image's aspect ratio while resized | M4 |
| 14 | Groups | Ctrl/⌘+G: backdrop plus an inline title field focused at once; clicking the title selects the whole group; dragging the title moves it; layers in a slide-out drawer | M4 |

---

## 4. Interaction decisions

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
- Minimap draws grey boxes instead of thumbnails; zoom buttons read "+ 100% −";
  handles drawn on every selected image.
- The two-item "Commands" menu and "Help" take about 90 px at the top.

---

## 6. Open questions

- Many images added at once: the default here is "arrange and show a toast with
  Undo", with "ask above N" as a setting. ROADMAP M2 currently records the prompt
  (`arrange_prompt_threshold`, default 5) as done — keep or change the default?
- Performance modes: the mock-up shows four; decision 0014 and M2.5 have five.
  Merge "Cache on disk" and "No cache" into "Least memory", or keep all five?
- The Shapes button in the bottom toolbar is not recognisable: labels under the
  tool icons, or a corner mark for "has a submenu"?
- Final moth mark: maintainer redraws the sketch, an illustrator does, or another
  round of exploration.
