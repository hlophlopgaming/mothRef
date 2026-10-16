# Changelog

## Unreleased

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
