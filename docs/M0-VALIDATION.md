# M0 validation — 2026-10-16

Environment: Linux x86_64, Rust 1.99 stable, AMD Radeon RX 5700 XT (RADV),
Mesa 26.2.2. These are development measurements, not universal hardware promises.

## Checks

- `cargo fmt --all --check`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass; also checked with
  `-D missing-docs`.
- `cargo test --workspace`: 12 tests pass, including command/inverse property tests,
  zoom anchoring, gesture behavior, PNG scanlines/Adam7, scratch checksums and every
  truncation boundary, a dhat allocation check, and a GPU pixel comparison.
- `cargo deny check`: pass. Transitive version duplicates remain reported as
  warnings; license and advisory findings were fixed rather than suppressed.
- Android core `cargo check`: pass for `moth-scene`, `moth-format`, `moth-io` on
  `aarch64-linux-android`. Android uses pure-Rust BLAKE3, without requiring the NDK;
  desktop builds retain SIMD.
- `cargo xtask corpus`: four image fixtures decoded. There is no `.moth` schema
  or importer yet, so board-file round-trips and the three board-reader fuzz
  targets cannot run in M0. Invoking those targets confirms they do not exist.
- `cargo +nightly fuzz run image_reader … -max_total_time=300`: two successful
  five-minute ASan runs, 3,796,794 and 128,181 executions. The second includes the
  final scratch parser and JPEG corpus. No crash or sanitizer failure.
- `cargo build -p mothref --release`: pass. Missing-adapter startup also exits
  nonzero instead of reporting success. Native Linux launch and screenshot
  inspection verified a loaded image. Empty and loaded windows settled after
  initial UI repaints; the 20/25-second trace captures stopped repainting at idle.

## Rendering and memory

The final `cargo xtask bench --quick --enforce` measured **4.093 ms p99** over
100 frames with 10,000 distinct resident images and **13.32 MiB** of texture pages.
The harness includes R-tree queries, ordered instancing, submission and GPU
completion at 1000×1000. It excludes window-system presentation, egui and decoding.
An earlier run during concurrent fuzzing/builds measured 8.358 ms and failed the
threshold; measure on an otherwise quiet reference system for comparisons.

Software rendering was tested with an isolated Mesa lavapipe driver under `/tmp`,
without installing or replacing system packages. The final CPU-adapter benchmark
reported **22.553 ms p99**, and the pixel golden passed on both RADV and llvmpipe.
The golden covers z-order, non-primary colors, odd image dimensions, mip padding
and texture release accounting.

The dhat test requires a 4K PNG/JPEG preview to stay below **8 MiB of transient
allocations** after input setup. The original PNG implementation failed at
33,656,848 bytes; streaming previews pass the allocation ceiling.

The 30-minute residency soak passed: **12,750 frames in 1,800 seconds**. RSS
started at **200,980 KiB**, settled at **199,112 KiB**, and stayed within the
16 MiB plateau threshold. This exercise covers the synthetic scene and image-cache
residency algorithm, rather than every native clipboard/desktop lifecycle. The
soak binary predates the final mip-sampling and UI-resource ownership edits;
those edits passed the final pixel comparison and tests, but a final-binary
release soak remains to be repeated.

## Paste target and remaining verification

The pipeline measurement dropped from **141.027 ms** with PNG staging to a latest
**68.237 ms** after raw staging and direct preview reuse (64.265 ms in durable
staging/preview preparation). Input is a deterministic noisy 3840×2160 RGBA image;
timing starts after OS clipboard transfer and includes the first completed GPU
frame. It therefore does **not** establish the 50 ms end-to-end paste target.
Durable writes remain enabled and the M0 exit criterion remains open.

Windows/macOS native launch, clipboard interoperability, pen identification and
pressure, real touch devices, forced live device loss, and suspend/resume need
manual platform testing. CI jobs exist for all three desktop OSes but were not
run remotely during this session. winit 0.30 lacks native Wayland file drops;
that M0 checkbox remains open. Boards are volatile until M1; there are no save/load
property tests before a board format exists.
