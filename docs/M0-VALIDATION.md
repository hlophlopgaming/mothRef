# M0 validation — 2026-10-16

## ZIP drops — 2026-10-26

Wayland now accepts KDE `zip:/absolute/archive.zip/folder/image.png` URLs,
including percent-encoded names, alongside ordinary local file drops. Selected
entries use the existing background decode and journal path; no archive contents
are extracted onto user paths. ZIP sources reject writes. Stored and Deflate
compression are supported, with a 1 GiB archive limit and the existing 64 MiB
image-input limit. The first `.zip/` separates the archive from the entry;
nested archives and directories whose names end in `.zip` are unsupported.
KDE's archive URL convention is documented by
[KProtocolManager](https://api.kde.org/kprotocolmanager.html).

Formatting and Clippy for `moth-platform` and `mothref` passed, as did `cargo deny
check` (warnings remain). Added regression tests cover mixed local/ZIP drops,
encoded names, invalid URLs, Deflate reads, input limits and read-only sources;
tests were not executed under the agent check policy.

Manual validation pending: in a rebuilt Linux Wayland app, drag multiple images
from a ZIP opened in Gwenview to a new board, then save/reopen and undo. Gwenview's
actual drag payload has not been captured. Windows/macOS native validation was
not run; archive URL support here is specific to the Wayland adapter.

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

## Clipboard staging review follow-up

Clipboard originals now use lossless PNG in the native application-data directory
(`mothref/recovery/<board UUID>/`). Each board has an 8 GiB disk budget and a
10,000-record limit. Confirmed discard removes only that board's directory on the
clipboard worker before exit. Crashes retain originals; automatic scene recovery
and cleanup of abandoned or legacy recovery directories remain M1 work. Previous
RGBA staging latency measurements do not measure this PNG implementation.

Windows/macOS application-data placement and native close behavior require manual
validation on those systems. The 30-minute soak and process-kill durability checks
have not yet validated this follow-up.

Follow-up checks on Linux: workspace tests (including board-isolated cleanup and
worker disposal acknowledgement), fmt, clippy and cargo-deny pass. Cargo-deny
still reports existing duplicate dependency warnings. The `dhat` check keeps
streaming PNG compression below 8 MiB extra allocations, excluding clipboard
pixels. `cargo xtask bench --quick`: 10,000 images, p99 4.098 ms, 13.32 MiB textures.
`cargo xtask paste`: 349.270 ms after clipboard transfer, 345.505 ms durable
staging for synthetic noisy 4K pixels. PNG compression is a latency cost; the
50 ms exit criterion remains unmet.

## Input lifecycle follow-up — 2026-10-16

The existing canvas provides pan mode and middle-button pan, pointer-anchored
wheel zoom, rubber-band selection in select mode, native file-drop import and
image clipboard paste (toolbar and Ctrl/Cmd+V). Touch uses the same pan/select
modes; two contacts feed a separate pan/pinch gesture layer.

Contact handling now ignores duplicate touch-down events, bounds pressure to a
finite 0–1 range, keeps a drag with its initiating pointer, and cancels a drag
as soon as a second contact starts a pinch. Contacts beginning outside the
canvas do not enter canvas gestures. Focus loss and suspension clear pending
canvas interactions; releasing a different mouse button cannot end a pan.

Automated input tests cover pinch deltas, duplicate contacts, gesture reset,
pressure bounds, mouse button ownership and focus cancellation. Workspace tests,
formatting, Clippy and cargo-deny pass on Linux; cargo-deny retains existing
transitive duplicate warnings. No renderer, decoder, scene or storage format
changes are part of this follow-up.

The two M0 checkboxes stay open: the current winit 0.30 adapter emits mouse and
touch events, but has no native pen-identification event, and native Wayland
file drops remain unsupported by this backend. `Kind::Pen` is available to a
future native pen backend; its presence alone does not establish pen support.
Native pen/touch devices and Windows/macOS interoperability were not manually
validated in this follow-up. The 50 ms paste target also remains open.

## Technical-debt fixes — 2026-10-16

Implementation at `36fbb0b`:

- Replaced render truncation with ordered submissions through the same bounded
  320,000-byte instance buffer. The 10,003-item pixel golden failed before this
  fix (the final green image disappeared) and passes afterward. App admission
  is independent of the render batch; waiting imports are capped at 1,024.
- Added an ordered fractional-index/UUID set maintained by commands. Property
  tests cover insert/remove, undo/redo, equal-z items and save/load reconstruction.
- Large baseline JPEGs are admitted by reduced-plane allocation; progressive
  coefficient storage is included in admission checks. Added synthetic large,
  CMYK and EXIF-oriented JPEG fixtures. Small previews no longer enlarge images.
- PNG and clipboard previews average source areas with alpha weighting, including
  Adam7. Tests cover a checker pattern, transparent colors and interlace parity.
  The dhat test includes the large JPEG and retains its 8 MiB preview ceiling.
- Added bounded native clipboard file-list paste and localized busy/unsupported
  feedback. Remote URL/HTML import is still unsupported and does not fetch data.

Validation:

- Formatting, workspace Clippy with warnings denied, workspace tests and
  cargo-deny pass. Existing transitive duplicate-version warnings remain.
- `cargo xtask corpus`: eight image/board fixtures pass.
- Android core build passes. All 13 supplied PNG/JPEG files in `images/` decode
  successfully at preview size; no supplied artwork was copied into the corpus.
- Discrete-GPU golden passes. Software golden passes using the existing isolated
  lavapipe driver under `/tmp/mothref-vulkan-software`; the default installed
  drivers expose no fallback adapter. No system driver was installed or replaced.
- `cargo xtask killtest 1000`: zero acknowledged commands lost.
- `moth_reader` fuzzing: 10,692,558 executions in 301 seconds, no failure.
  `bee_reader` and `pur_reader` commands were attempted but their M2 targets do
  not exist. Final `image_reader` fuzzing, seeded with the new JPEG fixtures:
  **23,246 executions in 301 seconds**, no crash or sanitizer failure.
  Log: `/tmp/mothref-debt-final-image-fuzz.log`.
- Combined render/persistence soak: **17,048 frames in 1,800 seconds**. RSS during
  the second half stayed between **165,096 and 165,112 KiB** (16 KiB spread).
  Log: `/tmp/mothref-tech-debt-soak.log`. The eight-hour release gate stays open.
- Native Linux launch/open/SIGTERM smoke passes with the normal backend and with
  X11 plus lavapipe. This does not qualify native clipboard, pen/touch, live
  device loss, Windows/macOS termination or interactive save/recovery UX.

Performance measurements on the shared development machine are noisy. One
`--enforce` run failed at **53.563 ms p99**. A subsequent adjacent comparison
measured **11.507 ms** for the fixes versus **14.449 ms** for original revision
`ab8ced5`, with **13.32 MiB** textures in both. These loaded runs do not qualify
an 8 ms exit criterion or a controlled 10% regression check. A shared-cache
comparison was discarded; release workspace artifacts were explicitly cleaned
before the adjacent comparison.

Latest noisy-4K paste measurement: **393.472 ms** after clipboard transfer,
including **390.439 ms** preparation/staging. The native 50 ms target is still
open; durable staging remains enabled. Logs and temporary benchmark binaries
under `/tmp` are local validation artifacts, not release qualification records.

## 250 ms paste follow-up — 2026-10-16

The maintainer accepts the earlier 11.5 ms p99 frame measurement and revises the
frame ceiling to 12 ms and immediate 4K paste target to 250 ms. The benchmark's
optional enforcement now uses those targets; historical results above are kept.

Clipboard PNG staging now uses bounded 256 KiB chunks instead of 4 KiB chunks.
Wholly opaque images are encoded as lossless RGB; images with any non-opaque
pixel retain RGBA. Conversion uses one RGB scanline, not another full image.
Pixel round-trip tests cover varying colors, odd row widths, and a single
non-opaque pixel. The existing dhat allocation check still passes. No decoder,
board format, journal ordering or fsync barriers were changed.

A pre-change measurement was **293.124 ms**. Three consecutive updated opaque
noisy-4K pipeline runs passed `paste --enforce`: **222.466, 245.491, 215.272 ms**
(median **222.466 ms**, about **24%** lower than that pre-change run). These
measurements include staging and GPU completion, but exclude OS clipboard
transfer and do not qualify every native clipboard backend or transparent image.
The system also produced second-scale staging outliers during the work. The
last verification measured **2006.995 ms** (2004.186 ms staging) and failed
`paste --enforce`. Thus the target is reached in the consecutive runs above,
but is **not consistently qualified** on this shared machine. File-write/sync
contention is suspected; it was not separately measured in that last run.

Formatting, Clippy, workspace tests, cargo-deny and corpus checks pass. Another
1,000-process kill run lost zero acknowledged commands. The
10,000-image render benchmark measured **6.665 ms p99**, with **13.32 MiB**
textures. An earlier concurrent build/load run failed the new 12 ms guard at
33.887 ms; this encoder-only change does not alter the draw path. No new
long-lived cache or decoder path was introduced, so the previous soak/fuzz
qualification was not repeated for this encoder optimization.

### 150 ms paste follow-up (2026-10-16)

The maintainer tightened the target to 150 ms. PNG output now hashes successfully
written bytes as it streams; staging reuses that hash for the preview instead of
hashing a temporary raw identity and rereading the PNG. Pixel fidelity, bounded
encoding, and file/directory sync barriers are preserved. No decoder or format
changes and no additional retained data were introduced.

Initial post-transfer runs: 204.196 and 198.770 ms, then 495.614 ms. Temporary
instrumentation on the 198.770 ms run measured 98.945 ms through encoding/writing,
165.306 ms through file sync and 171.381 ms through directory syncs. The temporary
instrumentation was removed. A different PNG filter was tried and reverted.
Final three `cargo xtask paste --enforce` runs all failed the new 150 ms guard:
6501.272, 1823.664 and 442.141 ms. These whole-staging outliers were not separately
profiled; their cause is not established. The target remains open.

Validation: workspace tests (including dhat), fmt, clippy with warnings denied,
and cargo deny passed. `cargo xtask killtest 1000` lost no acknowledged commands.
`cargo xtask bench --quick`: p99 5.113 ms, 13.32 MiB textures. No native clipboard
transfer timing or Windows/macOS manual validation was performed in this follow-up.

### TD-03 acceptance and TD-07 image compatibility (2026-10-16)

The maintainer accepts the approximately 200 ms paste result and closes TD-03;
the attempted 150 ms target is withdrawn. Historical outliers remain recorded
above. `paste --enforce` retains the prior 250 ms regression ceiling; this is not
a worst-case latency guarantee.

GIF previews now stream the first frame through the area accumulator, respecting
interlace order, transparency and frame offsets. BMP previews decode bounded
single-row records through the image crate for palettes and bitfield conversion.
They support uncompressed/bitfield layouts with standard DIB headers; RLE and
embedded JPEG/PNG are rejected, and the header/palette prefix is capped at 64 KiB.
The generic full-resolution fallback has been removed.

Before the row paths were implemented, enabling standard GIF/BMP decoding made
the 4K dhat test fail its 8 MiB limit. It now passes with both codecs included.
New synthetic corpus files cover ordinary/interlaced GIF, transparent offsets,
24-bit, indexed and alpha BMP, plus 4K GIF/BMP. Pixel checks compare against the
reference codecs and area averages; tests also cover BMP top-down rows, invalid
compression/offsets and truncated records. Original image bytes are unchanged.

At the initial GIF/BMP stage WebP awaited a maintainer decision. The maintainer
subsequently approved the bounded full-frame exception in ADR 0003.

TD-07 checks on Linux: workspace tests (including dhat), fmt, clippy with warnings
denied and cargo deny passed (existing duplicate warnings only). Core `moth-io`
Android cross-check passed. `cargo xtask corpus` validated 16 image/board files.
`cargo xtask bench --quick`: 10,000 images, p99 6.142 ms, 13.32 MiB textures.
A temporary release profiling example (removed after measurement) decoded the
synthetic 4K GIF/BMP at side 256 five times: GIF median 40.786 ms
(range 38.145–42.646), BMP median 39.508 ms (38.455–40.346). These codecs were
previously unavailable; the full-frame fallback comparison is an allocation
regression test, not a previously shipped import path. Native UI/manual tests
were not repeated because dispatch and platform code are unchanged by TD-07.

The completed TD-07 follow-up also adds static lossy/lossless WebP, transparency
and EXIF orientation. Hard admission limits are 8 MiB encoded input and 8 MiB
full RGBA frame, with embedded VP8/VP8L dimensions checked against the canvas.
The decoder dependency does not enforce an allocator-wide working-memory cap;
this limitation is explicit in ADR 0003. A largest-admitted 2048×1024 WebP passes
the dhat additional-peak gate of 64 MiB; a forged 4K header is rejected before
pixel decoding. Pixel/orientation checks and invalid internal-dimension checks
pass. Native clipboard behavior and persistence are unaffected by the codec paths.

Final WebP-enabled workspace tests, clippy, cargo deny and Android core checks
passed. The successful-import corpus now validates 21 files. Static WebP
animation rejection also has a negative fixture/test. A temporary release
profile decoded the maximum-admitted solid 2048×1024 WebP at side 256 five times:
median 14.966 ms, range 14.160–18.487 ms; the temporary example was removed.

Final sanitizer fuzzing: `image_reader` completed 1,100 runs in 306 seconds on a
separate temporary corpus seeded with JPEG/PNG/GIF/BMP/WebP and mutations, with
no crash or sanitizer finding. `moth_reader` completed 12,613,851 runs in 301
seconds without a finding. `bee_reader`/`pur_reader` were attempted but the targets
are absent (their importers belong to M2). Final `cargo xtask bench --quick
--enforce`: p99 6.466 ms, 13.32 MiB textures, accepted 12 ms guard passed.

## URL/HTML clipboard import — 2026-10-17

Paste now falls back from native files/images to a direct HTTP(S) image URL or
HTML's first `img[src]`. The HTML parser decodes entities and resolves relative
sources with `base[href]` or a supplied `SourceURL` header. No page, script, CSS,
`srcset`, data URL or link preview is loaded. HTML imports one image per paste;
a plain page URL must itself return a supported image. Native image/file formats
retain precedence.

Only an explicit Paste request starts HTTP work, on the existing bounded
clipboard worker. Captured text/HTML is limited to 1 MiB (after the native backend
returns it); URLs to 8 KiB; downloads to 64 MiB and five redirects, with a shared
20-second HTTP deadline. Redirect targets receive the same URL validation;
embedded credentials, non-HTTP(S) schemes and HTTPS downgrades are rejected.
TLS validates against OS trust roots. Decoder admission limits still apply.
Errors do not include the URL or query string.

Successful downloads preserve original encoded bytes in private, synced recovery
files under the existing 8 GiB/10,000-entry staging budgets. The existing command
path journals originals and insertion before updating the board. Tests use a
local HTTP server to check redirects, redirect loops, malformed/oversized/partial
responses, byte preservation and command-journal reopening. Parser tests cover
HTML entities, relative URLs, URL schemes/credentials and text/URL limits.

Native Windows/macOS/Linux clipboard interoperability was not manually exercised
for this follow-up. Backend-provided HTML metadata varies; relative URLs without
a usable base are rejected. Browser authentication/cookies and page scraping are
outside this import path. Existing board-reader fuzzing does not cover HTML.

Final Linux checks on 2026-10-17: formatting, workspace Clippy with warnings
denied, all 45 workspace tests (including the allocation check and GPU golden),
and cargo-deny pass. Cargo-deny retains transitive duplicate-version warnings.
`cargo xtask corpus` validates 21 files; `cargo xtask killtest 1000` loses no
acknowledged commands.

The benchmark run during simultaneous builds/fuzzing measured 50.019 ms p99,
and paste measured 1100.949 ms. With compilation finished, `bench --quick
--enforce` passes at **5.473 ms p99**, with **13.32 MiB** textures; paste measures
**228.234 ms** after clipboard transfer (**224.571 ms** durable staging). The
board-reader fuzzer was still running during these repeat measurements. These
are development checks, not controlled before/after regression qualification
or native URL-paste latency measurements.

The required `moth_reader` sanitizer fuzz run completed **10,260,591 executions
in 301 seconds**, without a crash or sanitizer finding. `bee_reader` and
`pur_reader` were attempted but remain absent until their M2 importers exist.
No new dedicated HTML fuzz target or native clipboard manual test was run.


## TD-09: native Wayland drops and tablet input

2026-10-17, EndeavourOS, KDE/KWin 6.7.4, native `wayland-0`:

- The adapter binds `wl_data_device_manager` and `zwp_tablet_manager_v2` on
  winit's display, with I/O on native workers and bounded queues/transfer data.
  Native protocol fixtures use a Wayland server and a guest backend, exercising
  actual event decoding and descriptor passing, with a concurrent host reader
  on the same display, rather than injected file paths.
- Automated drop coverage: URI MIME acceptance and copy negotiation; two local
  files with percent-encoded spaces; drop followed immediately by `leave`;
  successful `finish` only after EOF; rejection and destruction of non-URI
  offers. URI tests cover remote/malformed URLs, NUL paths, invalid UTF-8,
  empty transfers, stalled-source timeout and the 1 MiB limit.
- Automated tablet coverage: tool creation, native pen kind/ID, logical surface
  coordinates, 0.25/0.75 pressure, down/move/up, proximity cancellation and
  removal. Up preceding proximity-out in one frame is preserved. Transfer-worker
  shutdown wakes an idle native reader. UI tests ensure cancellation cannot
  click a toolbar action or release a different pointer; normal releases and
  up-plus-proximity-out in one batch still click. Synthetic touch tests retain touch kind,
  pressure, scaling and all contact phases; pan/pinch remains a separate layer.
- The maintainer confirmed that dragging **two files from Dolphin imported both**
  into the new native Wayland window.
- Real pen: kernel/Wayland discovery reports **Huion Tablet Monitor Pen**
  (USB `256c:006d`). The maintainer reports that the pen appears to work. The
  running application captured native `Kind::Pen` events with a stable tool ID,
  hover pressure zero, nonzero varying contact pressure and down/up/proximity-out
  events. The trace reached approximately **0.70** pressure during the initial
  check. This verifies native classification and measured pressure, rather than
  merely exposing `Kind::Pen` in the API. Drawing remains M4 functionality;
  current pen actions are selection and toolbar interaction. Full button,
  calibration and interruption combinations were not independently observed.
- **Real touch:** the maintainer has no touchscreen available. No real-device
  two-finger or pen-plus-touch validation is claimed. TD-09 and the grouped M0
  input qualification checkbox remain open for this reason.
- Windows/macOS: no native manual run in this session; their existing winit
  mouse/touch paths are unchanged, and this Linux backend does not add pen
  identification there. X11 pen identification is also outside this backend.

Logs: `/tmp/mothref-td09-native.log` (pen/device interaction),
`/tmp/mothref-td09-tests.log`, `/tmp/mothref-td09-clippy.log`, and
`/tmp/mothref-td09-soak.csv` (RSS samples). These are local validation artifacts,
not redistributable device fixtures.

TD-09 checks: workspace formatting, clippy with warnings denied, workspace tests,
`cargo deny check` (existing duplicate warnings), and Android core checks pass.
The required `moth_reader` fuzz command completed **12,882,900 executions in
301 seconds** without a failure. The repository has no `bee_reader` or
`pur_reader` targets; both prescribed commands were attempted and failed for
that reason. A Windows cross-check was attempted but this Linux host lacks the
MSVC `ml64.exe` assembler required by `ring`; Windows CI/manual validation is
still needed. No scene, renderer, decoder or board-format implementation changed
for TD-09, so benchmark/corpus commands are not required by the change scope.

Final-build launch/shutdown smokes passed on native Wayland and X11, each using
an isolated temporary data directory and exiting successfully on SIGTERM. The
maintainer's interactive board remains open. A short same-host Wayland comparison
used an isolated source snapshot with only `input.rs` and `app.rs` restored to
their pre-TD-09 versions, keeping the existing working-tree changes elsewhere.
Two ten-second samples per build measured 3.0–3.3% CPU before and 3.4–3.7% after,
with median RSS 137,892 versus 138,298 KiB and 26 versus 28 threads. These short
samples are **not** a zero-idle or regression qualification; the previous adapter
also has nonzero CPU on this host. Native worker state is bounded and the added RSS in
this probe is about 0.4 MiB. Raw samples: `/tmp/mothref-td09-compare.json`.

The additional `cargo xtask bench --quick` run passes: **10,000 distinct images,
100 frames, p99 5.253 ms**, 13.32 MiB textures, AMD Radeon RX 5700 XT (RADV
NAVI10). This is below the 12 ms M0 target. Log:
`/tmp/mothref-td09-bench.log`.

The maintainer stopped the long check after approximately **23.5 minutes**.
The interactive build stayed at **170.46–178.30 MiB RSS**
after warm-up, with 28 threads. The full 30-minute soak was **not completed**;
this does not close that qualification gate. The interactive build was retained
throughout; final UI cancellation and shutdown refinements were checked separately
by protocol/UI tests and isolated final-build launch/shutdown smokes.


### Foreign-board conversion (2026-10-26)

- `.bee` / `.pur` open as individually editable scene items with no working save
  destination. The conversion modal offers Save As .moth and Continue without
  saving; the save-path modal repeats the preservation and limitations report.
  Close uses the existing save/discard confirmation for dirty boards.
- The storage worker rejects non-`.moth` save destinations before opening them.
  Source files are read-only inputs; originals are streamed into the private
  journal and copied unchanged into native saves.
- Board admission does not use `moth_io::MAX_INPUT_BYTES`: PureRef and BeeRef
  accept at most 4 GiB, with 8 MiB / 8192-item import metadata budgets. SQLite
  records remain limited to 32 MiB. Native desktop board admission is 8 GiB;
  native record allocations remain bounded by `board::MAX_RECORD` (256 MiB).
  Existing checksums, geometry validation and decoder budgets remain active.
- Added persistence regressions cover both formats' source-byte preservation,
  native save/reopen, rejection of foreign save destinations, and recovery after
  killing an importer subprocess following durable import and a subsequent edit. A sparse BeeRef board
  above the individual-image cap imports successfully; oversized BeeRef/native
  boards are rejected before parsing.
- Passed: formatting and workspace Clippy with warnings denied; 19 platform
  persistence tests; 13 importer tests; the targeted workspace
  conversion-state test; corpus validation (29 fixtures, including PureRef native
  round-trips); a 90-second BeeRef fuzz run (2,190,344 inputs). The first sandboxed
  fuzz run completed its inputs but LeakSanitizer could not finish under ptrace
  restrictions; the unrestricted rerun exited successfully with sanitizers active.
- Wider suites are not green: the UI count assertion expects a zero-image count
  even though the implementation and an earlier assertion omit it. The serial
  application run also failed `note_commit_at_35_percent_preserves_editor_text_height`
  and aborted at `settings_columns_stack_their_controls_after_resize` (missing
  visible label, followed by an unapplied egui texture delta during unwinding).
  The conversion-state, unsaved-close and save-before-discard tests passed in
  that run. Interactive dialog validation on Linux, Windows and macOS remains
  manual work; tier-3 checks were not run.
