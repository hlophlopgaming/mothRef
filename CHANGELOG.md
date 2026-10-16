# Changelog

## Unreleased

- Add the M0 native canvas preview: image file drops, clipboard paste, pan/zoom,
  rubber-band selection, fit controls, adapter status and a custom egui theme.
- Add GPU fallback, bounded texture residency, background LOD decoding and a
  headless benchmark/validation harness.
- Boards are not saved yet. Closing a nonempty board asks before discarding its
  layout; preview detail is capped at 2048 pixels. Native Wayland file drops and
  the 50 ms end-to-end paste target remain open.
