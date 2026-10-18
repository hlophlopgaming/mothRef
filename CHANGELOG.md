# Changelog

## Unreleased

- Keep dragged images and selection outlines at their released position until storage acknowledgement, without replaying the move animation.

- Add canvas cursors and mouse/pen hover outlines, animated toolbar/tab feedback and a subtle library-card lift.

- Animate item entry, removal and acknowledged arrangement/undo geometry changes using bounded presentation copies.

- Smooth discrete wheel zoom around the pointer and fit/focus transitions; keep continuous input direct and add touch-pan inertia.

- Add grab-point drag sway, a gentle lift and animated cancellation; add a localized reduced-motion preference.

- Preview the entire selection during dragging, including Alt-drag copies, without changing board data.

- Add view-only motion primitives with exact endpoints and reduced-motion support.

- Build interface languages from translation catalogs, with automatic menu registration,
  English fallback and validation; document how to add a language.

- Keep the canvas tool rail compact instead of letting its separator stretch a
  panel across the board; preserve wrapping controls in short windows.

- Add Alt-drag duplication, selection dragging, arrow-key nudges, alignment,
  distribution, stacking, layer controls and repeat-to-reverse arrangement.
- Add per-tab canvas locks, canvas grayscale, image-focus toggling and persistent
  theme, interface scale, undo-depth, wheel and arrangement preferences.
- Export boards/selections as bounded raster PDFs and unchanged originals as ZIP;
  add RGB/HSV color-copy choices and original-pixel coordinate information.
- Fix grouped undo/redo ordering for dependent commands, including arranging an
  insertion burst as one undo step.

- Add saved/journaled and recovery status, editable toolbar zoom, live selection
  measurements, shortcut tooltips, and wrapping controls for small screens.

- Speed up bulk image imports and board opening by syncing originals with their
  command/checkpoint in the recovery journal, and reusing foreign-import working
  copies. Recovery still rebuilds originals from the journal before use.

- Add Board tools with Apply-based frame scaling, arbitrary rotation, crop-content
  pan/zoom/rotation, flips, opacity and grayscale; edits are journaled and undoable.
  Arrange selections with a custom gap and source-filename ordering, or scale them
  together. Schema 4 preserves appearance and filenames while reading schemas 1–3.
- Export scenes, selections and cropped regions to PNG/SVG, copy images and sampled
  HEX colors, and export one or all original images without re-encoding. Exports
  never overwrite files. Scene/clipboard exports and sampling use up to 2048-pixel
  source previews; scene output is capped at 4096 pixels on its longest side.


- Accept `.pur` boards up to 4 GiB with separate payload and metadata budgets;
  legacy boards now stream originals instead of buffering the whole file.
  Import SQLite columns by name, fixing skipped images in PureRef 2.1.2 boards
  whose column order differs from 2.1.3. The 175-image FemaleFighterPosesPack
  board passes import, `.moth` reload and grouped undo/redo checks.

- Open PureRef `.pur` boards through file open, command-line arguments and drops.
  Import supported embedded images and plain notes on the storage worker with
  journal recovery and one-step undo. A local purcore safety fork bounds memory
  and nesting; captured 1.10/2.1 boards are tested. Unsupported items are reported
  as skipped; links, rich notes, drawings and some crops remain unsupported.

- Import BeeRef text notes alongside images, and add text notes to the board
  format (schema 3): notes are imported, saved, reopened, created (context menu or
  double-click empty canvas) and edited in place (double-click a note). Schema 1
  and 2 boards still open.
- Carry BeeRef item rotation, horizontal flip and scale through the import:
  rotated/flipped images render correctly, and notes keep their rotation and
  scale (flip is preserved in the file).

- Open images and boards through the native file picker instead of typing a
  path: the Open button (canvas, empty board and library) and Ctrl/Cmd+O now
  browse the OS file manager and add the selected files to the board.

- Choose the image texture format at runtime: BC7 on desktop adapters that
  support it, uncompressed RGBA8 otherwise (including the software adapter).
  Previews are encoded to BC7 on the decode pool, quartering texture memory for
  the same residency. ASTC for mobile is deferred to the mobile milestone; the
  `.moth` format stays GPU-format independent.

## 0.1.0-alpha.2 — 2026-10-17

- Keep Undo, Redo and Save visually steady while edits are acknowledged by the journal.

- Fix crossed selection outlines and remove the filled drag preview: both now use a simple theme-colored frame without diagonals or tinting.

- Group multi-item edits into a single undo step: arranging, normalizing, rotating,
  mirroring, resetting or deleting a selection, the transform hotkeys, a `.bee`
  import, and a drop/paste burst of many images now all undo (and redo) with one
  Ctrl+Z. The previous 32-item selection cap is removed.
- After one drop or paste adds more than five images, ask how to arrange them
  (horizontal, vertical, grid or optimal) and place the result under the cursor.
  The chosen arrangement is a separate undo step; the threshold is a constant until
  a settings store lands. Pasted and dropped images now both land at the cursor.
- Add interface localization: English and Russian, with the OS language detected on
  first launch and a language picker in the title bar. The choice persists in a
  plain-text settings file next to the recent-file catalog.
- Add item content transforms to the scene and board format (schema 2). An item now
  stores frame rotation, content flip, pan, zoom and content rotation as a reversible
  `SetTransform` command, and rotated items are indexed by their rotated bounds.
  Schema 1 boards and journals still open, with the identity transform.
- Render and edit item transforms: content is fitted into the frame without distortion and
  clipped to it, so `content_scale`, `content_offset` and `content_rotation` crop, pan and
  spin the source inside a frame, and a flipped item mirrors its source. Selection outlines
  the rotated frame, clicks hit the rotated rectangle, the context menu and the
  `R`/`Shift+R`/`Shift+H`/`Shift+V`/`Shift+0` keys rotate, mirror and reset the selection,
  and the minimap and drag ghost follow the rotated footprint.
- Reject cyclic or excessively deep BeeRef table trees instead of overflowing the importer stack.
- Bound JPEG preview memory independently of worker scheduling, fixing allocation-test failures on macOS.
- Fix first-time saving of library pins and tags on Windows when the metadata file does not yet exist.
- Import BeeRef `.bee` boards by reading their embedded SQLite database directly:
  images keep their layout and stacking order, every original is journaled before
  the import is acknowledged, and notes or transforms MothRef cannot yet store
  are reported as skipped.
- Implement the reference workspace: board library with previews, folder filters, search, persistent pins and tags; four independent board tabs; empty-board import prompt and save-on-close dialog.
- Add the floating tool rail, selection handles, minimap, image dragging, arrangement and size normalization, plus a dot-grid canvas.
- Expand the selection menu: normalize height, width, size and area, and arrange
  horizontal, vertical, grid and optimal (packed to the window width), all as
  reversible commands.
- Read image and file clipboard contents on Wayland sessions by enabling arboard's
  `wayland-data-control` backend, fixing pasting a copied PNG returning
  "no supported image" on KDE Wayland and other compositors.
- Add transparent overlay mode with master opacity, click-through and a separate control window; show platform availability for always-on-top.
- Fix idle redraw loops, fitting boards opened from the library, and saving to relative filenames; prevent two tabs from writing the same destination.

- Accept native Wayland file drops and tablet-tool input with pen identity, pressure, hover and contact cancellation; keep touch pan/pinch in the gesture layer.

- Paste a direct HTTP(S) image URL or the first HTML image; download on the clipboard worker with bounded size/time and preserve original bytes for recovery.

- Import static GIF and uncompressed/bitfield BMP previews by rows; add bounded static WebP imports (8 MiB input and RGBA frame limits), including lossless/lossy, transparency and EXIF orientation.

- Clipboard staging hashes PNG bytes during encoding and reuses that identity for previews, avoiding redundant pixel hashing and rereading the staged file.

- Speed up durable clipboard staging with larger PNG write chunks and lossless RGB
  storage for fully opaque images; preserve RGBA when any pixel is transparent.

- Render visible images in ordered batches beyond the old 10,000-item ceiling,
  with a separate bounded import backlog and explicit queue feedback.
- Index fractional z-order to avoid scanning the board on every image insertion.
- Accept large JPEGs when reduced decoding fits the memory budget, honor JPEG
  EXIF orientation, convert CMYK colors and avoid enlarging small previews.
- Average PNG and clipboard preview areas (including Adam7 and transparency)
  instead of sampling individual source pixels.
- Paste native clipboard file lists through the image import path and explain
  empty/unsupported clipboard contents in the UI.

- Keep canvas drags owned by their starting pointer, cancel selection/pan when
  pinching or losing focus, and ignore gesture starts over the toolbar.
- Normalize touch pressure to a finite 0–1 range and ignore duplicate contacts.

## 0.1.0-alpha.1 — 2026-10-16

- Add version-one `.moth` boards, append-only saves, atomic background compaction,
  synced command journals, rotating backups, startup recovery and recent files.
- Add Save/Open path controls, save-on-close and undo/redo (256 session commands),
  with visible toolbar controls and Ctrl/Cmd+S, Ctrl/Cmd+Z and Ctrl/Cmd+Y.
- Add local crash reports and native termination handling. Tagged releases build
  Windows, macOS (Intel/Apple Silicon) and Linux binary archives from M1 / 0.1.

- Store clipboard originals as lossless PNGs in application data, grouped by board,
  with an 8 GiB disk budget per board. Explicit discard cleans that board's files;
  crashes retain them for future recovery tooling.

- Add the M0 native canvas preview: image file drops, clipboard paste, pan/zoom,
  rubber-band selection, fit controls, adapter status and a custom egui theme.
- Add GPU fallback, bounded texture residency, background LOD decoding and a
  headless benchmark/validation harness.
- Preview detail is capped at 2048 pixels. Native Wayland file drops and
  the 50 ms end-to-end paste target remain open.
