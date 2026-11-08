# M2 editing and export validation — 2026-10-18

## Accepted editing polish — 2026-10-25

The maintainer accepted the editing-polish follow-up. Board tools is replaced by
shared action surfaces: the bottom toolbar, selection bar, grouped context menu,
command palette and focused editor/arrangement/picker/export windows. Corner
handles now scale single or multiple items proportionally. See the
[native review and limits](plans/alpha9-review/README.md#native-follow-up--2026-10-25)
for this pass. The older notes below describe the initial implementation.


Scope: the first five M2 roadmap entries and the parity follow-up below. Existing notes, rotate/flip commands,
normalization, basic arrangement, and the empty-board drop zone are reused.

Board tools provides a touch-accessible inspector, arrangement, and export controls.
Select an item, open **Board tools → Edit selection**, edit its frame or content
transform, then **Apply**. Changes are journaled together as one undo step. Cancel
restores the inspector draft. Geometry is numeric; direct handle dragging is not
added by this slice. The gap, arrangement prompt threshold/default, auto-arrangement,
wheel behavior, resize scaling, undo depth, theme and interface scale are persisted
through the existing storage worker. View controls remain separate from board data.

**Pick image color → HEX / RGB / HSV** arms one canvas click/tap, inversely maps the image's
frame/content transforms and copies the selected representation of the source-preview color without applying
item opacity/grayscale. The status reports all three representations. Scene and clipboard export and the picker use up to 2048-pixel
source previews; PNG/SVG/PDF scene output is limited to a 4096-pixel longest side and
64 MiB of generated SVG. Exporting original files streams unchanged bytes with a
64 KiB buffer (up to the existing 256 MiB original-record limit). An original
collection creates a new directory and deduplicates identical source hashes.
Existing files/directories are refused. A failed new export may leave a partial
output; the board and existing files remain untouched.

Filename ordering uses filenames captured for newly opened/dropped images. Older
boards and foreign imports without filenames retain empty names and a stable UUID
tie breaker. Schema 1–3 load with opaque/color appearance and empty names; first
save atomically upgrades the old stream to schema 4. Frozen old-version fixtures
are retained, and `corpus/moth/v4.moth` covers the new fields.

Checks:

- Workspace formatting, Clippy with warnings denied, and workspace tests.
- Property tests for command undo identity and checkpoint round trips, including
  appearance and filenames; every-byte checkpoint truncation tests.
- Schema-3 read/migration and storage-worker save-upgrade test.
- Geometry reset regression: reproduced loss of appearance, then verified that
  both the menu and shortcut preserve opacity/grayscale.
- Headless GPU golden samples for half opacity and grayscale, alongside the
  existing frame/content rotation, flip and clip cases.
- PNG/SVG scene/selection/region export tests, original byte preservation,
  directory export and exclusive-create refusal.
- Existing decoder dhat allocation test (part of workspace tests).
- Corpus round trip: 28 image/board fixtures.
- `cargo deny check`: advisories, licenses, bans and sources passed.
- Reader fuzzing: 1,272,081 runs in 31 seconds, no crash, seeded from schemas 1–4.

- `cargo xtask killtest 50`: 50 process kills, no acknowledged commands lost.
- `cargo xtask bench --quick`: AMD Radeon RX 5700 XT (RADV NAVI10),
  10,000 distinct images, 100 frames, p99 **7.548 ms**, textures **13.32 MiB**.
- `cargo xtask soak 300 5`: **2,815 frames in 300 seconds**; RSS settled at
  **159,844 KiB** (about 156 MiB), flat from 60 seconds through the end.

Native manual tests were not run on Windows/macOS, physical touch/pen devices,
or the OS clipboard. The clipboard backend stays alive with its board worker;
clipboard retention after closing that tab depends on the OS clipboard manager.
Full-resolution tiled scene export and exact original-pixel sampling remain future
work; originals exported to files preserve full resolution today.

Tier-3 release qualification (1,000 kills, long fuzzing, long soak and enforced
performance gates) remains for CI/maintainer execution, per AGENTS.md.


## M2 parity follow-up

Board tools now exposes alignment, distribution, center stacking, scale normalization,
original-backed frame/crop/all resets, individual rotation/flip resets, layer steps and extremes, and touch-accessible duplication
and nudges. Arrange/normalize/align act on all items when nothing is selected. Layouts
support layer, filename, UUIDv7 insertion-time and random order; repeat a layout with
the same item set to reverse it. Pack all and fit leaves the scene metadata unchanged
except for journaled positions. Auto-arrangement stays inside the insertion undo group.
Grouped undo/redo now applies dependent inverses in reverse order; a regression test
covers insert + move, undo, redo and undo again.

Alt+drag duplicates the selected items on release; ordinary drags move the selection.
Arrow keys nudge by one world unit, Shift+arrows by ten. Board tools provides buttons
for pointer/touch use. Canvas locks belong to each tab and gate the shared mutation
path, import, paste and note creation; viewing, selection, exports and navigation remain
available. The interface preferences survive tab changes. Modified settings show a localized
asterisk marker and can be reset together. Double-click an image to
focus it, then double-click it again to restore the camera.

PDF uses one raster page with the same crop, selection, preview and resolution budgets
as PNG. Transparency is composited over white. ZIP stores deduplicated, unchanged
original bytes with hash-based filenames and validates each source hash; an empty
selection exports all originals, otherwise it exports selected originals. Existing
destinations are refused. Coordinate picking reports original-pixel X/Y, image name
and dimensions in the status bar; the selection badge supplies selection dimensions.

The overall M2 checkbox remains open. Remaining command-level gaps include file-path
ordering/provenance and opening source locations, automatic bring-to-front, optional scrollbars/menu visibility, picking outside
the window, color-gamut visualization, linked-image relinking, optional import downscale,
and source-location workflows. The picker rows remain unchecked because outside-window sampling
and source information are not implemented. System-viewer launch is implemented below; native OS qualification is pending.

Native Windows/macOS, physical touch/pen, native clipboard/color copying and native
viewer workflows are not manually qualified by this follow-up. Tier-3 gates remain
CI/maintainer work.

Follow-up checks (2026-10-18): workspace Clippy with warnings denied, workspace tests
(including headless rendering goldens and decoder allocation checks), corpus round-trip
(28 fixtures), and dependency license/advisory checks passed. Property tests cover the
new edit commands' undo identity and checkpoint round trips. PDF tests verify page
bounds, cross-reference offsets and raster bytes; ZIP tests read entries back with CRC
validation and compare original hashes. Preference tests cover persistence, bounded
reads and injection rejection; wheel tests cover zoom/pan/disabled and modifier routing.

The 50-kill journal test passed. On the same RX 5700 XT adapter, the idle-machine
final quick benchmark measured **7.360 ms p99** over 100 frames and **13.32 MiB** textures
(previous recorded run: 7.548 ms; an earlier follow-up sample was 4.521 ms, so
these short runs are timing samples, not a claimed speedup). No hot-path dependency was added: ZIP is used only by storage-worker export.

The five-minute soak completed **2,849 frames in 300 seconds**, with RSS stable at
**195,984 KiB** (about 191 MiB) from 55 seconds through the final sample. Original-backed
reset and preference-marker additions made afterward add no retained buffers or caches;
their regression/formatting/Clippy checks are included in the final check run.

## Square optimal packing

Optimal uses all selected displayed bounds to search a compact square. It preserves
image sizes, aspect ratios and existing rotations; neither viewport dimensions nor
camera zoom enters the packing API. It always uses zero spacing. Horizontal,
vertical and grid retain the configured gap (24 world units by default).
The bounded greedy search is approximate; unusually shaped sets can leave holes.
Distinct vector icons precede arrangement names in the inspector, context menu,
import prompt and default-method choices. Existing grouped commands retain undo,
journal ordering and save/load behavior.

Regression coverage includes exact square fills for equal and mixed image sizes,
independence from the flow-gap setting, rotated non-overlap for every heuristic,
atomic rejection of invalid layouts, undo and checkpoint round-trips.

Validation for this change: `cargo test -p mothref -p moth-ui` passed all 82 tests,
including the packing property tests and save/load/undo checks.
`cargo fmt --all --check` and Clippy for both changed crates also passed.
Manual GUI inspection and packing benchmarks were not run.

Grid uses the square packer with a shared gutter added to each displayed bounding
box. Unlike Optimal's zero padding, this gives the same edge spacing horizontally
and vertically, without resizing images or padding short images to uniform cells.
The maintainer explicitly chose this mosaic behavior over proportional resizing.
Fixed column controls apply only to horizontal/vertical layouts; Grid chooses its
shape from the entire selection. The icon and help text reflect this distinction.
Regression coverage includes a tall image beside two shorter images with exact
24-unit horizontal and vertical gaps, square fills, rotated gutters and grouped
undo/checkpoint round-trips.

A saved `arrange_gap=0` from the earlier shared setting now loads as the default
24-unit gutter. Both settings controls require a positive gap; Optimal continues
to ignore that setting and packs without spacing. A regression loads the legacy
zero preference and exercises Grid and Optimal through `Tools::arrange`.
Existing board positions change only when the user applies an arrangement again.

Mosaic validation: all 84 tests in `mothref` and `moth-ui` passed, including the
regression for equal horizontal/vertical edge spacing. Formatting and Clippy also
passed. Manual GUI inspection was not performed.


## M2 follow-up — 2026-10-26

- Open original: one selected image, through the context menu, export window or
  palette. The storage worker streams and verifies a copy; OS associations launch
  through `moth-platform::Platform`. A small feedback window shows progress and
  immediate failures. The private viewer cache is one original (at most 256 MiB)
  per board. Unsupported original encodings can still be exported as PNG.
  Linux/macOS launcher waits are capped at 100 ms on storage, with at most four
  background child waiters. Later launcher failures are logged.
- Sampling: Settings › Canvas owns the default; selection/context menus and the
  item editor own overrides. Smooth stays the default. Automatic uses nearest
  neighbor at two screen pixels per original pixel. Both filters reuse residency,
  mip selection and tile coordinates; no extra texture or cache is introduced.
- Notes: auto-size on creation/editing; dragging a single note's corner or changing
  its width in the editor enables wrapping without shrinking its font. Grouped
  scaling scales frames and fonts together. The editor also exposes wrapping and
  text size. Journaled command groups retain Undo/Redo; released previews keep the
  final text layout until acknowledgment. Exports use fixed fonts and wrapped lines.
- Schema 5: see FORMAT.md for the 4 → 5 migration. Frozen schemas 1–4 remain in
  the corpus; the generator writes only the new schema-5 fixture.

Added regression coverage: sampling inheritance and threshold, a mixed smooth/sharp
headless image comparison, preference persistence, schema-4 migration, malformed
schema-5 fields and truncation, scene undo/checkpoint properties, note editor and
paint layout, single-note wrapping, grouped text scaling, Unicode export wrapping,
and viewer-copy source preservation.

Validation: `cargo fmt --all --check`, workspace Clippy with warnings denied, and
`cargo deny check` passed; deny reports existing duplicate-dependency warnings.
`cargo xtask fixture` generated the new fixture. Tests, corpus round trips, fuzzing,
kill tests, soak and benchmarks were not run under AGENTS.md §3. Before release,
run the affected format/corpus tests, 90-second moth_reader fuzzing, headless image
checks, the short benchmark and five-minute soak; persistence also needs the
50-kill check. Tier-3 evidence belongs to CI/the maintainer.

No native system-viewer or GUI qualification was performed on Linux, Windows or
macOS for this change. Export text uses the bundled Noto Sans font; fallback glyphs
can differ from the canvas. Viewer copies are disposable; export a file to retain
external edits. Old builds cannot read schema-5 saves.

## Screen color picker — 2026-11-06

Nested KWin 5.27 stand, MothRef through XWayland (`stand.sh kde app x11`):

- Pick image color, press on the board, drag over another X11 window (white): the
  panel showed “Under the pointer #FFFFFF” live; the release copied `#FFFFFF` and
  showed “Copied #FFFFFF”. Rootless XWayland refuses `GetImage` on the root window
  (BadMatch), so the sampler reads the top-level window under the pointer instead;
  over Wayland-native windows nothing is readable and no color is shown.
- A click on an image inside the window still samples the original, as before.
- Pick a color from the screen… without a portal: the notice “This desktop has no
  color picker to open…” appears; with `xdg-desktop-portal` and the KDE backend
  started by hand the request was accepted but the nested KWin showed no picker, so
  the portal answer itself is **not qualified** here.

Not run here, to check natively:

- Windows: drag out of the window over another app and a second monitor (left of
  the primary, negative coordinates); per-monitor DPI.
- macOS: Pick a color from the screen… opens `NSColorSampler`; Esc cancels without
  a notice; the color is sRGB.
- GNOME and Plasma Wayland: the portal picker opens and its color is copied.


## Import completion latency — 2026-11-08

Measured before and after replacing the quiet timer with explicit file-list
completion for Wayland drops, clipboard lists, file-picker results and command-line
images. The headless probe runs the actual decode workers, journal worker,
workspace completion handling and egui arrangement prompt. It records the time
when `Intent::Arrange` is emitted; it does not measure GPU uploads, presented
frames, full-resolution baking or the native file-transfer duration.

Command: `cargo test -p mothref measure_import_latency -- --ignored --test-threads=1`.
The ignored probe is a manual measurement, not a timing assertion in ordinary CI.
It writes `/tmp/mothref-import-latency.csv`. Each case imports the repository's
`images/foxes/fox-in-snow.jpg` repeatedly into a fresh temporary board with automatic
arrangement enabled. Five runs per count, medians below; the same test profile and
input were used before and after. No other build, fuzzer or benchmark ran during
the measurements. The after probe marks the known file list complete, as the
production list receivers now do.

| Images | All items journaled, before / after (ms) | Arrange requested, before / after (ms) | Extra wait, before / after (ms) |
| --- | --- | --- | --- |
| 1 | 299.548 / 288.123 | 599.636 / 288.131 | 300.864 / 0.008 |
| 10 | 938.022 / 923.399 | 1238.938 / 923.408 | 300.917 / 0.011 |
| 100 | 8463.178 / 8356.592 | 8763.341 / 8356.614 | 300.218 / 0.018 |

The fixed change removes the approximately 300 ms completion wait; the smaller
decode/journal differences are run-to-run variation, not a decoder improvement.
The sub-millisecond after values are time spent in the headless prompt code,
not a promise of sub-millisecond screen response.

Regression coverage checks that a native drop can pause with idle workers between
400-item chunks without offering a partial arrangement, that the end marker makes
all 1,014 ids ready immediately, and that a complete list still waits for pending
worker work. A native Wayland protocol fixture checks that start/end markers
surround the paths. winit-only drops without a list-end signal retain the 300 ms
quiet fallback. No manual desktop qualification on Linux, Windows or macOS was
performed for this change.

Validation passed: `cargo test -p mothref app::document::tests -- --test-threads=1`
(15 tests), `cargo test -p moth-platform input::wayland:: -- --test-threads=1`
(11 tests), `cargo fmt --all --check`, and
`cargo clippy -p mothref -p moth-platform --all-targets -- -D warnings`.

## Import backpressure and missing images — 2026-11-08

The earlier latency probe did not cover a full storage queue: 100 copies of a
large JPEG decoded slowly enough for the journal to keep up. A separate shared
import-path bug discarded images when decoding outran storage. `complete` removed
the pending decode and attempted `persist`, whose `try_send` rejected the insert
when the 32-slot storage request queue was full. That insert was never retried.
The arrangement prompt could then refer to ids absent from the scene.

Reproduction: `cargo test -p mothref large_file_list_keeps_every_image_under_storage_backpressure -- --test-threads=1`.
The test imports 1,316 copies of `corpus/images/rows.png` through the actual decode
pool and workspace. Its wake callback pauses the real storage worker after its
first acknowledgment, allowing the bounded request queue to fill before storage
resumes. Before the fix the test failed with **32 scene items instead of 1,316**;
after the fix all **1,316 scene items and 1,316 prompt ids** are present.

Decode and clipboard completion drains now require three free storage request
slots, covering the possible EndBatch, BeginBatch and Apply requests of one
completion. Otherwise results remain in their existing bounded queues, which
backpressure the producers without blocking the UI or allocating a new backlog.
Journal acknowledgment still precedes scene insertion. The change is shared by
native drops, file-picker imports and clipboard imports.

## Large-selection move completion — 2026-11-08

A drag release becomes one `ApplyBatch`, but previously that batch called the
single-command durable path for every member. Moving 1,316 images therefore
waited for 1,316 `fsync` calls (1,317 with the group frame). The fix uses the existing
pending-record path for preceding members and syncs with the last command before
returning `AppliedBatch`. Record checksums, footer layout, validation, inverses and
undo grouping are unchanged. Undo/redo replay retains its existing behavior.

Measurement: `cargo test --release -p moth-platform measure_bulk_moves -- --ignored --test-threads=1`.
The manual probe prepares real image items sharing a generated original, commits
an initial board, then measures the real `State::request(ApplyBatch)` move path.
Boards live under a disposable `target/mothref-move-bench-<UUID>` directory on the
workspace's Btrfs disk, not `/tmp` (tmpfs). Each move shifts all members by one
unit. Five moves per case; medians below. No other build, test, fuzzer or benchmark
ran during either measurement. Timings exclude board setup, GPU drawing and UI
application of the completion. Test-only instrumentation counts successful
`commit` barriers; it does not change production builds.

| Images | Selected separately, before / after (ms) | Group, before / after (ms) |
| --- | --- | --- |
| 1 | 2.852 / 2.591 | 4.919 / 2.325 |
| 100 | 279.363 / 3.374 | 302.003 / 4.034 |
| 1,316 | 4618.888 / 21.192 | 4413.476 / 224.534 |

The group includes its frame in addition to the listed image count. Sync counts
changed from one per command to one per batch. Remaining grouped-move CPU work
includes maintaining the group's derived frame; this change does not optimize
that scene code. Raw samples were written to `/tmp/mothref-move-before.csv` and
`/tmp/mothref-move-after.csv`.

Regression coverage checks that three inserts have a single durable batch commit
and are readable from the journal when acknowledged. A separate child-process
writer repeatedly moves forty members in batches; fifty process kills check that
all acknowledged member positions survive recovery, including when the next
batch was interrupted. Native desktop/GPU qualification was not performed here.

Validation passed: `cargo test --release -p moth-platform boards::tests -- --test-threads=1`
(24 passed, the manual timing probe ignored), including the fifty interrupted move
writers; `cargo xtask killtest 50` (no acknowledged commands lost);
`cargo fmt --all --check`; and
`cargo clippy -p moth-platform -p mothref --all-targets -- -D warnings`.

## Large-group release preview — 2026-11-08

A release with more than 300 commands skipped `DragMotion`'s retained item
snapshots. Until journal acknowledgment, both canvas outlines/title chips and GPU
instances could therefore fall back to the old scene geometry. Applying a group's
resting-frame command could also leave an old-frame item transition after the
members had moved.

Large moves now retain UUID/target-rectangle pairs (up to the one-million-item
scene admission bound, at most 32 MiB), and UI/GPU presentation reads those target
rectangles until acknowledgment or rejection. They retain no item text, names or
pixels. The temporary geometry is freed at handoff, and transitions for the moved
ids are removed so the group frame does not animate from its old location again.
Small-selection animations retain their existing 300-item budget. A pending large
move does not keep the repaint loop running, and the scene is unchanged until its
journaled commands arrive.

Reproduction: `cargo test -p mothref large_group_release_never_returns_its_frame_to_the_old_position -- --test-threads=1`.
Before the fix, the 1,316-image group fell back to frame origin (-24, -36) instead
of the drop origin (76, 14). The regression now checks groups with 2, 299, 300 and
1,316 images, with regular and reduced motion. Every member and group frame holds
its target coordinates during a simulated delayed acknowledgment, stale group
transitions are cleared at acknowledgment, and a rejected move discards its view
geometry. The scene remains at its original coordinates while pending.

Validation: all 10 `app::motion::tests` passed, including the headless transform
golden; the canvas selection handoff regression also passed. The isolated
`cargo xtask bench --quick` completed on llvmpipe: 10,000 images, 100 frames,
p99 26.921 ms, textures 13.32 MiB; 4K JPEG tile medians were 13.128 ms uncached and
0.017 ms cached. These are software-adapter measurements of the existing render
benchmark, not native desktop qualification or a before/after measurement of this
presentation change. No build, fuzzer or other benchmark ran alongside it.
Manual desktop validation was not performed.

## Large-board save latency and progress — 2026-11-08

The first save previously synced each new original separately, then calculated
its compaction threshold from blobs present before the save. A large new file
therefore underwent an immediate extra full read/rewrite. The threshold also
omitted the live checkpoint, which caused unnecessary compaction of large note
boards. Saves now append only distinct originals referenced by the live scene,
sync them with the checkpoint, and include new blobs and snapshot bytes in the
live-size estimate. Backups, checksum verification, atomic compaction and the
independent working journal are retained. The optional preview is still synced
separately.

The manual probe is
`cargo test --release -p moth-platform measure_large_board_saves -- --ignored --test-threads=1`.
It prepares 1,000 distinct PNG originals of 64 KiB each (62.5 MiB total), journals
them, then measures the first save and a repeat explicit save with backup enabled.
Files live in a disposable `target/mothref-save-bench-<UUID>` directory on the
workspace Btrfs disk. Input generation uses a disposable `/tmp` source so fixture
rewrites do not dirty the destination directory. Every saved board is read back
and checked for 1,000 scene items and 1,000 originals outside the timed section.

A temporary test-only copy of the original save method from HEAD supplied the
baseline; it was removed after measurement. Both versions used the same updated
probe, one warmup board and five measured boards, with the same release profile.
No other agent build, test, fuzzer or benchmark ran alongside either measurement.

| Save | Before median (ms) | After median (ms) |
| --- | --- | --- |
| First save | 8696.123 | 326.337 |
| Repeat save, including backup | 236.801 | 186.768 |

Baseline first-save samples were 13268.091, 4270.429, 8696.123, 9683.858 and
3986.558 ms; optimized samples were 399.831, 322.881, 353.791, 302.511 and
326.337 ms. Earlier exploratory cohorts showed large disk-sync outliers even
without per-image syncs; those cohorts were not used as acceptance results. The
repeat-save difference is small relative to observed filesystem variability and
is not attributed to removing per-original syncs, since no originals are new on a
repeat save. These measurements use small decoded fixtures with padded original
bytes; larger files and expensive thumbnail codecs can take longer. Raw final
samples are `/tmp/mothref-save-before-final.csv` and
`/tmp/mothref-save-after-final.csv`; stage timings are
`/tmp/mothref-save-stages-final.csv`.

Manual-save progress appears in the shared non-interactive notice above the
bottom toolbar, in both Collect and View modes. It reports preparation, backup,
image copying with a count, checkpoint write, compaction when needed, preview and
finishing. A success or failure notice follows; autosaves keep their quiet UI.
Progress is best effort through the existing bounded result channel and never
acknowledges the save or reduces the outstanding request count.

Regression coverage checks a single checkpoint/preview sync pair for new
originals, no redundant first-save or metadata-only compaction, omission of deleted
originals on Save As, actual image progress counts, and the rendered localized
counter while the real worker is paused. It also checks clearing progress after
success and failure without marking a failed save clean. Fifty subprocess kills
interrupt initial and subsequent saves: all journaled images and every
acknowledged saved scene remain recoverable with their originals.

Passed: 17 document tests, 28 storage tests (including the save-kill check), the
enhanced first-save test, two localization tests, and `cargo xtask killtest 50`
(no acknowledged commands lost). Manual validation of the user's original board
and a native desktop save was not performed.

Final checks also passed: formatting, `git diff --check`, and
`cargo clippy -p moth-platform -p mothref -p moth-ui -p xtask --all-targets -- -D warnings`.


### Optional backups and missing-board recovery (2026-11-08)

Backup creation now defaults off; the General settings schema stores the global
preference, and library metadata stores an optional per-board override. Manual
saves resolve the preference on the storage worker. Timed autosave stays separate.
The library checks missing paths for an existing backup on its own worker and
shows a recovery badge and button. The registered action opens the latest copy
into an independent journal as an unnamed dirty board; Save As leaves both source
files intact. Old copies remain visible even when creation is disabled.

Regression coverage added for default-off behavior, global/per-board precedence,
legacy metadata, override persistence/relinking, missing-card availability,
three-copy rotation, source/original preservation, copied image originals and the
library recovery affordance. Existing explicit-backup tests now opt in. Release
validation ran the full workspace: 421 tests passed, three optional measurements
ignored, both sequentially and with the default parallel runner. Workspace Clippy
with warnings denied and formatting passed. No `.moth` schema change in the backup
controls; native desktop GUI checks remain deferred.

The first sandboxed run could not use local sockets or the default storage root;
native checks were repeated outside it. The new backup comparison initially used
snapshot bytes, whose item order is not canonical; it now checks the exact item
count and every item by UUID. One native parallel run saw a transient active-file
lock in the existing corrupt-recovery-metadata test after dropping the writer.
The full sequential run and a full ordinary parallel rerun both passed; no test
was skipped or disabled to obtain those results.

Release checks also passed: all 31 corpus fixtures round-trip and `cargo xtask
killtest 50` lost no acknowledged commands. The workspace tests additionally run
50 move-batch kills and 50 save kills, including original-image recovery.

The maintainer requested immediate publication after the passing workspace,
Clippy, corpus and kill checks; a fresh release-session quick benchmark was
skipped. Earlier move/save measurements remain documented above.
