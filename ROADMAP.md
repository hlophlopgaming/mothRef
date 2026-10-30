# MothRef Roadmap

MothRef is a fast, crash-proof reference board for artists, written in Rust and licensed under GPL-3.0-or-later. It carries on where [BeeRef](https://github.com/rbreu/beeref) stopped: everything BeeRef and PureRef 2 can do, plus a board library, Milanote-style elements, a browser extension, a web build and (eventually) real-time collaboration — while being faster and never losing the user's work. Board files use the `.moth` extension.

This file says **what comes next and in which order**. Related files:

- [`CHANGELOG.md`](CHANGELOG.md) — what has shipped, release by release
- [`docs/FEATURES.md`](docs/FEATURES.md) — the full feature inventory: priorities, PureRef 2 / BeeRef parity checklist, Milanote checklist
- [`docs/decisions/README.md`](docs/decisions/README.md) — decision log
- [`docs/DEFERRED.md`](docs/DEFERRED.md) — ideas to check later, without a milestone
- [`docs/DESIGN.md`](docs/DESIGN.md) — how the UI looks; [`AGENTS.md`](AGENTS.md) — how to build it

---

## How this file is kept

- Each milestone shows **open work first**. Finished items move into the collapsed "Done" list under the same milestone, unchanged, so their history stays.
- An item that is built but not yet checked on real hardware stays open and says what is left ("native qualification"). Long-running qualification that does not block a release lives in [Qualification](#qualification).
- Dated maintainer notes ("maintainer, 2026-10-24") belong in the [decision log](docs/decisions/README.md); a milestone keeps at most one short **Why** or **Priority** line.
- Work normally follows milestone order, but the maintainer may pull a later item forward; that is recorded in the decision log. A **release** ships only when every milestone listed for it meets its exit criteria.
- Agents tick an item when its PR lands and move it into "Done" when the milestone is next tidied.

---

## Guiding principles

Every milestone and every pull request is judged against these, in this order:

1. **Never lose the user's work.** Every action reaches disk within 1 second. A crash costs at most the last second. This beats any feature.
2. **Memory follows a budget, not the board size.** RAM stays within explicit budgets. Video memory follows the detected graphics card; the default full-resolution mode deliberately keeps the whole board resident up to that limit ([decision 0017](docs/decisions/0017-full-resolution-mode.md)), and the user can pick a leaner performance mode.
3. **The UI thread never waits.** Decoding, compression, disk and network run off the main thread. Frame time target: ≤ 4 ms of main-thread work. Nothing loads while zooming in the default mode.
4. **A reference viewer first.** View mode stays as calm as PureRef: images only. Board-building tools (notes, drawing, Milanote-style elements) live in Collect mode and never clutter View; switching modes never loses functionality.
5. **Small files.** Original image bytes are stored as-is (no re-encoding), deduplicated by content hash. Caches can be dropped and rebuilt.
6. **Open.** GPL-3.0-or-later, a documented file format, import of `.bee` and `.pur`.

---

## Where we are

Latest release: **0.1.0-alpha.16** (2026-10-29). Pre-alpha builds for Windows, macOS and Linux are attached to every tagged release on GitHub Releases.

| Milestone | State | What is left |
| --- | --- | --- |
| M0 Canvas | ✅ done | Native touch, desktop-pen and end-to-end paste qualification ([Qualification](#qualification)) |
| M1 Reliability and file format | ✅ done | Long kill / fuzz / soak runs ([Qualification](#qualification)) |
| M2 BeeRef parity | 🟡 almost | App icon (on hold); a dozen small parity items |
| M2.5 Zoom performance and memory modes | 🟡 almost | Measurements on reference hardware |
| M2.6 UI architecture | ✅ done | Native and CI screenshot qualification |
| M3 Everyday usability, window, overlay | 🟡 mostly done | Native window qualification (Windows, macOS, Plasma 6, GNOME 47+) |
| M4 PureRef 2 parity | ⚪ not started | Minimap is already done |
| M5 Library | 🟡 first slice | Start screen, search, tags, pins and folder stacks exist; index, previews, trash, health open |
| M6 Milanote-style elements | ⚪ not started | |
| M7 Import compatibility and 1.0 qualification | ⚪ not started | |
| M8–M16 | after 1.0 | |

---

## Releases

| Release | Ships when these are done | Promise | State |
| --- | --- | --- | --- |
| 0.1 pre-alpha | M0, M1 | Does not lose data | shipping as 0.1.0-alpha.N |
| **0.3 public alpha** | M2, M2.5, M2.6 | Replaces BeeRef, opens `.bee` and `.pur`, zooms faster than PureRef | **next** |
| 0.7 beta | M3, M4 | Feature parity with PureRef 2 | |
| 1.0 | M5, M6, M7 | All P0 and P1 features, all performance targets | |
| 1.5 | M10 items tagged 1.5, M12, M14 | Composition overlays and ruler, Obsidian interop, capture integrations | |
| 2.0 | M8, M9, M10 items tagged 2.0, M16 | Web build and collaboration; value and perspective tools; Excalidraw, draw.io, UML/XML, Blender, video frames | |
| 1.x, no target yet | M11, M13, M15, untagged M10 items | Review workflow, CLI, PDF references, other artist tools | |

M8 and M9 come after 1.0, but the architecture they depend on (stable UUIDs, fractional z-order, command-based mutations, content-addressed blobs) is already in place from M0–M1. M10–M16 do not depend on M8 or M9 and ship in 1.x minor releases as they land.

---

## Shipped — 0.1 pre-alpha

M0 and M1 are complete; the open qualification runs are listed under [Qualification](#qualification).

<details>
<summary>M0 — Canvas, review follow-up, M1 — Reliability and file format</summary>

### M0 — Canvas

- [x] Cargo workspace, CI on Windows / macOS / Linux, `cargo-deny` license check for GPLv3 compatibility
- [x] `winit` window + `wgpu` renderer, redraw on demand only (0% CPU when idle)
- [x] Software fallback adapter (WARP on Windows, llvmpipe/lavapipe on Linux), automatic switch, adapter indicator in the UI
- [x] Scene model: `UUIDv7` item ids, fractional z-order indices, `Command` enum with inverses, R-tree spatial index
- [x] Instanced quad rendering, texture arrays, mip selection by on-screen size, VRAM budget with LRU eviction
- [x] Decode pool, JPEG DCT downscaling, LOD pyramid generation
- [x] Pan, zoom, rubber-band selection, file drag-and-drop (X11 / Windows / macOS), paste from clipboard
- [x] Native Wayland file drag-and-drop (TD-09; native protocol tests and Dolphin two-file drop validated)
- [x] Accept local ZIP-entry drops from KDE viewers on Wayland through background image import (native Gwenview validation pending).
- [x] Benchmark boards and measurement harness
- [x] UI skeleton on egui: custom theme, color tokens, transition animations
- [x] Portability: storage behind a byte-stream trait (no `std::fs::Path` in core crates)
- [x] Portability: input abstraction — pointer events carry kind (mouse / touch / pen) and pressure; gestures are a separate layer
- [x] Portability: surface loss handled like device loss (recreate the renderer, keep the scene)
- [x] CI: core crates (`moth-scene`, `moth-format`, `moth-io`) build for `aarch64-linux-android`

**Status:** runnable preview; native Wayland drops and Huion pen identity/pressure validated. Real touch-device, other desktop pen and native end-to-end paste qualification remain open. See [validation notes](docs/M0-VALIDATION.md).

**Exit criteria:** a 10,000-image board pans with p99 frame time ≤ 12 ms (the measured 11.5 ms is accepted); a pasted 4K image is visible in approximately 200 ms (accepted by the maintainer).

### M0 review follow-up

Concrete M0/M1 carry-over work and completion checks: [technical debt backlog](docs/TECH-DEBT.md).

- [x] Clipboard staging: application data, lossless compression, per-board ownership
  and cleanup on explicit discard (originals survive a crash; layout recovery is M1)
- [x] Allow large JPEG sources when DCT-reduced allocation fits the decode budget
- [x] WebP, static GIF and BMP import (bounded WebP per ADR 0003; BMP RLE/embedded codecs unsupported)
- [x] EXIF orientation and CMYK JPEG conversion
- [x] Area-averaged PNG and clipboard previews, including Adam7 input
- [x] Avoid linear `Scene::top()` scans on repeated insertion
- [x] Replace the 10,000-item board/render ceilings with bounded render batches by M1–M2
- [x] Clipboard backpressure feedback and native file-list paste
- [x] URL/HTML clipboard import (direct HTTP(S) image URLs and the first HTML image)
- [x] Add storage temporary objects and atomic replacement before M1 compaction

### M1 — Reliability and file format

- [x] Tagged GitHub Actions builds attach Windows `.zip`, macOS `.tar.gz` (Intel and Apple Silicon), and Linux `.tar.gz` binaries to GitHub Releases from 0.1

- [x] `.moth` format: chunks, footer, append-only checkpoints, background compaction
- [x] Write-ahead journal, autosave, rotating backups
- [x] Recovery dialog on startup, save prompt on exit
- [x] Undo/redo for every command (bounded to 256 commands per session)
- [x] Panic hook, crash report, decoder isolation (`catch_unwind`, header size checks)
- [x] Recent files
- [x] Portability: compressed texture format chosen at runtime (BC7 on desktop, uncompressed RGBA8 fallback; ASTC deferred to M8); LOD caches in `.moth` stay rebuildable for any format

**Status:** persistence, recovery and release packaging implemented; BC7/RGBA8
runtime texture-format selection implemented (ASTC deferred to M8). Per maintainer
direction on 2026-10-18, the eight-hour soak and native Windows/macOS manual
qualification move to 1.2+ and do not block earlier releases; they remain unverified. See [M1 validation notes](docs/M1-VALIDATION.md).

**Exit criteria:** 1,000 random `kill -9` runs lose no more than 1 second of work; the short soak test shows a flat RSS. → **Release 0.1**

</details>

---

## Now — toward 0.3 public alpha

### M2 — BeeRef parity (+ PureRef import)

**Exit criteria:** every BeeRef board opens losslessly; every feature in BeeRef's README and CHANGELOG is present; all M2 items in [FEATURES.md](docs/FEATURES.md) are ticked. → **Release 0.3**

Open:

- [ ] New app icon and README logo — **on hold** until the final mark is drawn
- [ ] Parity leftovers from [FEATURES.md](docs/FEATURES.md) (a few were tagged M0/M1 there but never done, so they are collected here):
  - [ ] Color picker: keep picking outside the window while the button is held (the rest is done)
  - [ ] Coordinate picker: check every field (pixel X/Y, image name, source, size, selection size)
  - [ ] Show the color gamut of an image (BR)
  - [ ] Open image source — the URL or path the image came from (PR)
  - [ ] Arrange by file path (PR)
  - [ ] Setting "bring selected images to front" (PR)
  - [ ] Optional scrollbars and menu bar (BR)
  - [ ] Optional auto-downscale of very large images on import (PR)
  - [ ] Load the most recent scene; option to reopen the last scene on start (PR)
  - [ ] Open files from the command line: the first `.moth` / `.bee` / `.pur` argument opens a board, image paths are added (BR)
  - [ ] Notify when pasting from an empty or unusable clipboard (BR)
  - [ ] Debug log viewer in the app (BR)

Validation notes: [editing and export](docs/M2-EDITING-VALIDATION.md), [import](docs/M2-IMPORT-VALIDATION.md), [load performance](docs/LOAD-PERFORMANCE.md), target layout [DESIGN.md](docs/DESIGN.md).

<details>
<summary>Done</summary>

- [x] Rework and polish the editing tools before accepting M2: validate actual
  workflows for selection, transforms/cropping, arrangement, pickers and export;
  improve discoverability, control layout and feedback; fix interaction and resize
  issues found during native manual use. Confirm usability with the maintainer
  before marking this task complete.
  Target layout agreed on 2026-10-24: [docs/DESIGN.md](docs/DESIGN.md) (screens 1–7, 13).
  Maintainer accepted usability on 2026-10-25. Native validation: [alpha review notes](docs/plans/alpha9-review/README.md#native-follow-up--2026-10-25).
  - [x] One floating bottom toolbar (tool group, Insert ▾, undo/redo, fit, zoom "− % +") replaces the top toolbar and the left tool rail; Select is the default tool and Esc returns to it
  - [x] Selection bar above the selection and a grouped context menu with right-aligned shortcuts; one bounding box with four handles for multi-selection
  - [x] Settings in their own window (Ctrl/⌘+,) with sections, icons and search; language picker moves there from the title bar
  - [x] Status bar replaced by a "Saved · N images" chip; adapter, journal and budgets only in Settings › For developers
  - [x] Adding many images arranges them at once, fits the camera and shows a toast with method ▾ and Undo; "ask above N" remains an option (maintainer confirmed the default of 5, see DESIGN.md §6)
  - [x] Empty board: plain wording, every supported format listed, "Choose files…" and "Paste"
  - [x] Fix defects from the 2026-10-24 alpha review (DESIGN.md §5): missing "→"/"▸" glyphs, palette focus and position, minimap silhouettes (maintainer-approved), zoom button order, top menu height
- [x] Apply the «Album» identity (DESIGN.md §2): light paper theme by default, dark theme in the «Pixel» palette (screen, ink, bone, sage, wing), sage/moss accents, Bricolage Grotesque for the logo and large headings
- [x] Speed up bulk image import and opening `.moth`, `.bee` and `.pur` boards: measure time to first image and full load before/after on representative large boards; profile parsing, decoding and durable writes, remove repeated recovery-directory scans where confirmed, and keep memory budgets and crash recovery intact
- [x] Scale, rotate, flip, crop; transform image content inside its crop frame (BeeRef #140)
- [x] Mass scaling and arrangement: vertical, horizontal, optimal packing, by filename, configurable gap
- [x] Text notes, opacity, grayscale, color sampler (hex to clipboard)
- [x] Export: whole scene to PNG/SVG, a selected region (BeeRef #143), a single image to file or clipboard (BeeRef #132), all images to a folder
- [x] Empty-board state: drop zone with hint text and Paste / Open buttons (touch path for both)
- [x] Status bar: render adapter, item count, selection count, autosave/journal state ("saved", "journaled", "recovery on") — later replaced by the save-state chip; diagnostics moved to Settings › For developers
- [x] Zoom controls in the toolbar: − / + buttons and an editable zoom percentage field
- [x] Selection badge: item count and bounding size, shown while selecting and transforming
- [x] Every toolbar and tool-rail button has a tooltip with its name and current shortcut
- [x] Portability: responsive UI layout — no fixed desktop sizes, panels adapt to small screens
- [x] `.bee` import (SQLite reader, embedded images, notes, layout and stacking; unsupported transforms are reported as skipped)
- [x] Bounded `.pur` import via the local `purcore` safety fork from [LazyRef](https://github.com/achrllrogia45/LazyRef): 1.10/2.1 corpus and private 175-image 2.1.2 qualification, 4 GiB file admission, embedded-image/plain-note import, journal/undo integration
  - Compatibility is limited to verified files and supported content; see [import validation](docs/M2-IMPORT-VALIDATION.md). Extend coverage when reproducible sample files become available; qualifying every PureRef version is not an M2 requirement.

</details>

### M2.5 — Zoom performance and memory modes

**Why:** zoom speed and memory use come before new features. Users choose how decoded previews are kept, as in PureRef's performance modes ([decision 0014](docs/decisions/0014-performance-modes-lod-cache.md), [0016](docs/decisions/0016-baked-tile-packs.md), [0017](docs/decisions/0017-full-resolution-mode.md)).

**Exit criteria:** in the default mode, zooming to full detail on an image already shown completes in ≤ 100 ms with a warm cache; every mode stays within its documented memory budget; "No cache" releases texture memory when zoomed out to fit the board.

Open:

- [ ] Measure per mode on reference hardware: warm-cache zoom latency to 2048 px, RSS after zooming in and back out, board open time; record results in [load performance](docs/LOAD-PERFORMANCE.md)

<details>
<summary>Done</summary>

- [x] Performance mode setting: In memory (default), In memory with low zoom quality, Cache on disk, No cache; persisted, explained in English and Russian
- [x] Decode once per zoom range: larger whole-image previews use 2× headroom; coarse fallbacks stay at their requested size, and cached previews/tiles avoid re-reading, decoding and BC7-encoding the original
- [x] RAM cache with an explicit budget and LRU eviction (12.5% of available RAM, at most 1 GiB, written through to disk), released when leaving an in-memory mode
- [x] Disk cache: GPU-ready pyramids in the OS cache folder, per-mip checksums, 8 GiB budget, LRU pruning off the UI thread
- [x] Select BC7 only on builds that can encode it (Apple silicon / Windows on Arm previews never uploaded)
- [x] Release VRAM on zoom-out: replace oversized resident textures with a lower mip copied on the GPU, without decoding
- [x] Request scheduler: order requests by visible area and distance to the viewport center, cancel superseded requests, request the final level during an active zoom gesture
- [x] Tiled pyramids for large images: fixed-size tile atlas instead of square power-of-two size classes, full-resolution inspection beyond 2048 px, memory proportional to screen pixels
- [x] Settings: show RAM/disk cache usage and a "Clear disk cache" action
- [x] Evaluate a global allocator (mimalloc) against glibc arenas with a heap profiler; keep it only with a measured RSS win
- [x] Baked tile packs: one background pass per large image writes every detail tile, so zoom reads ready tiles instead of decoding; parent tiles stand in while a tile loads; "Prepare zoom tiles" setting: on screen (default), whole board, off (decision 0016)
- [x] "Everything at full resolution" mode (default): whole board in video memory at open, progress bar, overflow warning with on-demand fallback; video-memory limit from the detected adapter (decision 0017)

</details>

### M2.6 — UI architecture: surfaces from actions, settings model, theme tokens

**Why:** buttons run registered actions, settings are a schema and colors are tokens, so rearranging the UI is a list edit instead of a rewrite ([decision 0018](docs/decisions/0018-ui-surfaces-from-action-registry.md), step plans in [docs/plans/ui-architecture](docs/plans/ui-architecture/README.md)).

**Exit criteria:** no `Command` is built in UI code; no `Color32` literal outside the theme module; moving a button between surfaces is a one-line list change; the Board tools window is gone with no lost capability; headless UI screenshots of the board, library, settings and selection states are compared in CI.

Open:

- [ ] Native and CI qualification listed in [UI refactor validation](docs/plans/ui-architecture/VALIDATION.md); the minimap-thumbnail improvement remains a follow-up (no new image cache)

<details>
<summary>Done</summary>

- [x] **Step 1 — Theme tokens.** One `moth-ui` theme module owns every color, radius and spacing value as named tokens, with a dark set (today's look) and a light set; `theme()` takes the active set; no `Color32` literal remains outside the module. Appearance unchanged.
- [x] **Step 2 — Buttons run actions.** Every button in the Board tools window, the toolbar, the canvas context menu and modals runs a registered `Action` through `Intent::Action`; missing actions are added to `app/bindings.rs`; no surface builds a `Command`. Each action gains an icon id. Behavior unchanged.
- [x] **Step 3 — Settings model and window.** Move every persisted preference out of `Tools` into one `Settings` struct with a schema (section, label, kind, default, range); persistence, a settings window opened by Ctrl/⌘+, and settings search in the palette are built from the schema. The settings part of Board tools is removed. Sections: General, Appearance, Canvas and navigation, Adding images, Window and overlay, Performance and memory, Keys and mouse, Integrations, For developers.
- [x] **Step 4 — Surfaces as lists.** A bottom toolbar (tool group, Insert menu, undo/redo, fit, zoom), a selection bar with a list per item type, and the canvas context menu are built from ordered action lists; diagnostics leave the status bar for Settings › For developers.
- [x] **Step 5 — Remove Board tools.** Delete the window and `Intent::ToggleTools`; every former control is reachable from a surface list, the palette and a touch path.
- [x] **Step 6 — Fixes from the alpha.9 review.** Select by default and fits that respect the floating chrome; notes keep their size after editing; save chip in the window bar with plurals and live translation; menus with an icon column, named submenus and no "More actions" drawer; settings dialog, light-theme contrast and a recovery dialog without raw paths. Plan and findings: [alpha.9 review](docs/plans/alpha9-review/README.md).

</details>

---

## Next — toward 0.7 beta

### M3 — Everyday usability, board conversion, window, overlay

**Exit criteria:** daily navigation and batch editing work without middle-click or wheel input; shortcuts are discoverable and rebindable; opened `.bee` / `.pur` boards save and reopen safely as `.moth`, with conversion prompts and reports of unsupported content; the overlay works on top of Krita, Photoshop and Blender on Windows, macOS and X11.

Navigation, shortcuts, the command palette, batch editing and board conversion are done (see "Done" below). Open work, grouped:

**Window modes and overlay** — layout in [DESIGN.md](docs/DESIGN.md) §4, screens 15–17; checks in [M3 window validation](docs/M3-WINDOW-VALIDATION.md)

- [ ] Window modes: Collect (full chrome) ⇄ View (images only) with `Tab`, Over with `Ctrl/⌘+T` — built; left:
  - [ ] Native qualification of mode transitions, opacity and blur on Linux, Windows and macOS
  - (Group-only viewing moved to M4 groups; it needs the scene group model)
- [ ] Transparent overlay with adjustable opacity and click-through (BeeRef #100) — X11 qualified; Windows / macOS and controller-recovery checks open
- [ ] Attach to another application's window: implemented on Windows, X11, KDE Plasma Wayland (KWin script), GNOME Wayland (Shell extension) and macOS (follows the chosen app); KDE 5.27 and GNOME 46 checked in nested sessions; native qualification on Windows, macOS, Plasma 6 and GNOME 47+ open
- [ ] Always on top on Wayland ([decision 0019](docs/decisions/0019-wayland-window-stacking.md)): KWin scripts on KDE Plasma, bundled GNOME Shell extension with an install button in Settings › Window and overlay — implemented; Plasma 5.27 and GNOME 46 checked in nested sessions (see M3 window validation); Plasma 6, GNOME 47–49 and real sessions open
- [ ] Native qualification of the added parity controls: bottom stacking, position lock/maximize, floating image surfaces, resize-to-content, opacity keys, custom colors and system-global shortcuts; see [validation notes](docs/M3-WINDOW-VALIDATION.md#remaining-parity-controls-implementation)

**Window reliability**

- [ ] Desktop GUI launch without a terminal: no extra console window on Windows, normal Finder launch on macOS; keep diagnostic logging and intentional CLI output; qualify both launch paths
- [ ] Windows maximize / restore / fullscreen: repeated transitions, resize and surface recreation without losing the scene (a PureRef field report, not a reproduced MothRef bug)

**Tabs**

- [ ] Board tabs in one window; drag a tab out into its own window and back — implemented (drag out of the tab strip; tab menu: move to new / another window, gather all tabs; drop onto another window's tab strip on Windows, macOS and X11); checked under KWin 5.27; native qualification on Windows and macOS open

**Foreign-format policy:** `.pur` writing/export and save-back are out of scope. A `.bee` export exception may be considered separately; it is not an M3 commitment or a normal Save action. If introduced, its dialog must recommend `.moth`, explain measured size benefits and warn that repeated editing/saving between applications has no guaranteed round-trip compatibility. It must report unsupported content before writing and preserve the original file.

<details>
<summary>Done</summary>

- [x] Overlay HUD: opacity sliders, click-through, always on top, exit, and a one-time hint with the exit shortcut (Settings › Window and overlay brings it back); the always-clickable grab handle was dropped (maintainer, 2026-10-29)
- [x] Tab state: italic title and an explanation for boards that live only in the journal, close button inside the tab, middle-click to close, "+" for a new board (Ctrl/⌘+N)

- [x] Remaining window-mode parity controls: bottom stacking, position lock (with a maximized-window preference), floating selected images, resize to selected/all items, opacity shortcuts, custom colors and opt-in system-global shortcuts. Native backend qualification remains open above; Wayland uses the Over controller rather than global grabs.

- [x] Consistent multi-selection editing: move, scale, rotate, flip, duplicate, delete, reorder and arrange selected items together; show the affected selection and make each batch action one undo step
- [x] Canvas multi-selection: Ctrl/Command-click and rectangle toggle, Shift-click and rectangle add; preserve group moves and expose bindings in the shortcut cheat sheet. Touch multi-select mode is planned in [SHORTCUTS.md](docs/SHORTCUTS.md).
- [x] Configurable keyboard and mouse bindings: conflict detection, reset to defaults, and familiar BeeRef/PureRef-style navigation presets; show active bindings in tooltips and menus
  - Use the [screenshot shortcut comparison](docs/SHORTCUTS.md) as a command/gesture inventory; an existing Tools action does not imply that its PureRef binding is implemented
  - One action/binding registry for dispatch, tooltips, cheat sheet and palette; exact modifiers, text/modal focus isolation, and explicit conflict preview for optional presets
  - Resolve Space focus versus Space+drag pan, Alt-drag pan versus duplicate, and arrows for focus/layering versus nudging; keep visible pen/touch alternatives
- [x] Zoom preferences: choose smooth or immediate stepped zoom and configure the zoom increment/sensitivity; persist the settings, keep the point under the pointer stable, and validate mouse-wheel and trackpad behavior
- [x] Tap Space to fit the selection, or the whole board when nothing is selected; Space + drag pans without triggering fit on release; ignore these bindings while typing
- [x] Tighter optimal packing for mixed-size images: search a compact square using every item's displayed bounds, preserve relative sizes/aspect ratios, and use zero gap; one undoable action (refinement of the completed M2 packer)
- [x] Arrangement controls: choose row/column count for vertical and horizontal layouts through an exact slider; vertical fills columns top-to-bottom and horizontal fills rows left-to-right. Per maintainer clarification, Grid uses a compact mosaic with the same gutter on both axes instead of fixed cells/columns. Preserve image sizes and make each arrangement one undoable action.
- [x] Hotkey feedback: a small non-blocking toast with the executed action and its current binding, remain visible for 3–4 seconds, then dissolve; hovering keeps the toast visible, and pointer leave dismisses it immediately. Provide an equivalent hold/dismiss path for keyboard and touch; repeated presses of the same action update one toast and restart its fade timer instead of stacking notifications; allow disabling, and stop redraws after fading
- [x] Hotkey to jump between images (BeeRef #116)
- [x] In-app shortcut cheat sheet: all actions and their current bindings (including custom bindings), grouped by category and searchable; open with `/` or from Help
- [x] Command Palette (Omnibar / Quick Search): search and run application actions, show their current shortcuts; open with Ctrl/⌘+K or a visible menu button, navigate with arrows, Enter to run, Esc to close
- [x] Navigation without pressing the middle mouse button: Space + left-drag and rebindable pan gestures; middle-click remains optional. Select stays active; the separate Hand/Pan mode was removed by maintainer request.
- [x] Pan the canvas with WASD as an alternative to middle-button dragging; allow rebinding and disable canvas shortcuts while typing in text fields; configurable flying speed or fixed screen-pixel steps in Settings › Canvas
- [x] Navigation without a working wheel: visible and keyboard zoom controls, fit board / fit selection, keyboard panning, trackpad two-finger pan and pinch zoom; keep touch and pen paths available (native pinch where supplied by the backend, Control + continuous scroll otherwise)
- [x] Open `.bee` / `.pur` boards for editing in MothRef, keeping the source file untouched; `.moth` is the only working-board save format
- [x] After opening a foreign board, offer “Save as .moth to continue working in MothRef” with Save As and Continue without saving; the first Save opens the `.moth` destination chooser, and closing an unsaved conversion prompts to save or discard
- [x] Explain conversion before saving: retain original image bytes and supported layout/edits, list unsupported content, and keep the original `.bee` / `.pur` available; test import → `.moth` save → reopen and crash recovery
- [x] Separate board-file admission from individual-image limits: `.pur`, `.moth` and `.bee` must not inherit the image-import byte cap; use bounded streaming and explicit board-specific limits, retaining checksum, dimension, decoded-memory and corrupt-file protections
- [x] MothRef application menu for File and Settings; compass navigation menu in the bottom action list; configurable View window controls (right by default) and pin-mode picker
- [x] Always-on-top, frameless window, move window by hotkey
- [x] Auto-reveal window bar: minimize, maximize/restore, close and pin controls appear on pointer approach; also reveal through keyboard focus and an explicit touch action
- [x] Collect/View action, automatic narrow-window View, sliding bar and touch reveal, shared appearance controls, per-board local presentation history, and Over controller integration
- [x] Pin button and rebindable shortcut: choose above all windows or above a chosen window; for the latter, let the user click the target, with cancel and visible active mode. A shortcut toggles the mode off/on using the last choice; respect platform capability flags
- [x] Native overlay controls and composited master opacity; X11 qualification (Windows/macOS pending)
- [x] Four independent journaled tabs, with dirty-close prompts and stable ordering
- [x] Platform capability flags surfaced in the UI

</details>

### M4 — PureRef 2 parity

**Exit criteria:** every P0/P1 item sourced from PureRef in the [feature matrix](docs/FEATURES.md#feature-matrix) is done. → **Release 0.7 (beta)**

Open:

- [ ] View a group on its own in View mode (moved from M3)
- [ ] Group follow-ups: images pasted or dropped from outside onto a backdrop join the group; exports draw group backdrops; drag and drop to reorder in the Layers drawer
- [ ] Freehand drawing (3 widths, eraser, color) and shapes (ellipse, rectangle, line)
- [ ] Drawing pens: marker (thin / thick) and highlighter (thin / thick), each with color selection
- [ ] Washi tape: textured tape strips from a built-in set, plus user-added custom textures
- [ ] Eraser in two modes: pencil-style (erases part of a stroke) and object/layer eraser (removes a whole stroke at once)
- [ ] Emoji stamp: drop emoji reactions (like, not done, …) from a base set, with a custom emoji picker
- [ ] Rich text notes with links and checklists; configurable heading font size (BeeRef #130)
- [ ] Animated GIFs with frame stepping: the player sits on the image (scrubber, play/pause, frame counter, speed) and steps 1, 2, 3 or 4 frames at a time
- [ ] Line/dot grid with snapping; automatic alignment guides and edge/center snapping (BeeRef #129). Images snap by their displayed bounds; make grid visibility and snapping independently toggleable, with a temporary bypass while dragging
- [ ] Slideshow (incl. random order), embedded ICC profiles
- [ ] **Key feature of 0.7.** Timed drawing sessions (gesture mode) on top of the slideshow: images from the board, a group or a selection; fixed intervals (30 s, 1, 2, 5, 10 min) or a session plan (for example 10 × 30 s, 5 × 2 min); pause, skip, countdown and a sound cue; list of the shown images at the end; runs full screen or in a mini window pinned over other apps that keeps the current image's aspect ratio while resized
- [ ] All parity- and Milanote-checklist items tagged M4 (presentation mode, note formatting and markdown shortcuts, sketch card, drawing shortcuts, GIF frame tools, drawing details, locked groups, hierarchy tools, note parenting and links)

<details>
<summary>Done</summary>

- [x] Minimap / navigator: board overview with the viewport rectangle; click or drag to move the camera; toggle with a shortcut — shipped early with configurable size, position and opacity (0.1.0-alpha.9–10)
- [x] Groups and a hierarchy tree view: Ctrl/⌘+G adds a tinted backdrop that hugs its members and focuses an inline title field at once; clicking the title selects the whole group, dragging it moves the group, dropping an item on the backdrop adds it and dragging it out takes it out; groups nest; the hierarchy tree lives in a slide-out Layers drawer (DESIGN.md screen 14; `.moth` schema 6, [decision 0021](docs/decisions/0021-groups.md))

</details>

---

## Later — toward 1.0

### M5 — Library

**Exit criteria:** a library of 1,000 boards is on screen < 200 ms after launch.

A first slice shipped early ([decision 0004](docs/decisions/0004-reference-workspace.md)); items below say what already exists.

- [ ] Board index (`redb`) with file watcher (`notify`)
  - [x] Bounded recent/pinned catalog with background previews, search, folder filters and persistent tags/pins (requested reference UI slice)
- [ ] Start screen: card grid with board previews; stacks shown as a fan of previews or as a folder with icon and color; tags; search; pinning — grid, previews, tags, search and pins exist; stack fans, icons and colors open
- [ ] Stacks are real folders on disk (Procreate-style: drop a board on a board to create a stack); "smart collections" are saved tag filters
- [ ] File manager thumbnails for `.moth` and `.bee` (BeeRef #152): Windows `IThumbnailProvider`, freedesktop thumbnailer, macOS Quick Look extension
- [ ] Board preview image rendered off the UI thread after a checkpoint and stored as a rebuildable chunk (format version bump); used by the start screen, file-manager thumbnails and the note-app sidecar (M12)
- [ ] Library sidebar (All boards, Pinned, Stacks and Smart collections exist): All boards, Recent, Pinned, Trash (boards deleted from the library go to the OS trash and are listed here), Stacks, Smart collections; counts next to each entry; an icon per entry (pin for Pinned); stack colors picked from a palette and repeated as a dot on the board cards
- [ ] Library view options: sort by last opened, last edited, name, item count or size; grid / list view; tag filter chips above the grid — last opened / name sort and grid / list exist
- [ ] Board cards show item count, last edited time, file size, pinned state and an "unsaved" badge when a recovery journal is pending; the library header shows totals
- [ ] Embed images or link to files on disk (PR): a per-image choice, linked images reload when the file changes; a format change (schema bump, migration, corpus files, `docs/FORMAT.md`). Moved from M2 on 2026-10-26
- [ ] Library health: boards with missing linked files (with relink), duplicate images across boards by content hash, largest boards
- [ ] All Milanote-checklist items tagged M5 (templates, search inside boards, item trash)

### M6 — Milanote-style elements

- [ ] Arrow connectors that stick to item edges
- [ ] Columns (auto-stacking containers), nested boards with breadcrumbs
- [ ] Color swatch cards and palette extraction from an image; link cards; comment pins
- [ ] Palettes: a palette element holding several swatches; export to `.gpl` (Krita, GIMP, Inkscape) and `.ase` (Adobe); import of both
- [ ] Create elements from the "Insert ▾" menu of the bottom toolbar, with an icon per element (replaces the planned collapsible left toolbar; maintainer, 2026-10-24)
- [ ] All Milanote-checklist items tagged M6 (unsorted tray, item labels, synced notes, card colors, captions, to-do cards, board shortcuts, back/forward navigation, Markdown export)

### M7 — Full import compatibility and release qualification

**Exit criteria:** all P0 and P1 features done, all performance targets met. → **Release 1.0**

- [ ] Extend `.pur` import coverage for supported content and geometry; verify preservation through `.moth` save/reopen, with explicit reports for unsupported content
- [ ] Corpus covers every released PureRef 1.x and 2.x version; import → `.moth` save → reopen tests in CI
- [ ] Format discrepancies are fixed upstream in LazyRef, not in a private fork

---

## After 1.0

Release targets: items tagged **1.5** ship by 1.5, items tagged **2.0** by 2.0; untagged items have no target yet.

### M8 — Web build and multilingual handbook (target 2.0)

**Exit criteria:** a 1,000-image board opens in the browser and saves to a `.moth` file that the desktop app opens; the separate handbook repository and public website provide illustrated English and Russian guides and an API reference matching the released app.

- [ ] `wasm32` build; WebGPU renderer with WebGL2 fallback
- [ ] Web backend for `moth-io` using `createImageBitmap` and Web Workers
- [ ] Storage in OPFS; open/save via File System Access API (download/upload fallback); installable offline PWA
- [ ] Receive images from the browser extension directly into the tab
- [ ] User comments on the board
- [ ] Create a separate GitHub repository for the MothRef handbook and publish it as a documentation website on GitHub Pages; link it from the app and the main repository
- [ ] Illustrated user guide: getting started, navigation, image and board editing, library, import/export, saving and recovery, settings and keyboard/touch controls; original MothRef screenshots, annotated examples and step-by-step workflows. Use the [PureRef handbook](https://www.pureref.com/handbook/) as a structural reference, without copying its text or assets
- [ ] API reference for the interfaces available in the documented release, with usage examples, inputs/outputs, errors and compatibility notes; link to the `.moth` format specification. Document future CLI/automation interfaces when they ship in M13
- [ ] Multilingual handbook: English and Russian at launch, language switcher, a shared page structure and a contribution workflow for further translations; translated captions and alt text for illustrations
- [ ] Keep handbook versions aligned with app releases; provide navigation and search across user guides and API pages, and check links and documentation builds in the handbook repository's CI

### M9 — Collaboration (no fixed timeline)

- [ ] Prototype on two CRDT libraries (candidates: Loro, Yrs, Automerge); pick by speed and history size
- [ ] `moth-sync`: commands → CRDT operations; blob sync by `blake3` hash
- [ ] `moth-server`: WebSocket relay + blob store, Docker image (license to be decided when we get there)
- [ ] Participant cursors and selections; share links with view/edit rights

**Exit criteria:** three people edit one board from different OSes and the browser; after a network drop everything merges without loss. → **Release 2.0**

### M10 — Artist tools

Tools that help while painting from references, not only while collecting them. None of them changes the source image; they are view settings or overlays.

- [ ] **1.5** Composition overlays per image (non-destructive, toggled and cycled by a rebindable shortcut, with a touch path); line color and opacity:
  - Grids: rule of thirds, N × M grid, square grid for transferring a drawing (cell size or cell count), center cross
  - Golden ratio: phi grid (0.382 / 0.618), golden spiral in four orientations with mirroring, golden triangles; for non-golden images choose "stretch to image" or "largest inscribed golden rectangle"
  - Diagonals: main diagonals, baroque and sinister diagonals, dynamic-symmetry armature (diagonals plus perpendiculars from the corners)
  - Aspect-ratio frames (3:4, 4:3, 1:1, 16:9, custom) to check how a reference would crop
  - Overlays are stored in normalized coordinates of the visible (cropped) image, so they follow crop, rotation and flip; drawn on top of the image texture with the item transform; scene export offers "with overlays"
- [ ] **1.5** Ruler and measuring (ideas from [This Ruler Rules](https://github.com/TheSmallKiwi/ThisRulerRules), MIT; no code copied without attribution in `THIRD_PARTY.md`):
  - A measurement is a child item of an image, stored in source-image pixel coordinates; it survives move, scale, any rotation, flip and crop. No screen-anchored lines and no PNG round-trip
  - Calibration per image (enter what a segment represents: cm, m, inches, "heads" or a custom unit; recalibrating recomputes every measurement) and an optional board-wide scale for size-comparison collages
  - Click a scale bar on the image to detect it: scan the row for the contrast edge, refine both edges to sub-pixel precision from the gradient, accept only thin bars, then open the calibration prompt
  - Loupe on the image texture (nearest-neighbour zoom with a pixel crosshair, no screen capture, works on Wayland); mild horizontal/vertical snapping (about 7°, relative to the image) with a toggle
  - Protractor (angle between two lines), ratio between two segments
  - Numbered labels laid out to avoid each other, with thin leaders when pushed away; measurement table in the side panel; CSV export
- [ ] Mirror view: flip the whole canvas view horizontally or vertically without changing the board, to check one's own drawing with fresh eyes
- [ ] **2.0** Value check: posterize an image or the whole canvas into 2–5 tones (notan), next to the existing grayscale; a blur ("squint") view
- [ ] **2.0** Perspective helper: draw two converging lines to find a vanishing point; show the horizon and perspective guide lines through one, two or three vanishing points
- [ ] Video references (replaces the "video on the canvas" P2 row): MP4 and WebM items, play / pause, frame step, playback speed 0.1–2×, A–B loop, grab a frame as a new image item; decoding through an LGPL-compatible library, off the UI thread
- [ ] "Open in…": open an image in an external editor (Krita, Photoshop, GIMP, Clip Studio or any configured program); linked images reload when the file changes on disk
- [ ] Timer: a countdown / Pomodoro-style timer overlay for focused drawing sessions

**Exit criteria:** every tool works on a 1,000-image board without breaking the frame budget; none of them mutates image data. Persisted overlays, measurements and calibrations follow the format-change rules (schema bump, migration, corpus files, `docs/FORMAT.md`).

### M11 — Review and presentation

For art directors who collect references and give feedback on work in progress. Everything is local; sharing and live comments stay in M9.

- [ ] Redline annotations: a drawing layer attached to an image (moves and scales with it), shown or hidden per image and for the whole board
- [ ] Review status on items: "approved", "needs work", "rejected" as a preset label set (built on item labels from M6), with a filter to show one status
- [ ] Compare two images: side by side, swipe slider, or onion skin with adjustable opacity; swap and align by a picked point
- [ ] Client presentation: hide notes, comments, labels and annotations with one switch; a chosen item order for stepping through (extends presentation mode from M4)
- [ ] Pitch sheet export: a board or a selection to a paginated PDF or a high-resolution PNG with a title, optional captions and optional notes

**Exit criteria:** a review round (annotate, set statuses, compare versions, export a PDF) is possible without leaving the app.

### M12 — Note-app interop: Obsidian and other Markdown tools

**Target: 1.5** (maintainer, 2026-10-21), except the Obsidian viewer plugin, which waits for M8.

Integration goes through open formats and URIs, not through one application's API, so it also works with Logseq, Zettlr and others.

- [ ] JSON Canvas ([jsoncanvas.org](https://jsoncanvas.org)) export and import: images → file nodes, notes → text nodes, groups and columns → group nodes, lines → edges; a board opens as an Obsidian Canvas
- [ ] Note text round-trips losslessly through Markdown (prerequisite for JSON Canvas and linked note files)
- [ ] Preview sidecar: optionally write `board.preview.webp` next to `board.moth` on save, so a note can embed it (`![[board.preview.webp]]`)
- [ ] Deep links: `mothref://open?board=…&item=<uuid>` opens a board and focuses an item (extends the board links from M6); "Copy link to item" in the context menu
- [ ] `[[wiki-links]]` in notes resolve against a configured vault folder and open in the user's note app (`obsidian://` or the system default for `.md`)
- [ ] Linked note card: a note backed by an external `.md` file, edited from either side; changes on disk are picked up by the file watcher and go through a `Command`, conflicts keep both versions
- [ ] Read-only `.moth` viewer as an Obsidian plugin, built from the WASM build of M8 — only after M8

**Exit criteria:** a board exported as JSON Canvas opens in Obsidian with every image, note, group and line, and imports back without loss of those elements.

### M13 — Command-line tools and automation

A headless mode of the `mothref` binary, so agents, scripts and power users can build and edit boards without the GUI. Arguments are parsed with `clap` (subcommands, `--help`, `--json`); it lives in the existing binary, not a new crate. Every command reaches the board through the same `Command` / journal path as the GUI — the CLI never writes the `.moth` byte stream by hand.

- [ ] Subcommand dispatch in `mothref` (`clap`, no new crate): `create`, `add`, `info`, `check`, `export`, and the scene edits below
- [ ] Import: `create` a board; `add` images from local paths; fetch and add images from HTTP(S) URLs (an explicit invocation counts as the user action required by the network rule)
- [ ] Arrange: optimal, by name, by time, by path; horizontal, vertical and square; configurable gap — the M2 layout operations, headless
- [ ] Align and distribute, stack, normalize (width, height, size, area, scale) — the M2 operations, headless
- [ ] Transform and metadata: move, scale, rotate, flip, crop; z-order (raise/lower, to-top/to-bottom); notes and labels
- [ ] Read and export: `info` with items, bounds, blobs and recovery state; `check` for integrity with script exit codes; export originals and the scene to PNG/SVG
- [ ] Machine-readable `--json` output for every command, with stable field names an agent can depend on
- [ ] Batch: run a sequence of commands or a script file in one invocation, journaled like interactive edits
- [ ] Data safety: the single `moth-platform` storage worker, checkpoint plus atomic replace, never an in-place write, never a panic on a corrupt file
- [ ] Every command and flag documented in `--help` and in a CLI reference under `docs/`

**Exit criteria:** an agent creates a board, fetches reference images from the internet, arranges them into a tidy layout, and exports a preview — in one scripted invocation, producing files the GUI opens unchanged.

### M14 — Capture integrations for third-party tools (target 1.5)

PureRef has no plugin API, so its community built tools that push images into it from outside: browser clippers with a local helper (RefDrop, PureDrop), a ComfyUI connector, ShareX setups and hotkey scripts. MothRef offers this officially.

The browser extension moved here from M3 (maintainer, 2026-10-29): it is a separate project with its own release cycle, built after 1.0 on top of the local drop API below.

- [ ] Browser extension "Send to MothRef" (Firefox + Chromium, Manifest V3) as a separate project, talking to MothRef through `moth-bridge`; images land in the chosen board's Inbox
- [ ] Web clipper items from [FEATURES.md](docs/FEATURES.md#web-clipper-after-10-in-the-separate-browser-extension-project): Save button on hover, text clips, board picker, source URL kept

- [ ] Local drop API in `moth-bridge`: a running MothRef accepts image bytes or an image URL plus source metadata (page URL, title, tool name) and adds them to a chosen board or its Inbox through a `Command`. Off by default, enabled in settings; local connections only (named pipe / Unix socket, or loopback HTTP with a per-install token); fetching a URL counts as the user action required by the network rule
- [ ] Documented protocol with ready-to-copy examples: `curl`, Python, AutoHotkey; the M13 CLI `add` covers the "app not running" case
- [ ] ShareX custom destination (Windows) and examples for Flameshot and Spectacle (Linux)
- [ ] ComfyUI custom node "Save to MothRef": generated images go to a chosen board in rows, with the workflow kept in the image metadata
- [ ] "Send to MothRef" from the OS file manager: Explorer context menu, Nautilus and Dolphin actions, macOS Share / Services
- [ ] Source and provenance shown on the item (where it came from, which tool sent it), feeding "Open image source"

**Exit criteria:** with MothRef running, a screenshot from ShareX, an image from a ComfyUI run and a file sent from the file manager each land on the chosen board within 1 second, journaled like a paste, and nothing listens on the network unless the user enabled it.

### M15 — PDF references (release to be confirmed)

PureRef handles images and GIFs only; a PDF (an artbook, a model sheet, a manual) has to be converted to images first. MothRef imports PDFs directly.

- [ ] PDF backend behind a `PdfBackend` trait: [`hayro`](https://github.com/LaurenzV/hayro) (pure Rust, Apache-2.0 OR MIT, no `unsafe`, builds for Android and wasm) by default; [`pdfium-render`](https://github.com/ajrcarey/pdfium-render) as an optional feature for files hayro cannot render. MuPDF is excluded (AGPL). Encrypted PDFs are reported as unsupported until a backend handles them
- [ ] **Import dialog before anything is added:** a grid of page thumbnails rendered off the UI thread; select all, select none, click or Shift-click pages to pick a subset, or type page ranges by hand (for example `1-3, 7, 10-12`); the selection and the page count stay in sync; Cancel adds nothing
- [ ] Import modes in the same dialog: one document item with page navigation and a page counter; selected pages as separate items, arranged with the M2 arrange methods; extract the embedded images of the selected pages as separate image items (useful for artbooks)
- [ ] Rendering by zoom: pages are vector, so render at the on-screen resolution in the decode pool, show the previous rendering until the new one is ready, tile pages at high zoom, and keep results in the M2.5 performance-mode caches with their budgets
- [ ] Storage: the original PDF bytes are stored once as a content-addressed blob (deduplicated by hash), never rasterized pages; this is a format change (schema bump, migration, corpus files, `docs/FORMAT.md`)
- [ ] Crop, overlays and measurements (M10) work on pages like on images; no text selection or search (MothRef is not a PDF reader)
- [ ] Corrupt or hostile PDFs never crash the app: parsing and rendering run inside `catch_unwind` with page-count, page-size and decoded-memory limits; a fuzz target for the PDF importer

**Exit criteria:** a 300-page artbook PDF shows its thumbnails in the import dialog within 2 seconds of the first page, a hand-typed page range imports exactly those pages, and the board stays within the memory budget of the active performance mode while zooming through them.

### M16 — Diagram formats, Blender and video-frame capture (target 2.0)

- [ ] Excalidraw: read `.excalidraw` JSON and Obsidian's `.excalidraw.md` (plain and compressed); images become image items (their embedded files as blobs), text becomes notes, rectangles, ellipses, arrows, lines and freehand strokes map to M4/M6 shapes, lines and drawings; unsupported elements are reported. Export is a later decision
- [ ] draw.io / diagrams.net: read `.drawio` XML (compressed and uncompressed diagrams); shapes, text and connectors map to MothRef elements where they exist, otherwise the diagram is imported as one vector item
- [ ] UML and other XML diagram formats: choose the formats worth importing (candidates: XMI, PlantUML and Mermaid sources) in a decision record before implementation
- [ ] Blender integration as a MothRef plugin with a matching Blender add-on: send selected images to Blender as reference images with crop and flip applied and the real-world scale taken from the M10 calibration; bring a viewport capture or render back into a board's Inbox. Talks to MothRef through the M14 local API; the plugin API itself (scope, sandboxing, versioning) is designed in a decision record first
- [ ] Video-frame capture: grab the current frame of a video playing in the browser through the extension (frame plus page URL and timestamp as the source), alongside the frame grab of M10 video items

**Exit criteria:** an Excalidraw file and a draw.io file open as editable boards with a report of anything skipped; a reference sent to Blender arrives at its calibrated size; a captured video frame keeps a link back to the exact moment in the source video.

### Ideas without a milestone

- [ ] **2.0 candidate only:** 3D model reference items; scope, formats, interaction and resource budgets remain open. Do not start before 1.0.
- Android port: portability rules are built in from M0 (see [AGENTS.md](AGENTS.md)); no milestone yet. Mobile items are listed in [FEATURES.md](docs/FEATURES.md#mobile-and-tablet-future-android-port).
- Checks and ideas moved out of M3 on 2026-10-26: [docs/DEFERRED.md](docs/DEFERRED.md).

---

## Qualification

Checks that need real hardware or long runs. They do not block the next alpha unless a milestone's exit criteria name them.

- [ ] M0: real touch devices, other desktop pens and native end-to-end paste ([M0 validation](docs/M0-VALIDATION.md); Huion pen and Wayland drops are done)
- [ ] M1: truncated-file recovery and reader fuzzing at release length; 1,000× `kill -9` ([M1 validation](docs/M1-VALIDATION.md))
- [ ] 1.0 release only: five-minute smoke soak (`cargo xtask soak 300 5`); omitted from intermediate releases per maintainer instruction on 2026-10-27
- [ ] 1.2 or later: full eight-hour combined render/persistence soak (CI / maintainer only)
- [ ] 1.2 or later: native Windows/macOS manual save, recovery, clipboard, close and OS-shutdown checks

The 1.2 items were deferred by the maintainer on 2026-10-18. Automated cross-platform CI and short change-specific safety checks remain required for every release. Carry-over technical debt: [TECH-DEBT.md](docs/TECH-DEBT.md).

---

## Performance targets

Reference machine: 8 cores, SSD, GTX 1660 / Apple M1 class GPU. Calibrate after the first measurements of PureRef 2 and BeeRef on the same boards.

| Metric | Target |
| --- | --- |
| Cold start to library on screen | < 200 ms |
| Open a 1,000-image board: first frame | < 300 ms |
| Same board: everything visible at full detail | < 2 s |
| Pan/zoom a 10,000-image board, p99 frame | ≤ 12 ms (discrete GPU; 11.5 ms accepted), < 16 ms (integrated) |
| Paste a 4K image until visible | ≈ 200 ms (accepted) |
| Save one change in a 2 GB board | < 200 ms |
| RAM on a 1,000 × 12 MP board | < 1.5 GB |
| `.moth` size vs sum of source images | ≤ 1.1× |
| Idle CPU | 0% |

A benchmark regression of more than 10% blocks a merge.

---

## Distribution

All builds come from one GitHub Actions pipeline triggered by a release tag. Raw binary archives are available on GitHub Releases from **0.1 (M1)**; installers and package-manager channels follow the schedule below.

| OS | Format | Channel | From release |
| --- | --- | --- | --- |
| Windows | Binary `.zip` | GitHub Releases | 0.1 (M1) |
| macOS | Binary `.tar.gz` (Apple Silicon and Intel) | GitHub Releases | 0.1 (M1) |
| Linux | Binary `.tar.gz` (x86_64) | GitHub Releases | 0.1 (M1) |
| Windows | Portable `.zip` (settings next to the exe when `portable.txt` exists) | GitHub Releases | 0.3 |
| Windows | MSI (`cargo-wix`) | GitHub Releases, winget | 0.3 |
| Windows | — | Scoop, Chocolatey | 1.0 |
| macOS | Universal `.dmg` (Apple Silicon + Intel) | GitHub Releases | 0.3 |
| macOS | — | Homebrew cask | 0.7 |
| Linux | AppImage | GitHub Releases | 0.3 |
| Linux | Flatpak | Flathub | 0.7 |
| Linux | `.deb` | own apt repository | 0.7 |
| Linux | PKGBUILD | AUR | 0.7 |
| Linux | `.rpm` | Fedora COPR | 1.0 |
| Linux | Nix | nixpkgs | 1.0 |
| Linux | Snap | Snap Store | after 1.0, on demand |
| Browser | PWA | static hosting | M8 |
| Extension | WebExtension (MV3) | addons.mozilla.org, Chrome Web Store | M14 (separate project, after 1.0) |

Known issues to plan for: macOS notarization needs a paid Apple Developer account; Windows signing (apply to a free OSS program such as SignPath Foundation early); Flatpak sandbox limits drag-and-drop sources, native messaging and overlays; the in-app update check is disabled in package-manager builds.
