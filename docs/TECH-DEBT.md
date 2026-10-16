# M0 / M1 technical debt

Snapshot: 2026-10-16. This backlog records implementation limits and missing
qualification from M0 and M1. It does not mark either milestone complete.
Work in the order below; preserve journal-before-acknowledgment, bounded memory,
and the storage/input abstractions throughout.

## Release qualification first

- [ ] **TD-01 — Qualify the final persistence build.**
  [M1 validation](M1-VALIDATION.md) records successful 1,000-kill and reader-fuzz
  runs, but no completed final-build eight-hour combined soak. Run
  `cargo xtask soak 28800` on a machine that can finish it; retain the build
  revision, environment, RSS samples and plateau result. Repeat kill/fuzz checks
  if persistence changes. The [hosted workflow](../.github/workflows/reliability.yml)
  has a six-hour limit and cannot establish this gate. Close when the final
  build has recorded eight-hour render/persistence results with flat RSS.
- [ ] **TD-02 — Finish native lifecycle qualification.**
  Exercise save, recovery, clipboard, close, shutdown, suspend/resume and live
  device/surface loss on Windows, macOS and Linux. Include WM_QUERYENDSESSION
  and applicationShouldTerminate. Record OS/backend and observed recovery of
  acknowledged work in the validation notes. CI/type-checks alone do not close
  this item. See [M0](M0-VALIDATION.md) and [M1](M1-VALIDATION.md) validation.

## M0 implementation debt

- [ ] **TD-03 — Meet the durable 4K paste latency target.**
  [Clipboard staging](../crates/moth-platform/src/lib.rs) uses lossless PNG;
  the latest documented noisy-4K measurement is 349.270 ms after clipboard
  transfer. `cargo xtask paste` excludes OS clipboard transfer. Profile staging
  and first-frame delivery; keep originals durable before acknowledgment.
  Close with a measured native clipboard-to-visible time below 50 ms, plus
  allocation and crash-durability checks for the changed path.
- [x] **TD-04 — Remove the 10,000-item ceiling without unbounded residency.**
  [App admission](../crates/mothref/src/app.rs) and
  [renderer instances](../crates/moth-render/src/lib.rs) previously imposed fixed ceilings.
  Replace render truncation with bounded batches and make import admission
  independent of that batch size. Close with a board above 10,000 items that
  renders every visible item, preserves ordering, and keeps cache budgets and
  benchmark targets. The roadmap schedules this for M1–M2. Implemented ordered
  submissions using the same 320,000-byte instance buffer and separate import
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
- [ ] **TD-07 — Complete planned image compatibility and preview quality.**
  Finish WebP, static GIF and BMP imports, EXIF orientation and CMYK JPEG
  conversion; replace sampled PNG/clipboard previews with area averaging,
  including Adam7. Scope each as a separate change in
  [moth-io](../crates/moth-io/src/lib.rs). Close each case with representative
  corpus fixtures, pixel/orientation checks, allocation checks and fuzzing.
  Do not enable a codec through an unrestricted full-resolution decode path.
  **Implemented:** JPEG EXIF orientation, CMYK conversion, small-preview sizing,
  alpha-weighted PNG/clipboard area averages and Adam7 coverage.
  **Still open:** WebP, static GIF and BMP preview decoding.
- [ ] **TD-08 — Report clipboard backpressure and handle other payloads.**
  [App paste](../crates/mothref/src/app.rs) queues image clipboard work;
  the M0 follow-up also requires busy feedback and file/URL/HTML handling.
  Close with localized, pointer-accessible feedback for busy/empty/unsupported
  payloads and explicit tests for accepted payloads. URL/HTML support must not
  cause network requests without an explicit user action.
  **Implemented:** localized queue/empty/unsupported feedback and bounded native
  file-list paste (1–1,024 absolute paths, preserving order).
  **Still open:** URL/HTML import; unsupported remote content is explained without
  fetching it.
- [ ] **TD-09 — Finish native Wayland drops and pen input.**
  winit 0.30 does not supply native Wayland file drops or pen identification
  for the current adapter. Implement through
  [moth-platform input](../crates/moth-platform/src/input.rs), preserving pointer
  kind, pressure and the separate gesture layer. Close with native Wayland drop
  tests and real pen/touch device notes; `Kind::Pen` alone is insufficient.

## M1 implementation and documentation debt

- [ ] **TD-10 — Select compressed texture formats at runtime.**
  [Renderer](../crates/moth-render/src/lib.rs) currently uploads RGBA8 previews.
  Add supported compressed upload/encoding paths selected from adapter
  capabilities, retaining RGBA8 and the software adapter fallback. Originals
  and rebuildable LOD caches must remain GPU-format independent. Close with
  capability/fallback coverage, golden images and memory/performance results.
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
