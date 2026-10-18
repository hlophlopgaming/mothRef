# M0 / M1 technical debt

Snapshot: 2026-10-16. This backlog records implementation limits and missing
qualification from M0 and M1. It does not mark either milestone complete.
Work in the order below; preserve journal-before-acknowledgment, bounded memory,
and the storage/input abstractions throughout.

## Remaining refactor plan (App god-object)

Continues the `01-refactor-app-god-object.md` plan. Each step is one local commit
(`refactor(app): …`); quick checks (`fmt`, `clippy -p mothref`, `test -p mothref`)
after each, workspace-wide checks once at the end.

- [x] **Step 1 — Replace the 12 `frame()` flags with `Vec<Intent>`.**
  `intent.rs` defines `Intent` and `intents_from_actions`. Close buttons emit one
  `SaveAndClose` / `Discard` / `KeepOpen` intent; `apply` sets `close_after_save`
  and sends `Save`. Test: `SaveAndClose` on a board with a path never discards
  before the save completes; `Discard` with `in_flight > 0` loses nothing.
- [x] **Step 2 — UI no longer mutates App state.**
  Every direct assignment inside `run_ui` becomes an intent. Widget-local input
  (`&mut String` / `&mut bool` text fields, `board.view.pan` in the toolbar) stays.
  The `tools.preference_dirty` / `tools.fit` mailboxes become intents from `Tools::show`.
- [x] **Step 3 — Extract UI sections from `frame()`.**
  `app/chrome.rs` (toolbar/tabs/status), `app/canvas.rs` (CentralPanel: notes,
  selection, overlays), `app/modal.rs` (board_path, confirm_close, arrange_prompt,
  recovery). `draw_note` / `measure_note` / `screen_corners` / `canvas_clip` move to
  `canvas.rs`. `frame()` reads as surface check → results → run_ui → apply → render.
- [x] **Step 4 — Rendering moves into `Shell::render`.**
  Surface texture, Lost/Outdated/Occluded, egui buffers, `registry.draw`, present,
  free egui textures, deadline. Visible-item collection and LOD requests stay outside.
- [x] **Step 5 — Tabs and worker results move into `Workspace`.**
  `reset_board_ui` consolidates the tab-switch reset. `Workspace::{activate,new_board,
  open_in_tab,request_close,finish_close}` return a `TabChange`; `results`/
  `drain_results`/`complete` become `Workspace::pump`. Test: `request_close` on a
  dirty board.
- [x] **Step 6 — Cleanup.**
  Fold `cursor`/`command`/`native_pressed`/`input`/`gestures` into `InputState`.
  Close `pub(super)` fields that are only read externally. Update `ARCHITECTURE.md`.


### Manual refactor validation (pending; does not block the refactor)

Follow [UI validation](UI-VALIDATION.md) and
[M2 editing validation](M2-EDITING-VALIDATION.md), using disposable boards:

- [ ] Dirty close: Save with an existing path, Save As, cancel Save As, Keep Open,
  and Discard; repeat while import/save work is in flight. Reopen saved boards
  and verify the acknowledged edits.
- [ ] Open/new/switch/close tabs, including closing the whole window with several
  dirty tabs; confirm each tab keeps its camera, lock flags and separate undo history.
- [ ] Create/edit notes, focus loss, unchanged draft, undo/redo, selection and
  double-click image focus/return.
- [ ] Mouse/pen drag, Alt+drag duplicate, pen hover/cancellation, touch pan/pinch
  where hardware is available; repeat after switching tabs.
- [ ] Resize with/without camera scaling, fit/zoom, tools, arrangement prompts,
  library pin/tags/previews, recovery browser and overlay controls.
- [ ] Minimize/restore, suspend/resume and device/surface recovery; verify library
  thumbnails reload and the board survives. Repeat native checks on each OS.

Automated refactor checks (2026-10-19):

- `cargo fmt --all` / `cargo fmt --all --check`: passed.
- `cargo clippy -p mothref --all-targets -- -D warnings` and `cargo test -p mothref`:
  passed after each resumed step (19 mothref tests on the final build).
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including headless golden/BC7 rendering,
  allocation checks, parked-tab worker routing and save-before-discard coverage.

Per the task's quick-check workflow, stress/benchmark/fuzz runs were not repeated.
No format or feature change is intended. Native checks above remain unexecuted.

## Release qualification first

- [ ] **TD-01 — Qualify the final persistence build.**
  [M1 validation](M1-VALIDATION.md) records successful 1,000-kill and reader-fuzz
  runs, but no completed final-build eight-hour combined soak. Run
  `cargo xtask soak 28800` on a machine that can finish it; retain the build
  revision, environment, RSS samples and plateau result. Repeat kill/fuzz checks
  if persistence changes. The [hosted workflow](../.github/workflows/reliability.yml)
  has a six-hour limit and cannot establish this gate. Close when the final
  build has recorded eight-hour render/persistence results with flat RSS.
  The 30-minute combined follow-up passed (17,048 frames; second-half RSS spread
  16 KiB); the eight-hour gate remains open.
- [ ] **TD-02 — Finish native lifecycle qualification.**
  Exercise save, recovery, clipboard, close, shutdown, suspend/resume and live
  device/surface loss on Windows, macOS and Linux. Include WM_QUERYENDSESSION
  and applicationShouldTerminate. Record OS/backend and observed recovery of
  acknowledged work in the validation notes. CI/type-checks alone do not close
  this item. See [M0](M0-VALIDATION.md) and [M1](M1-VALIDATION.md) validation.

## M0 implementation debt

- [x] **TD-03 — Improve durable 4K paste latency (accepted).**
  The maintainer accepts approximately 200 ms (198.770–204.196 ms measured
  after clipboard transfer), improved from roughly 400 ms. Lossless PNG staging,
  bounded buffers and streaming identity hashing preserve durability. Workspace
  tests, dhat and 1,000 process kills passed. The 150 ms goal is withdrawn.
  Timing outliers and native clipboard-transfer qualification remain documented
  in [M0 validation](M0-VALIDATION.md); this acceptance does not establish a
  worst-case latency guarantee or close TD-02.
- [x] **TD-04 — Remove the 10,000-item ceiling without unbounded residency.**
  [App admission](../crates/mothref/src/app.rs) and
  [renderer instances](../crates/moth-render/src/lib.rs) previously imposed fixed ceilings.
  Replace render truncation with bounded batches and make import admission
  independent of that batch size. Close with a board above 10,000 items that
  renders every visible item, preserves ordering, and keeps cache budgets and
  benchmark targets. The roadmap schedules this for M1–M2. Implemented ordered
  submissions using the same instance buffer and separate import
  admission/backlog limits. The 10,003-item pixel golden now passes; it failed
  before the batching fix.
- [x] **TD-05 — Avoid linear top-index scans for repeated insertion.**
  [Scene::top](../crates/moth-scene/src/lib.rs) previously scanned all items. Maintain the
  maximum through the existing command path, including remove and undo/redo.
  Close with command/inverse properties and insertion benchmarks showing the
  repeated scan is gone; do not introduce integer z-order. Implemented an
  ordered `(fractional index, UUID)` set updated only through commands; property
  tests cover removal, undo/redo and equal-z items.
- [x] **TD-06 — Budget JPEGs by reduced allocation.**
  [Decode](../crates/moth-io/src/lib.rs) previously rejected sources above `MAX_PIXELS`
  before DCT scaling. Permit larger JPEG headers only when the actual reduced
  decode fits an explicit allocation budget. Keep dimension/input limits and
  panic isolation. Close with large-source fixtures, rejection tests, dhat,
  decoder fuzzing and benchmark results. Implemented reduced-plane and
  progressive-coefficient admission checks, an 8192×4097 JPEG fixture, and
  an allocation regression check.
- [x] **TD-07 — Complete planned image compatibility and preview quality.**
  Finish WebP, static GIF and BMP imports, EXIF orientation and CMYK JPEG
  conversion; replace sampled PNG/clipboard previews with area averaging,
  including Adam7. Scope each as a separate change in
  [moth-io](../crates/moth-io/src/lib.rs). Close each case with representative
  corpus fixtures, pixel/orientation checks, allocation checks and fuzzing.
  Do not enable a codec through an unrestricted full-resolution decode path.
  **Implemented:** JPEG EXIF orientation, CMYK conversion, small-preview sizing,
  alpha-weighted PNG/clipboard area averages and Adam7 coverage.
  **Implemented in the TD-07 follow-up:** first-frame GIF (interlace, transparent
  offsets) and row-based BMP previews (uncompressed/bitfields, palettes, alpha,
  top-down and bottom-up). 4K allocation tests stay below 8 MiB.
  **WebP:** lossy/lossless, alpha and orientation use the maintainer-approved
  [bounded exception](decisions/0003-bounded-webp-previews.md): encoded input and
  full RGBA frame each at most 8 MiB; oversized/animated files are rejected.
  Private decoder allocations are not covered by a global allocator cap.
  RLE/embedded-codec BMPs remain explicitly unsupported; they never fall back
  to an unrestricted full-frame decode.
- [x] **TD-08 — Report clipboard backpressure and handle other payloads.**
  [App paste](../crates/mothref/src/app.rs) queues image clipboard work;
  the M0 follow-up also requires busy feedback and file/URL/HTML handling.
  Close with localized, pointer-accessible feedback for busy/empty/unsupported
  payloads and explicit tests for accepted payloads. URL/HTML support must not
  cause network requests without an explicit user action.
  **Implemented:** localized queue/empty/unsupported feedback and bounded native
  file-list paste (1–1,024 absolute paths, preserving order).
  **Implemented:** direct HTTP(S) image URLs and the first HTML `img[src]`,
  fetched only on Paste with size/time/redirect limits. Web pages, scripts, data
  URLs and embedded credentials are unsupported. Native interoperability remains
  part of TD-02.
- [ ] **TD-09 — Finish native Wayland drops and pen input.**
  Native Wayland `wl_data_device` and `tablet-v2` input now supplement winit
  through [the input adapter](../crates/moth-platform/src/input.rs). Native
  protocol tests cover descriptor transfer, copy negotiation, rejected offers,
  tool frames/pressure/cancellation and reader shutdown. The maintainer confirmed
  a two-file Dolphin drop; Huion pen traces verify native identity and pressure.
  Real touchscreen hardware is unavailable, so this qualification item remains
  open. Windows/macOS pen identification is also not provided by this backend.
  See [device notes](M0-VALIDATION.md#td-09-native-wayland-drops-and-tablet-input).


## M1 implementation and documentation debt

- [x] **TD-10 — Select compressed texture formats at runtime.**
  [Renderer](../crates/moth-render/src/lib.rs) now picks BC7 when the adapter
  reports `TEXTURE_COMPRESSION_BC` and falls back to RGBA8 (including the
  software adapter). BC7 is encoded on the decode pool via `intel_tex_2`, a
  target-gated x86/x86_64 dependency, so `aarch64-linux-android` keeps building
  with RGBA8. ASTC/ETC2 are deferred to the mobile milestone (M8), where ARM
  `astcenc` adds the ASTC path; originals and rebuildable LOD caches remain
  GPU-format independent. Closed with capability/fallback coverage (unit
  round-trip and a headless BC7 golden).
- [x] **TD-11 — Reconcile historical architecture notes with M1.**
  [Architecture](ARCHITECTURE.md) previously described raw RGBA staging, a 512 MiB
  staging budget and deferred persistence in its M0 sections; later M0 validation
  describes PNG staging and an 8 GiB per-board budget, and M1 adds persistence.
  Clearly separate historical decisions from current behavior and verify each
  current budget against code. Close when the architecture and validation notes
  agree without discarding historical measurements or claiming unrun checks.
  Updated the architecture to current M1 persistence, PNG staging, batching,
  decode and queue budgets; retained historical validation measurements.

## Already resolved or intentionally outside this backlog

- Storage temporary objects and atomic replacement from the M0 follow-up are
  implemented by [Storage](../crates/moth-format/src/lib.rs) and
  [DesktopStorage](../crates/moth-platform/src/lib.rs); the stale open roadmap
  checkbox is now checked.
- M1 truncation, 1,000-kill and moth-reader fuzz checks have recorded passing
  results. The grouped roadmap testing checkbox remains open for the soak gate;
  it does not mean those checks are unimplemented.
- The 256-command session history and empty-board-only opening are documented
  product limits. Tabs belong to M3; persistent undo is not currently promised.
- Later-milestone features and unchecked feature-inventory entries are not
  automatically technical debt. In particular, BeeRef/PureRef reader fuzz targets
  belong with their M2 importers.

## Validation

See the technical-debt follow-up in [M0 validation](M0-VALIDATION.md) for current
checks and measurements. The remaining release gates above stay open until
executed on the final build and appropriate native platforms.
