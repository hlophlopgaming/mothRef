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
