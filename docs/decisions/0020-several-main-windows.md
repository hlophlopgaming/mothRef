# 0020 — Several main windows in one process

## Context

M3 asks for dragging a board tab out into its own window and back. The
application was written around one main window: `App` (about 3,000 lines with
its submodules) holds one window, one egui context, one renderer surface and
one set of tabs, and every handler reads those fields directly.

## Decision

- Each main window is a set of per-window fields: tabs, Collect/View/Over
  state, floating views, pin, input, modal and drag state, egui context,
  surface and texture registry. The current window lives in `App`'s fields as
  before; every other window keeps the same fields in a `Pane`
  (`app/panes.rs`) and is swapped in before its events are handled. Existing
  handlers did not change.
- Shared between windows: settings, shortcuts, caches, workers outside the
  boards, and the GPU device. Each window has its own texture registry,
  because switching boards clears a registry; the video-memory limit is split
  evenly between windows.
- Worker wake-ups, repaint requests and frame deadlines visit every window.
  Operations that change the current window (moving tabs) are queued during a
  frame and run after it, so a frame never renders into another window.
- Moving a board moves its `Document` (journal, workers, scene) between
  windows; nothing is written or reloaded from disk. Moving a window's last
  board closes that window. Blank, never-used boards are dropped instead of
  becoming tabs.
- The way back: the tab menu everywhere; additionally a drop onto another
  window's tab strip where window positions are known (Windows, macOS, X11).
  Wayland exposes no window positions, and the implicit pointer grab keeps a
  drag inside its window.
- Main windows get unique titles ("MothRef (2)"): KWin scripts and the GNOME
  extension identify windows by title.

## Consequences

- Per-window state must be added to the `pane!` list in `panes.rs`;
  forgetting it shares the field between windows.
- Two windows showing large boards in full-resolution mode each get half the
  video memory.
- After device loss, the other windows' surfaces and registries are rebuilt
  on the new device.
