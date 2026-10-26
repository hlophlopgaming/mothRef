# v0.1.0-alpha.9 review — findings and fix plan

Hands-on review of the `mothref-linux-x86_64` build of
[v0.1.0-alpha.9](https://github.com/hlophlopgaming/mothRef/releases/tag/v0.1.0-alpha.9)
on 2026-10-24: Xvfb display, `--software` (llvmpipe), no window manager, input
driven through XTest. Covered: library, settings, light theme, adding 12 images,
arrange, selection, double-click focus, context menu and submenus, command
palette, rotate/undo, notes, save through the path dialog, `kill -9` recovery,
the 29-image `hardPics.moth`, deep zoom, `.pur` import, Russian locale.

Not covered here (needs a native session): native file dialogs, drag and drop,
always-on-top, overlay and click-through, Windows and macOS, a real GPU.

Target layout for every fix is [docs/DESIGN.md](../../DESIGN.md). Rules from the
[M2.6 plans](../ui-architecture/README.md#rules-for-every-step) apply: tier-1
checks only, no pull request, no waiting for CI, strings through `moth_ui::Text`
in English and Russian, a touch path for every control.

## What works

Settings window built from the schema, settings search in the palette, palette
focus and grouping, one bounding box for multi-selection, rotate + undo, saving
through the fallback path dialog, `kill -9` recovery keeps unsaved moves, `.pur`
2.1 import, the Russian locale, a 29-image board opening in under 3 s, deep zoom
with no loading, double-click focus with the Select tool.

## Findings

Severity: **A** breaks or hides work, **B** visibly wrong or confusing,
**C** polish.

| # | Sev | Finding | Where to look |
| --- | --- | --- | --- |
| 1 | A | A note typed at 35 % zoom shrinks to unreadable size after editing: the editor is screen-sized, the committed note is created at world size | note creation in `app/canvas.rs` (`Intent::AddNote`), note sizing in `app/document.rs` |
| 2 | A | Fit board, fit after arrange and double-click focus ignore the floating chrome: the top of the content sits under the selection bar, the bottom under the toolbar | camera fit in `app/camera_motion.rs` / `app/document.rs`; surface rects in `moth-ui/src/chrome.rs` |
| 3 | A | Hand is still the default tool, so clicks pan and double-click focus does nothing out of the box | default in `app/settings.rs` or `Tools`; `activate` in `app.rs` (`pointer_action == MoveDrag && !view.pan`) |
| 4 | A | After `kill -9`, the recovery window shows a raw path in `~/.local/share/mothref/recovery/…/board.moth` and only "Keep working"; the restored tab is named "board" instead of the original file name — verify it is still linked to the original `.moth` | recovery dialog in `app/modal.rs`, tab title in `app/workspace.rs` |
| 5 | B | Save chip sits on top of the tab bar and the logo, wraps to 2–4 lines, says "1 Images", lowercase "saved", counts notes as images, and keeps an English message after switching to Russian | `moth-ui/src/chrome.rs` `save_chip`; messages are stored as rendered strings |
| 6 | B | The selection badge ("1 selected · 600 × 758") wraps and is clipped; the hotkey toast ("Hold / Dismiss") covers the bottom toolbar | badge in `moth-ui/src/canvas.rs`; toast in `app/shortcuts.rs` |
| 7 | B | The selection bar overlaps the top of the selection and runs past the right window edge for items near the edge | selection bar placement in `app/surfaces.rs` |
| 8 | B | Menu icons are drawn over the first letters of their labels (Save, Open, Fit, pickers, Export); only some items have icons | menu row layout in `app/surfaces.rs` |
| 9 | B | Submenu arrows render as empty squares. The glyph test in `moth-ui/src/lib.rs` checks `→▸▾›`, but egui draws its own submenu arrow character | replace egui's arrow with an icon from `moth-ui/icons` |
| 10 | B | Submenus are titled after their first item ("Align left", "Bring to front" instead of "Align", "Order") and open to the left even with room on the right | group labels in `app/surfaces.rs` |
| 11 | B | "More actions" in the toolbar is a new junk drawer of 12 mixed items (save, new, open, fit, lock, grayscale, overlay, settings, library, palette, cheat sheet); Settings has two buttons, both with a ☰ menu icon | `TOOLBAR` and window-bar lists in `app/surfaces.rs` |
| 12 | B | Insert menu offers "Open" (ambiguous) instead of "Images…"; no visible way to insert images other than paste | Insert list in `app/surfaces.rs` |
| 13 | B | Light theme: white text on light sage buttons ("New board"), the library "Open" button has no visible background, sage toggles have low contrast | light token set in `moth-ui/src/theme.rs` |
| 14 | C | Settings is a small floating window that moves between openings; sidebar and content scroll together so the search field scrolls away; section icons do not match (image for Appearance, grid for Performance, ☰ for developers) | settings window in `app/settings.rs` |
| 15 | C | Settings › Adding images exposes packing jargon, and labels repeat the value ("Pack: best area" over a "Pack: best short side" dropdown) | schema entries in `app/settings.rs` |
| 16 | C | Palette: settings results are centered while commands are left-aligned; keys show as "Equals", "Minus", "Comma" | palette rows in `app/shortcuts.rs`; key labels in `app/bindings.rs` |
| 17 | C | Adding images still asks how to arrange (open question in DESIGN.md §6) and leaves the camera at 100 % afterwards | import arrange flow |
| 18 | C | Library: two "Open" buttons on the empty state | `moth-ui/src/library.rs` |
| 19 | info | `hardPics.moth` (29 images, 77 MB file) reaches 1.16 GB RSS in the default full-resolution mode on llvmpipe, where video memory is system RAM. Expected by decision 0017; check the same board on a discrete GPU | — |

## Fix plan

Five tasks, each one change set. Task 1 first; the rest in any order.

### Task 1 — Defaults and framing (findings 2, 3, 17)

Implemented; tier-1 formatting and Clippy checks only. Regression tests added for Select defaults and safe-area framing.

1. Make Select the default tool for new and reopened boards; Hand stays one key
   away (`H`, Space+drag, middle button).
2. Let double-click focus work with any navigation tool: compute `activate` from
   "not dragging" instead of requiring the move-drag pointer action.
3. Give camera fits a safe area: subtract the window bar, the bottom toolbar and
   the selection bar (when shown) from the viewport before fitting. Apply it to
   Fit board, Fit selection and double-click focus. Arranging items preserves the camera position and zoom.
4. After adding images, fit the camera to the added images.

### Task 2 — Notes keep their size (finding 1)

Implemented; tier-1 formatting and Clippy checks only. Added a regression test comparing editor and rendered text heights at 35% zoom; test execution deferred.

1. When a note is created, set its world size so it appears at the editor's
   on-screen size at the current zoom.
2. Edit and display the note at the same scale: the inline editor uses the
   note's world rect and font size multiplied by the zoom.
3. Regression test: create a note at 35 % zoom, commit it, and check the rendered
   text height equals the editor's.

### Task 3 — Chips, badges and toasts (findings 5, 6, 7)

Implemented; tier-1 checks only. Regression tests added; execution deferred.

1. Move the save state into the window bar, right of the tabs, as one line
   ("Saved" / "Not saved", plus the item count). It never covers tabs or content.
2. Count images and notes separately; use plural rules per locale
   ("1 image", "2 images"; "1 изображение", "2 изображения", "5 изображений").
3. Store status messages as `Text` keys with arguments and render them at draw
   time, so a language switch updates them.
4. Selection badge: single line, sized to its text, placed under the selection
   or inside the bottom-left chip area; never clipped.
5. Hotkey toast: stack above the bottom toolbar, never over it.
6. Selection bar: above the selection when there is room, otherwise below; clamp
   to the window and keep it clear of the window bar.

### Task 4 — Menus and toolbar contents (findings 8–12, 16)

Implemented; tier-1 checks only. Regression tests added; execution deferred.

1. Menu rows: a fixed icon column (blank when an action has no icon), label,
   right-aligned shortcut. No overlap at any interface scale.
2. Draw submenu arrows with a `moth-ui` icon instead of egui's glyph; extend the
   glyph test to the characters actually drawn.
3. Name submenu groups with their own `Text` labels: Arrange, Normalize, Align,
   Order. Open submenus to the right when they fit.
4. Remove "More actions" from the bottom toolbar. Move file actions (New, Open,
   Save, Library) to a window-bar menu; view actions (Lock canvas, Grayscale
   canvas, Overlay, Cheat sheet) to a View menu; one Settings button with a gear
   icon. The palette still lists everything.
5. Insert menu: "Images…" (file dialog for images), "Paste", "Note".
6. Shortcut labels as symbols: `=`, `−`, `,`, `/`.

### Task 5 — Settings window, light theme, recovery (findings 4, 13–15, 18)

Implemented; tier-1 checks only. Regression tests added; execution deferred.

1. Settings as a centered dialog with a fixed size; the sidebar and the content
   scroll separately; the search field stays visible.
2. Section icons: gear (General), palette (Appearance), hand (Canvas), image-plus
   (Adding images), window (Window and overlay), gauge (Performance), keyboard
   (Keys), plug (Integrations), code (For developers). Add missing icons to
   `moth-ui/icons`.
3. Adding images: plain labels ("Layout for new images", "Gap between images",
   "When many images are added"); packing heuristics move to For developers.
4. Light theme: dark text on accent buttons (`on_accent` token), visible
   background for secondary buttons, contrast of at least 4.5:1 for text.
5. Recovery dialog: list boards by their original name with the time of the last
   change, buttons "Restore" and "Discard"; no raw paths. A restored board keeps
   its original file name and path.
6. Library empty state: one "Open file…" button.

## Done when

- Every finding A and B is fixed or has a written reason in the change log.
- Re-running this review on the next alpha shows no overlap of chrome over
  content or over other chrome at 1440×900 and at 900×600.

## Validation notes for tasks 3–5

- Run formatting and Clippy for `moth-ui`, `moth-platform`, `mothref` and `xtask`; no tests, benchmarks, PR or CI wait in this session.
- Regression coverage includes plural rules and live message translation, text contrast, reserved menu icon slots, shortcut symbols, selection-bar placement, toast placement, recovery destination preservation, sidecar checksums, rejection of live journals and files outside recovery, and unnamed recovery from damaged destination metadata.
- Recovery destinations are checksummed native-path sidecars, written and atomically replaced by the storage worker. The `.moth` schema is unchanged. Source journals are archived after the replacement journal is durable, including any damaged tail; live journals are excluded using non-blocking file locks.
- Older journals did not record the original filename or destination. Guessing from the recent-file list could overwrite an unrelated board, so those journals restore as unnamed boards and require Save As. A damaged sidecar or an unavailable/already-owned original destination has the same safe fallback and does not block recovering the board itself.
- Native UI checks at 1440×900 and 900×600, Windows/macOS file locks and atomic replacement, and the recovery/journal kill-process checks remain for the maintainer or release preparation under AGENTS.md section 3.

## Regression guard: selection jumps back after dropping items

The shared selection bounds once read `board.scene` / `board.motion` after
pointer release, ignoring `DragMotion::returning`. The scene deliberately keeps
the old position until the journal acknowledges the move. Images already used
the released preview, so the shared frame and selection bar briefly jumped back.

When changing selection chrome, use the same presentation geometry as the drawn
items: live drag preview while dragging, then released (`returning`) geometry
before ordinary item transitions or scene geometry. Keep that priority even when
animations are disabled or the release animation has finished but storage is
still pending. Never fix this by mutating the scene before journal acknowledgment.
Cover delayed storage with multiple selected items and reduced motion; the canvas
regression is `selection_keeps_released_positions_until_storage_acknowledges`.


## Native follow-up — 2026-10-25

**Accepted by the maintainer on 2026-10-25:** “Можно принять”. The editing-polish
roadmap entry and implemented Album theme/type work are complete. The minimap
intentionally uses silhouettes, per the maintainer; the final mark, app icon and
README logo are on hold. Double-click focus was confirmed working by the
maintainer and left unchanged by this follow-up.

Implemented:

- Paper default while retaining explicit saved theme choices; Pixel dark palette;
  OFL Bricolage Grotesque Bold headings/wordmark with translated-text fallbacks.
  Removed the opaque dark decorative grid so the canvas follows theme tokens.
- Settings navigation/content stack independently instead of inheriting the outer
  horizontal layout. Transform, crop, duplicate and delete have vector icons;
  dropdowns have indicators, and keyboard palette navigation skips disabled items.
- Single and multi-selection corner scaling with the opposite corner fixed,
  proportional live previews, Escape cancellation and one journaled undo step.
  Released resize previews remain until storage acknowledges the change.
- Import method switching and Undo in one notification. Import and layout changes
  share the existing undo group until dismissal or the next mutation. Commands
  still reach the journal before the scene changes. Fit runs after acknowledgment.
  The default remains ask above five; automatic arrangement is opt-in.
- Picker hints, results and errors are visible in the tool window. Closing it
  disarms picking. Export has its own destination label and visible result/progress;
  shared action rows close menus rather than collapsing editor/export windows.
- Acquired surface texture dimensions determine clipping during resize, fixing
  a reproduced wgpu scissor panic. Old recovery offers no longer interrupt an
  active board after Undo empties it. Empty-board Choose files… inserts images.

Native validation used a release build, repository images and an isolated Linux
X11 display `:93`, profile `/tmp/mothref-editing-native-93/data`. Captures and
export artifacts remain in that temporary directory; artwork was not redistributed.
Covered 1440×900 and 900×600, repeated window shrinking/growing, selection frames,
corner scaling and Undo for one/two images, rotation/Undo, numeric frame cropping
and content scaling/Undo, six-image prompt → Grid → Vertical → Undo-all → Redo,
Settings/keyboard opening, palette focus, source-coordinate and color picking,
and native Copy image → Paste. The maintainer independently confirmed double-click
focus. A separate `--software` launch could not find a fallback adapter here.

Export evidence: PNG 2048×996 with a valid PNG header; parsed SVG with two embedded
images; PDF with a `%PDF-1.4` header; original-file and folder exports matched the
source SHA-256 `494ba158164fca56b7ff4f1ef78290cfb31cd9d03dd00f883c81c96f196ac62c`.
ZIP contained one original, passed CRC inspection and preserved its bytes.
Repeating the PNG export left its SHA-256 unchanged, confirming refusal to overwrite.

Regression coverage was added for import undo grouping/post-ack fitting, proportional
resize undo/checkpoint round trips, rotated anchors, theme defaults, Settings column
layout, translated feedback counts, keyboard availability and editor-window retention.
Formatting, workspace Clippy with warnings denied and the release build passed.
Automated tests, benchmarks, fuzzing, kill tests and soak tests were **not run** under
AGENTS.md §3; no long qualification was started. Windows/macOS, native file dialogs
and physical touch/pen still require platform/release qualification. Tests for earlier
recovery-worker changes remain subject to the same policy. The maintainer requested
ending lengthy validation; no additional native loops were run after acceptance.
