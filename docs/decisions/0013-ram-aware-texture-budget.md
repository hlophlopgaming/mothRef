# 0013 — Available-RAM and texture-byte cache budgeting

## Context

The maintainer replaces decision 0009's 1 MiB-per-image policy with an allowance
based on free physical RAM and the memory required by images. Encoded file sizes
do not represent decoded texture memory; duplicate items can share one texture.

## Decision

Grow the image-cache budget on demand using actual size-class texture-page bytes,
including mip levels, padding, unused layers and fallback pages. Start with at
most 32 MiB without preallocation. Each new page can grow the budget up to
`min(selected cap, (available RAM + allocated image pages) × RAM fraction)`; reaching the cap
uses the existing LRU eviction. Reductions evict immediately and may go below
32 MiB or reach zero. Empty boards without pending work clear all image pages.
The image count only distinguishes empty boards; its value does not size a budget.

Default to the 96 MiB shared/software and 512 MiB discrete adapter caps: physical RAM
does not measure available VRAM. Settings expose a 1–50% RAM allowance and a
manual 32–512 MiB cap, or automatic adapter limits. The default remains 12.5% and
automatic limits. Explicit manual limits can exceed the shared-adapter default,
but the renderer's absolute 512 MiB ceiling and RAM allowance always apply.
Apply changes immediately and persist through existing settings; invalid values
fall back to defaults. The ordinary Defaults action resets both preferences.
Add back reclaimable image pages to avoid a
feedback loop as shared-memory allocations reduce reported available RAM.
This is a conservative cache allowance, not a reservation or an OOM guarantee.
Other allocations (decoders, surfaces, egui, metadata) retain their own budgets.

Use MIT-licensed `sysinfo` with only its `system` feature through the `Platform`
boundary. Query only RAM, not processes, CPU or swap. A single platform worker,
bounded request/result channels and a two-second activity throttle keep OS I/O
off the main thread. Unknown RAM falls back to the adapter cap. No polling timer
wakes an idle app; updated pressure is observed during subsequent activity.
API reference: https://docs.rs/sysinfo/0.39.6/sysinfo/struct.System.html#method.available_memory.

## Consequences

A few detailed images may grow the budget, while many small or repeated images
do not. Texture dimensions and format, not compressed source bytes, determine
allocation. Existing explicit budgets, journal and recovery semantics remain.
Deletion may leave nonempty-board cache pages until LRU eviction; empty-board
and board-switch teardown still release them. RAM readings are snapshots, so
other processes can consume memory between sampling and allocation. Native
Windows/macOS pressure handling remains manual qualification; Linux is checked
locally. Long stress/benchmark qualification is deferred per the maintainer.

Local validation: the RAM-allowance unit test and two targeted renderer tests
passed (including headless page growth, pressure eviction and a ceiling below
one page). Changed-crate all-target Clippy, formatting, diff whitespace checks
and offline cargo-deny checks passed. Release `mothref` and `xtask` built.
No full workspace tests, load timings, fuzz, soak or benchmark suite was run.
Native RAM-pressure behavior on Windows/macOS and long-running pressure recovery
remain unverified.

The settings follow-up passed three targeted tests: preference round-trip and
invalid-value fallback, RAM-fraction calculation, and English/Russian translation
coverage. Changed-crate all-target Clippy and formatting passed; release `mothref`
and `xtask` were rebuilt. No additional load, fuzz or stress runs were performed;
the settings panel was not manually exercised in a native window.
