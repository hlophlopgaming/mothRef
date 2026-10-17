# Changelog

## Unreleased

- Fix first-time saving of library pins and tags on Windows when the metadata file does not yet exist.
- Import BeeRef `.bee` boards by reading their embedded SQLite database directly:
  images keep their layout and stacking order, every original is journaled before
  the import is acknowledged, and notes or transforms MothRef cannot yet store
  are reported as skipped.
- Implement the reference workspace: board library with previews, folder filters, search, persistent pins and tags; four independent board tabs; empty-board import prompt and save-on-close dialog.
- Add the floating tool rail, selection handles, minimap, image dragging, arrangement and size normalization, plus a dot-grid canvas.
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
