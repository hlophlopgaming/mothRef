# UI architecture refactor — implementation notes

Implementation of steps 1–5, 2026-10-24. Board storage, scene commands,
renderer residency and decode policy retain their existing implementations.

## Control inventory and new homes

Every command below uses the action registry and `Intent::Action`; parameter
fields edit local drafts. File/recovery/catalog entries carry their selected path
in a data intent. Automatic import arrangement and canvas gestures retain their
existing contextual intents.

| Previous controls | New home | Shared effect |
| --- | --- | --- |
| Select, Hand, fit, zoom | Floating bottom toolbar | Select, Hand, Fit, ZoomOut/ZoomIn; exact percentage remains a value intent |
| New/open/save, undo/redo, library | Window bar, Insert, View menu, palette; undo/redo also bottom toolbar | Existing registered actions |
| Note creation and editing | Insert, note selection bar, double tap | Note, EditNote; pointer position remains a contextual intent |
| Rotate, flip, reset rotation/flip/crop/transform | Selection bar, Transform menu, palette | Transform/reset actions |
| Position, size, opacity, grayscale, content scale/offset/rotation | Edit selection window | Validated draft, ApplyItem / CancelItem |
| Horizontal/vertical counts, arrange method/order/gap, Pack and Fit | Arrange options window, Arrange menu, selection bar | Shared arrangement actions, one command batch |
| Normalize, align, distribute, stack, relative scale | Selection bar/context submenus, Arrange options, palette | Normalize/align actions, ScaleApply |
| Reset scale, normalize scale | Normalize submenu and palette | OriginalGeometry requests, image bytes remain on workers |
| Duplicate, nudges, layer order, delete | Context menu, palette, existing keyboard bindings | Shared edit actions |
| Lock and grayscale canvas | View menu and palette | Lock, Grayscale |
| Previous/next image, reset camera | Palette and existing keyboard bindings | Previous, Next, ResetCamera |
| Color and coordinate pickers, HEX/RGB/HSV choice | Picker window opened from context menu or palette | SampleColor / CoordinatePicker; results still come from the worker |
| Export scene/selection/originals/archive, region and size | Export window, context menu, palette | Export actions; path and crop validation retained |
| Theme, scale, motion, feedback, wheel/zoom, undo limit | Settings sections; Ctrl/Command+comma | Settings schema, existing preference keys and validation |
| Arrange defaults, gap, prompt threshold, packing options | Settings › Adding images | Settings schema |
| Performance mode, tile preparation, video-memory override, disk usage/clear | Settings › Performance and memory | Settings schema; existing maintenance worker |
| Language | Settings › General | Existing locale and worker persistence |
| Binding editor, presets, cheat sheet | Settings › Keys and mouse; View menu/palette | Existing binding editor and conflict preview |
| Adapter, residency, RAM cache, journal diagnostics | Settings › For developers | Read-only diagnostics |
| Save-path, close confirmation and import-layout buttons | Existing modal dialogs | Registered actions dispatch to the original intents; save completion still precedes discard |

`Tools` now holds transient editor/picker/arrangement state and diagnostic
snapshots. It has no persisted preference and opens no Board tools window.
`Settings` owns preference values; its schema generates persistence, setting rows,
section resets and palette results. Settings remain independent of tab changes.

## Checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed, including
  compilation of the new and relocated test targets. No dependencies added.

- Added checks for theme switching, schema round-trip/invalid values, settings
  search, registered surface references, narrow toolbar layout and idle repaint.
- Moved the former Tools regression tests with their implementation, retaining
  arrangement order, zero-gap restoration, preference validation and scale checks.
- Updated persistence-chip expectations and moved obsolete tool-rail layout
  coverage to the bottom toolbar.
- Test execution, screenshots, benchmarks and native interaction checks are
  deferred under AGENTS.md section 3; compiling tests through Clippy is not a test run.

## Manual/CI qualification still needed

- Light/dark board, library, settings and selection screenshots, including narrow
  windows, wrapping toolbars, changed-setting markers and palette focus/position.
- Saving, Save As cancellation, dirty close, discard during queued work, recovery,
  and undo/redo after every migrated edit command on disposable boards.
- Import burst arrangement, all export modes, inspector Apply/Cancel, picking,
  touch long-press and selection-bar More; physical pen/touch on supported OSes.
- GPU/surface recovery preserves theme and UI scale; idle UI stops repainting.

The auxiliary step-4 minimap-thumbnail improvement is deferred: the minimap
continues to draw its bounded metadata samples. Sharing image previews with egui
needs a separately budgeted implementation; this refactor adds no pixel cache or
decoding on the UI thread. The main surface-list migration does not depend on it.
Design open questions (arrangement default, performance-mode merging, future
Shapes tools and the final moth mark) remain as recorded in DESIGN.md.

## Release preparation — v0.1.0-alpha.9

- All 263 workspace tests passed, including the new UI checks; local socket tests
  ran outside sandbox restrictions with `XDG_DATA_HOME` under `/tmp`. This
  supersedes the earlier deferral of automated test execution.
- `cargo check --workspace`, `cargo fmt --all --check`, workspace Clippy and
  `cargo deny check` passed. Cargo deny retains existing duplicate-version warnings.
- The toolbar size regression exposed nested enabled scopes bypassing button
  wrapping. Native enabled buttons now fit the 320–1440 px test viewports.
- egui 0.36's `has_glyph` compares fallback face IDs with its replacement face,
  giving false negatives for Hack fallback glyphs. The regression compares actual
  rasterized glyphs with a missing-character glyph instead; removing Hack makes
  the check fail, and restoring it passes.
- Quick benchmark passed on llvmpipe: 10,000 images, p99 27.199 ms; this does not
  establish a discrete-GPU performance comparison.
- The 300-second soak passed: 2,441 frames, RSS 438,860–452,448 KiB;
  second-half RSS spread 768 KiB, below the 16 MiB plateau guard.
- Manual/native qualification and screenshot comparison remain pending.


## Zoom, file picker and initial UI scale follow-up

Automated coverage exercises held +/− toolbar buttons through egui pointer events,
release without an extra step, idle behavior, QHD/4K/portrait monitor defaults,
native DPI compensation and persisted scale overrides.

Linux validation: `cargo test -p mothref` passed all 134 tests;
`cargo fmt --all --check` and workspace Clippy with `-D warnings` passed.

Native Windows/macOS interaction remains unverified in the Linux agent session:

- Windows: with the board pinned and unpinned, use Insert › Images and Open;
  verify the picker appears above its owner, cancel and reopen it, select several
  images, then repeat from a second MothRef window. The picker now runs on a fresh
  thread so its STA initialization cannot conflict with the caller's COM apartment.
  The reported failure could not be reproduced locally; this addresses the thread
  and ownership hazards, rather than establishing which caused that report.
- macOS: repeat Open/Add images and cancellation; rfd's Cocoa backend dispatches
  panel creation and its modal run to the main thread while the event loop runs.
- On both: first launch with no saved scale at QHD and 4K, change Settings ›
  Appearance › Interface scale, restart, and verify the chosen value remains.
