# MothRef Roadmap

MothRef is a fast, crash-proof reference board for artists, written in Rust and licensed under GPL-3.0-or-later. It aims to carry on where [BeeRef](https://github.com/rbreu/beeref) stopped: everything BeeRef and PureRef 2 can do, plus a board library, Milanote-style elements, a browser extension, a web build and (eventually) real-time collaboration — while being several times faster and never losing the user's work.

Board files use the `.moth` extension.

---

## Guiding principles

Every milestone and every pull request is judged against these, in this order:

1. **Never lose the user's work.** Every action reaches disk within 1 second. A crash costs at most the last second. This beats any feature.
2. **Memory follows a budget, not the board size.** RAM and VRAM stay flat as boards grow. The user chooses the trade-off between zoom speed and memory (performance mode, default "In memory"); every mode stays within an explicit budget.
3. **The UI thread never waits.** Decoding, compression, disk and network run off the main thread. Frame time target: ≤ 4 ms of main-thread work.
4. **Small files.** Original image bytes are stored as-is (no re-encoding to PNG), deduplicated by content hash. Caches can be dropped and rebuilt.
5. **Open.** GPL-3.0-or-later, a documented file format, import of `.bee` and `.pur`.

---

## Releases at a glance

| Release | After milestone | Promise |
| --- | --- | --- |
| 0.1 pre-alpha | M1 | Does not lose data |
| 0.3 public alpha | M2, M2.5, M2.6 | Replaces BeeRef, opens `.bee` and `.pur`, zooms faster than PureRef |
| 0.7 beta | M4 | Feature parity with PureRef 2 |
| 1.0 | M7 | All P0 and P1 features done, all performance targets met |
| 1.5 | M10 (items tagged 1.5), M12, M14 | Reference analysis (composition overlays, ruler with calibration), Obsidian interop, capture integrations for third-party tools |
| 2.0 | M9, M10 (items tagged 2.0), M16 | Web build and real-time collaboration; perspective and value tools; Excalidraw, draw.io and UML/XML interop, Blender plugin, video-frame capture |

Starting with **M1 / release 0.1**, every tagged release includes downloadable Windows, macOS and Linux binaries on **GitHub Releases**.

Milestones run strictly in order. M8 and M9 come after 1.0, but the architecture decisions they depend on (stable UUIDs, fractional z-order, command-based mutations, content-addressed blobs) are made in M0–M1.

M10–M16 (artist tools, review workflow, note-app interop, CLI and automation, capture integrations, PDF references, diagram-format and Blender interop) also come after 1.0. They do not depend on M8 or M9, so they may run in parallel with them and ship in 1.x minor releases as they land. Release targets set by the maintainer on 2026-10-21: items tagged **1.5** ship by release 1.5, items tagged **2.0** by release 2.0; untagged post-1.0 items have no target yet.

---

## Milestones

### M0 — Canvas

- [x] Cargo workspace, CI on Windows / macOS / Linux, `cargo-deny` license check for GPLv3 compatibility
- [x] `winit` window + `wgpu` renderer, redraw on demand only (0% CPU when idle)
- [x] Software fallback adapter (WARP on Windows, llvmpipe/lavapipe on Linux), automatic switch, adapter indicator in the UI
- [x] Scene model: `UUIDv7` item ids, fractional z-order indices, `Command` enum with inverses, R-tree spatial index
- [x] Instanced quad rendering, texture arrays, mip selection by on-screen size, VRAM budget with LRU eviction
- [x] Decode pool, JPEG DCT downscaling, LOD pyramid generation
- [x] Pan, zoom, rubber-band selection, file drag-and-drop (X11 / Windows / macOS), paste from clipboard
- [x] Native Wayland file drag-and-drop (TD-09; native protocol tests and Dolphin two-file drop validated)
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
- [ ] Tests: truncated-file recovery, 1,000× `kill -9`, reader fuzzing (eight-hour soak deferred to 1.2+)
- [x] Recent files
- [x] Portability: compressed texture format chosen at runtime (BC7 on desktop, uncompressed RGBA8 fallback; ASTC deferred to M8); LOD caches in `.moth` stay rebuildable for any format

**Status:** persistence, recovery and release packaging implemented; BC7/RGBA8
runtime texture-format selection implemented (ASTC deferred to M8). Per maintainer
direction on 2026-10-18, the eight-hour soak and native Windows/macOS manual
qualification move to 1.2+ and do not block earlier releases; they remain unverified. See [M1 validation notes](docs/M1-VALIDATION.md).

**Exit criteria:** 1,000 random `kill -9` runs lose no more than 1 second of work; the short soak test shows a flat RSS. → **Release 0.1**

### M2 — BeeRef parity (+ PureRef import)

**Tooling status (maintainer feedback, 2026-10-18):** the current editing and
Board tools UI is an unfinished first pass. Checked feature entries record initial
implementation, not usability acceptance or finished parity. Automated tests and
the tool-rail sizing fix do not close this work.

- [ ] Rework and polish the editing tools before accepting M2: validate actual
  workflows for selection, transforms/cropping, arrangement, pickers and export;
  improve discoverability, control layout and feedback; fix interaction and resize
  issues found during native manual use. Confirm usability with the maintainer
  before marking this task complete.
  Target layout agreed on 2026-10-24: [docs/DESIGN.md](docs/DESIGN.md) (screens 1–7, 13).
  - [ ] One floating bottom toolbar (tool group, Insert ▾, undo/redo, fit, zoom "− % +") replaces the top toolbar and the left tool rail; Select is the default tool and Esc returns to it
  - [ ] Selection bar above the selection and a grouped context menu with right-aligned shortcuts; one bounding box with four handles for multi-selection
  - [ ] Settings in their own window (Ctrl/⌘+,) with sections, icons and search; language picker moves there from the title bar
  - [ ] Status bar replaced by a "Saved · N images" chip; adapter, journal and budgets only in Settings › For developers
  - [ ] Adding many images arranges them at once, fits the camera and shows a toast with method ▾ and Undo; "ask above N" becomes an option (maintainer to confirm the default, see DESIGN.md §6)
  - [ ] Empty board: plain wording, every supported format listed, "Choose files…" and "Paste"
  - [ ] Fix defects from the 2026-10-24 alpha review (DESIGN.md §5): missing "→"/"▸" glyphs, palette focus and position, minimap thumbnails, zoom button order, top menu height
- [ ] Apply the «Album» identity (DESIGN.md §2): light paper theme by default, dark theme in the «Pixel» palette (screen, ink, bone, sage, wing), sage/moss accents, Bricolage Grotesque for the logo and large headings; new app icon and README logo once the final mark is drawn

**Current priority (maintainer, 2026-10-18):** accelerate bulk image import and
board opening. Other feature work waits until this performance task is complete.
Short data-safety checks and existing automated CI remain in scope.

- [x] Speed up bulk image import and opening `.moth`, `.bee` and `.pur` boards: measure time to first image and full load before/after on representative large boards; profile parsing, decoding and durable writes, remove repeated recovery-directory scans where confirmed, and keep memory budgets and crash recovery intact

  Before/after measurements, recovery checks and remaining latency limits: [load performance](docs/LOAD-PERFORMANCE.md).
- [x] Scale, rotate, flip, crop; transform image content inside its crop frame (BeeRef #140)
- [x] Mass scaling and arrangement: vertical, horizontal, optimal packing, by filename, configurable gap
- [x] Text notes, opacity, grayscale, color sampler (hex to clipboard)
- [x] Export: whole scene to PNG/SVG, a selected region (BeeRef #143), a single image to file or clipboard (BeeRef #132), all images to a folder
- [x] Empty-board state: drop zone with hint text and Paste / Open buttons (touch path for both)

  Editing/export validation and preview-resolution limits: [M2 notes](docs/M2-EDITING-VALIDATION.md).

- [x] Status bar: render adapter, item count, selection count, autosave/journal state ("saved", "journaled", "recovery on")
- [x] Zoom controls in the toolbar: − / + buttons and an editable zoom percentage field
- [x] Selection badge: item count and bounding size, shown while selecting and transforming
- [x] Every toolbar and tool-rail button has a tooltip with its name and current shortcut
- [x] Portability: responsive UI layout — no fixed desktop sizes, panels adapt to small screens
- [x] `.bee` import (SQLite reader, embedded images, layout and stacking; notes and unsupported transforms are reported as skipped)
- [x] Bounded `.pur` import via the local `purcore` safety fork from [LazyRef](https://github.com/achrllrogia45/LazyRef): 1.10/2.1 corpus and private 175-image 2.1.2 qualification, 4 GiB file admission, embedded-image/plain-note import, journal/undo integration
  - Compatibility is limited to verified files and supported content; see [import validation](docs/M2-IMPORT-VALIDATION.md). Extend coverage when reproducible sample files become available; qualifying every PureRef version is not an M2 requirement.
- [ ] All parity- and Milanote-checklist items tagged M2 (see checked entries below and [validation / remaining gaps](docs/M2-EDITING-VALIDATION.md#m2-parity-follow-up))

**Exit criteria:** every BeeRef board opens losslessly; every feature in BeeRef's README and CHANGELOG is present; all M2 parity items are ticked. → **Release 0.3 (public alpha)**

### M2.5 — Zoom performance and memory modes

**Priority (maintainer, 2026-10-20):** zoom speed and memory use come before M3.
Users choose how decoded previews are kept, as in PureRef's performance modes; the
default is "In memory". See [decision 0014](docs/decisions/0014-performance-modes-lod-cache.md).

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
- [ ] Measure per mode on reference hardware: warm-cache zoom latency to 2048 px, RSS after zooming in and back out, board open time; record results in [load performance](docs/LOAD-PERFORMANCE.md)

**Exit criteria:** in the default mode, zooming to full detail on an image already shown completes in ≤ 100 ms with a warm cache; every mode stays within its documented memory budget; "No cache" releases texture memory when zoomed out to fit the board.

### M2.6 — UI architecture: surfaces from actions, settings model, theme tokens

**Why (maintainer, 2026-10-24):** the open M2 task "rework and polish the editing
tools" is a UI redesign. Today buttons build commands inline, settings are fields
of the Board tools panel and colors are literals, so every redesign would be a
rewrite. This milestone makes the UI cheap to rearrange first
([decision 0018](docs/decisions/0018-ui-surfaces-from-action-registry.md)).
Board logic, the format and the renderer are not touched. Step plans for agents:
[docs/plans/ui-architecture](docs/plans/ui-architecture/README.md).

Each step is one PR, keeps behavior and appearance unchanged unless it says
otherwise, and leaves the app shippable.

- [ ] **Step 1 — Theme tokens.** One `moth-ui` theme module owns every color, radius and spacing value as named tokens, with a dark set (today's look) and a light set; `theme()` takes the active set; no `Color32` literal remains outside the module. Appearance unchanged.
- [ ] **Step 2 — Buttons run actions.** Every button in the Board tools window, the toolbar, the canvas context menu and modals runs a registered `Action` through `Intent::Action`; missing actions are added to `app/bindings.rs`; no surface builds a `Command`. Each action gains an icon id. Behavior unchanged.
- [ ] **Step 3 — Settings model and window.** Move every persisted preference out of `Tools` into one `Settings` struct with a schema (section, label, kind, default, range); persistence, a settings window opened by Ctrl/⌘+, and settings search in the palette are built from the schema. The settings part of Board tools is removed. Sections: General, Appearance, Canvas and navigation, Adding images, Window and overlay, Performance and memory, Keys and mouse, Integrations, For developers.
- [ ] **Step 4 — Surfaces as lists.** A bottom toolbar (tool group, Insert menu, undo/redo, fit, zoom), a selection bar with a list per item type, and the canvas context menu are built from ordered action lists; diagnostics leave the status bar for Settings › For developers.
- [ ] **Step 5 — Remove Board tools.** Delete the window and `Intent::ToggleTools`; every former control is reachable from a surface list, the palette and a touch path.

**Exit criteria:** no `Command` is built in UI code; no `Color32` literal outside the theme module; moving a button between surfaces is a one-line list change; the Board tools window is gone with no lost capability; headless UI screenshots of the board, library, settings and selection states are compared in CI.

### M3 — Everyday usability, board conversion, window, overlay, browser

**Priority within M3 (maintainer, 2026-10-18):** comfortable daily editing,
discoverable shortcuts and navigation without a working middle mouse button come
before the browser extension and advanced window integration.

- [x] Consistent multi-selection editing: move, scale, rotate, flip, duplicate, delete, reorder and arrange selected items together; show the affected selection and make each batch action one undo step
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
- [ ] Full navigation without pressing the middle mouse button: Space + left-drag to pan, a visible Hand tool with left-drag, and rebindable pan controls; middle-click remains optional
- [ ] Pan the canvas with WASD as an alternative to middle-button dragging; allow rebinding and disable canvas shortcuts while typing in text fields
- [ ] Navigation without a working wheel: visible and keyboard zoom controls, fit board / fit selection, keyboard panning, trackpad two-finger pan and pinch zoom; keep touch and pen paths available
- [ ] Open `.bee` / `.pur` boards for editing in MothRef, keeping the source file untouched; `.moth` is the only working-board save format
- [ ] After opening a foreign board, offer “Save as .moth to continue working in MothRef” with Save As and Continue without saving; the first Save opens the `.moth` destination chooser, and closing an unsaved conversion prompts to save or discard
- [ ] Explain conversion before saving: retain original image bytes and supported layout/edits, list unsupported content, and keep the original `.bee` / `.pur` available; test import → `.moth` save → reopen and crash recovery
- [ ] Separate board-file admission from individual-image limits: `.pur`, `.moth` and `.bee` must not inherit the image-import byte cap; use bounded streaming and explicit board-specific limits, retaining checksum, dimension, decoded-memory and corrupt-file protections
- [ ] Opening a board keeps images and other supported entities individually editable; never introduce an uneditable wrapper group or flatten the board. Preserve supported source groups with a clear enter/ungroup path (M4); report unsupported hierarchy
- [ ] Zoom-out memory qualification: measure RAM and VRAM separately while zooming out/in on a large board; promptly evict unnecessary high-resolution decoded data/textures, retain useful low mips within budgets, and avoid decode thrashing on zoom-in. Reuse cached mip levels before decoding again; evaluate a short bounded retention of recently used detail to avoid oscillation when zooming back and forth, with immediate eviction under budget pressure. Keep original image bytes unchanged and decoding/compression off the UI thread. Record time to reclaim memory and frame latency before setting numeric acceptance targets
- [ ] Explain `.moth` storage benefits with measured evidence (original-byte retention, deduplication and incremental saves); show “more compact” only where measurements support it, without promising a smaller file for every board

**Foreign-format policy:** `.pur` writing/export and save-back are out of scope.
A `.bee` export exception may be considered separately; it is not an M3 commitment
or a normal Save action. If introduced, its dialog must recommend `.moth`, explain
measured size benefits and warn that repeated editing/saving between applications
has no guaranteed round-trip compatibility. It must report unsupported content
before writing and preserve the original file.

- [x] Always-on-top, frameless window, move window by hotkey
- [x] Auto-reveal window bar: minimize, maximize/restore, close and pin controls appear on pointer approach; also reveal through keyboard focus and an explicit touch action
- [ ] Window modes (DESIGN.md §4, screens 15–17): Collect (full chrome) ⇄ View (images only, PureRef-like) with `Tab`; Over with `Ctrl/⌘+T`; a group can be viewed on its own; narrow windows switch to View; mode remembered per board. Opacity and board background (solid / translucent / glass with platform blur) live in the View popover and the Over panel
- [x] Pin button and rebindable shortcut: choose above all windows or above a chosen window; for the latter, let the user click the target, with cancel and visible active mode. A shortcut toggles the mode off/on using the last choice; respect platform capability flags
- [ ] Desktop GUI launch without a terminal: investigate the reported extra cmd window on Windows and ensure packaged GUI launches do not open a console; check normal Finder launch on macOS for the same symptom. Preserve diagnostic logging and intentional CLI output; qualify both launch paths
- [ ] Windows maximize/restore/fullscreen reliability: exercise repeated transitions, resize and surface recreation without losing the scene. Field report: PureRef crashed on Windows when expanded to fill the screen; maximize versus fullscreen is unconfirmed, and this is not yet a reproduced MothRef bug
  Implementation and pending native qualification: [M3 window validation](docs/M3-WINDOW-VALIDATION.md).
- [ ] Transparent overlay with adjustable opacity and click-through (BeeRef #100)
  - [x] Native overlay controls and composited master opacity; X11 qualification (Windows/macOS pending)
  - Pending cross-platform overlay and controller-recovery checks: [M3 window validation](docs/M3-WINDOW-VALIDATION.md#transparent-overlay).
- [ ] Board tabs in one window; drag a tab out into its own window and back
  - [x] Four independent journaled tabs, with dirty-close prompts and stable ordering
- [ ] Tab state: unsaved marker for boards that live only in the journal, close button, middle-click to close, "+" for a new board
- [ ] Overlay HUD: opacity slider, click-through toggle, always-on-top toggle, exit button, one-time hint with the exit shortcut
- [ ] Overlay grab handle that always receives clicks, so the overlay can be left without a global hotkey (Wayland and Flatpak fallback)
- [ ] Watch folder: images that appear in a chosen folder (for example the OS screenshots folder) are imported into a chosen board's Inbox
- [ ] Browser extension "Send to MothRef" + `moth-bridge` native host + "Inbox" board
- [ ] Attach to another application's window: Windows and X11 (macOS and Wayland are P2)
- [x] Platform capability flags surfaced in the UI
- [ ] All parity- and Milanote-checklist items tagged M3 (web clipper: hover Save, text clips, board picker; always on bottom, lock window, overlay selection, master opacity, resize to selection, appearance themes)

**Exit criteria:** daily navigation and batch editing work without middle-click or wheel input; shortcuts are discoverable and rebindable; opened `.bee` / `.pur` boards save and reopen safely as `.moth`, with conversion prompts and reports of unsupported content; the overlay works on top of Krita, Photoshop and Blender on Windows, macOS and X11.

### M4 — PureRef 2 parity

- [ ] Groups and a hierarchy tree view: Ctrl/⌘+G adds a tinted backdrop and focuses an inline title field at once; clicking the title selects the whole group, dragging it moves the group, dropping an image on the backdrop adds it; the hierarchy tree lives in a slide-out Layers drawer (DESIGN.md screen 14)
- [ ] Freehand drawing (3 widths, eraser, color) and shapes (ellipse, rectangle, line)
- [ ] Drawing pens: marker (thin / thick) and highlighter (thin / thick), each with color selection
- [ ] Washi tape: textured tape strips from a built-in set, plus user-added custom textures
- [ ] Eraser in two modes: pencil-style (erases part of a stroke) and object/layer eraser (removes a whole stroke at once)
- [ ] Emoji stamp: drop emoji reactions (like, not done, …) from a base set, with a custom emoji picker
- [ ] Rich text notes with links and checklists; configurable heading font size (BeeRef #130)
- [ ] Animated GIFs with frame stepping: the player sits on the image (scrubber, play/pause, frame counter, speed) and steps 1, 2, 3 or 4 frames at a time
- [ ] Line/dot grid with snapping; automatic alignment guides and edge/center snapping (BeeRef #129). Images snap by their displayed bounds; make grid visibility and snapping independently toggleable, with a temporary bypass while dragging
- [ ] Slideshow (incl. random order), embedded ICC profiles
- [ ] Timed drawing sessions on top of the slideshow: images from the board, a group or a selection; fixed intervals (30 s, 1, 2, 5, 10 min) or a session plan (for example 10 × 30 s, 5 × 2 min); pause, skip, countdown and a sound cue; list of the shown images at the end; runs full screen or in a mini window pinned over other apps that keeps the current image's aspect ratio while resized
- [ ] Minimap / navigator: board overview with the viewport rectangle; click or drag to move the camera; toggle with a shortcut
- [ ] All parity- and Milanote-checklist items tagged M4 (presentation mode, note formatting and markdown shortcuts, sketch card, drawing shortcuts, GIF frame tools, drawing details, locked groups, hierarchy tools, note parenting and links)

**Exit criteria:** every P0/P1 item sourced from PureRef in the feature matrix is done. → **Release 0.7 (beta)**

### M5 — Library

- [ ] Board index (`redb`) with file watcher (`notify`)
  - [x] Bounded recent/pinned catalog with background previews, search, folder filters and persistent tags/pins (requested reference UI slice)
- [ ] Start screen: card grid with board previews; stacks shown as a fan of previews or as a folder with icon and color; tags; search; pinning
- [ ] Stacks are real folders on disk (Procreate-style: drop a board on a board to create a stack); "smart collections" are saved tag filters
- [ ] File manager thumbnails for `.moth` and `.bee` (BeeRef #152): Windows `IThumbnailProvider`, freedesktop thumbnailer, macOS Quick Look extension
- [ ] Board preview image rendered off the UI thread after a checkpoint and stored as a rebuildable chunk (format version bump); used by the start screen, file-manager thumbnails and the note-app sidecar (M12)
- [ ] Library sidebar: All boards, Recent, Pinned, Trash (boards deleted from the library go to the OS trash and are listed here), Stacks, Smart collections; counts next to each entry; an icon per entry (pin for Pinned); stack colors picked from a palette and repeated as a dot on the board cards
- [ ] Library view options: sort by last opened, last edited, name, item count or size; grid / list view; tag filter chips above the grid
- [ ] Board cards show item count, last edited time, file size, pinned state and an "unsaved" badge when a recovery journal is pending; the library header shows totals
- [ ] Library health: boards with missing linked files (with relink), duplicate images across boards by content hash, largest boards
- [ ] All Milanote-checklist items tagged M5 (templates, search inside boards, item trash)

**Exit criteria:** a library of 1,000 boards is on screen < 200 ms after launch.

### M6 — Milanote-style elements

- [ ] Arrow connectors that stick to item edges
- [ ] Columns (auto-stacking containers), nested boards with breadcrumbs
- [ ] Color swatch cards and palette extraction from an image; link cards; comment pins
- [ ] Palettes: a palette element holding several swatches; export to `.gpl` (Krita, GIMP, Inkscape) and `.ase` (Adobe); import of both
- [ ] Create elements from the "Insert ▾" menu of the bottom toolbar, with an icon per element (replaces the planned collapsible left toolbar; maintainer, 2026-10-24)
- [ ] All Milanote-checklist items tagged M6 (unsorted tray, item labels, synced notes, card colors, captions, to-do cards, board shortcuts, back/forward navigation, Markdown export)

### M7 — Full import compatibility and release qualification

- [ ] Extend `.pur` import coverage for supported content and geometry; verify preservation through `.moth` save/reopen, with explicit reports for unsupported content
- [ ] Corpus covers every released PureRef 1.x and 2.x version; import → `.moth` save → reopen tests in CI
- [ ] Format discrepancies are fixed upstream in LazyRef, not in a private fork

**Exit criteria:** all P0 and P1 features done, all performance targets met. → **Release 1.0**

### M8 — Web build and multilingual handbook (after 1.0)

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

**Exit criteria:** a 1,000-image board opens in the browser and saves to a `.moth` file that the desktop app opens; the separate handbook repository and public website provide illustrated English and Russian guides and an API reference matching the released app.

### M9 — Collaboration (after 1.0, no fixed timeline)

- [ ] Prototype on two CRDT libraries (candidates: Loro, Yrs, Automerge); pick by speed and history size
- [ ] `moth-sync`: commands → CRDT operations; blob sync by `blake3` hash
- [ ] `moth-server`: WebSocket relay + blob store, Docker image (license to be decided when we get there)
- [ ] Participant cursors and selections; share links with view/edit rights

**Exit criteria:** three people edit one board from different OSes and the browser; after a network drop everything merges without loss. → **Release 2.0**

### M10 — Artist tools (after 1.0)

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

### M11 — Review and presentation (after 1.0)

For art directors who collect references and give feedback on work in progress. Everything is local; sharing and live comments stay in M9.

- [ ] Redline annotations: a drawing layer attached to an image (moves and scales with it), shown or hidden per image and for the whole board
- [ ] Review status on items: "approved", "needs work", "rejected" as a preset label set (built on item labels from M6), with a filter to show one status
- [ ] Compare two images: side by side, swipe slider, or onion skin with adjustable opacity; swap and align by a picked point
- [ ] Client presentation: hide notes, comments, labels and annotations with one switch; a chosen item order for stepping through (extends presentation mode from M4)
- [ ] Pitch sheet export: a board or a selection to a paginated PDF or a high-resolution PNG with a title, optional captions and optional notes

**Exit criteria:** a review round (annotate, set statuses, compare versions, export a PDF) is possible without leaving the app.

### M12 — Note-app interop: Obsidian and other Markdown tools (after 1.0)

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

### M13 — Command-line tools and automation (after 1.0)

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

### M14 — Capture integrations for third-party tools (after 1.0, target 1.5)

PureRef has no plugin API, so its community built tools that push images into it from outside: browser clippers with a local helper (RefDrop, PureDrop), a ComfyUI connector, ShareX setups and hotkey scripts. MothRef offers this officially. The browser extension itself stays in M3; M14 opens the same path to other tools.

- [ ] Local drop API in `moth-bridge`: a running MothRef accepts image bytes or an image URL plus source metadata (page URL, title, tool name) and adds them to a chosen board or its Inbox through a `Command`. Off by default, enabled in settings; local connections only (named pipe / Unix socket, or loopback HTTP with a per-install token); fetching a URL counts as the user action required by the network rule
- [ ] Documented protocol with ready-to-copy examples: `curl`, Python, AutoHotkey; the M13 CLI `add` covers the "app not running" case
- [ ] ShareX custom destination (Windows) and examples for Flameshot and Spectacle (Linux) and the macOS screenshot folder (via the M3 watch folder)
- [ ] ComfyUI custom node "Save to MothRef": generated images go to a chosen board in rows, with the workflow kept in the image metadata
- [ ] "Send to MothRef" from the OS file manager: Explorer context menu, Nautilus and Dolphin actions, macOS Share / Services
- [ ] Source and provenance shown on the item (where it came from, which tool sent it), feeding "Open image source"

**Exit criteria:** with MothRef running, a screenshot from ShareX, an image from a ComfyUI run and a file sent from the file manager each land on the chosen board within 1 second, journaled like a paste, and nothing listens on the network unless the user enabled it.

### M15 — PDF references (after 1.0, release to be confirmed by the maintainer)

PureRef handles images and GIFs only; a PDF (an artbook, a model sheet, a manual) has to be converted to images first. MothRef imports PDFs directly.

- [ ] PDF backend behind a `PdfBackend` trait: [`hayro`](https://github.com/LaurenzV/hayro) (pure Rust, Apache-2.0 OR MIT, no `unsafe`, builds for Android and wasm) by default; [`pdfium-render`](https://github.com/ajrcarey/pdfium-render) as an optional feature for files hayro cannot render. MuPDF is excluded (AGPL). Encrypted PDFs are reported as unsupported until a backend handles them
- [ ] **Import dialog before anything is added:** a grid of page thumbnails rendered off the UI thread; select all, select none, click or Shift-click pages to pick a subset, or type page ranges by hand (for example `1-3, 7, 10-12`); the selection and the page count stay in sync; Cancel adds nothing
- [ ] Import modes in the same dialog: one document item with page navigation and a page counter; selected pages as separate items, arranged with the M2 arrange methods; extract the embedded images of the selected pages as separate image items (useful for artbooks)
- [ ] Rendering by zoom: pages are vector, so render at the on-screen resolution in the decode pool, show the previous rendering until the new one is ready, tile pages at high zoom, and keep results in the M2.5 performance-mode caches with their budgets
- [ ] Storage: the original PDF bytes are stored once as a content-addressed blob (deduplicated by hash), never rasterized pages; this is a format change (schema bump, migration, corpus files, `docs/FORMAT.md`)
- [ ] Crop, overlays and measurements (M10) work on pages like on images; no text selection or search (MothRef is not a PDF reader)
- [ ] Corrupt or hostile PDFs never crash the app: parsing and rendering run inside `catch_unwind` with page-count, page-size and decoded-memory limits; a fuzz target for the PDF importer

**Exit criteria:** a 300-page artbook PDF shows its thumbnails in the import dialog within 2 seconds of the first page, a hand-typed page range imports exactly those pages, and the board stays within the memory budget of the active performance mode while zooming through them.

### M16 — Diagram formats, Blender and video-frame capture (after 1.0, target 2.0)

- [ ] Excalidraw: read `.excalidraw` JSON and Obsidian's `.excalidraw.md` (plain and compressed); images become image items (their embedded files as blobs), text becomes notes, rectangles, ellipses, arrows, lines and freehand strokes map to M4/M6 shapes, lines and drawings; unsupported elements are reported. Export is a later decision
- [ ] draw.io / diagrams.net: read `.drawio` XML (compressed and uncompressed diagrams); shapes, text and connectors map to MothRef elements where they exist, otherwise the diagram is imported as one vector item
- [ ] UML and other XML diagram formats: choose the formats worth importing (candidates: XMI, PlantUML and Mermaid sources) in a decision record before implementation
- [ ] Blender integration as a MothRef plugin with a matching Blender add-on: send selected images to Blender as reference images with crop and flip applied and the real-world scale taken from the M10 calibration; bring a viewport capture or render back into a board's Inbox. Talks to MothRef through the M14 local API; the plugin API itself (scope, sandboxing, versioning) is designed in a decision record first
- [ ] Video-frame capture: grab the current frame of a video playing in the browser through the extension (frame plus page URL and timestamp as the source), alongside the frame grab of M10 video items

**Exit criteria:** an Excalidraw file and a draw.io file open as editable boards with a report of anything skipped; a reference sent to Blender arrives at its calibrated size; a captured video frame keeps a link back to the exact moment in the source video.

---

## Feature matrix

Priorities: **P0** = no release without it, **P1** = required for 1.0, **P2** = after 1.0.

| Feature | Source | Priority | Milestone |
| --- | --- | --- | --- |
| Infinite canvas: pan, zoom, rubber-band selection | BeeRef, PureRef | P0 | M0 |
| Drag-and-drop from files and browsers, paste from clipboard | BeeRef, PureRef | P0 | M0 |
| Works without a dedicated GPU (software fallback) | MothRef | P0 | M0 |
| Autosave + crash recovery | MothRef | P0 | M1 |
| Save prompt on exit | MothRef, BeeRef 0.3.4-dev | P0 | M1 |
| `.moth` format, recent files | MothRef, PureRef 2.1 | P0 | M1 |
| Undo/redo for everything | BeeRef, PureRef | P0 | M1 |
| Move, scale, rotate, flip | BeeRef, PureRef | P0 | M2 |
| Crop | BeeRef, PureRef | P0 | M2 |
| Transform content inside crop frame (#140) | MothRef | P1 | M2 |
| Mass scale to same width/height/area | BeeRef | P0 | M2 |
| Arrange: vertical, horizontal, optimal, by filename, gap | BeeRef | P0 | M2 |
| Text notes | BeeRef, PureRef | P0 | M2 |
| Opacity and grayscale | BeeRef | P1 | M2 |
| Color sampler with hex copy | BeeRef, PureRef | P1 | M2 |
| Export scene to PNG/SVG, export all images | BeeRef | P1 | M2 |
| Export a region (#143), image to file/clipboard (#132) | MothRef | P1 | M2 |
| Hotkey to jump between images (#116) | MothRef | P1 | M3 |
| Configurable hotkeys and mouse | BeeRef, PureRef | P1 | M3 |
| Navigation without middle-click or wheel input | MothRef | P1 | M3 |
| Consistent batch editing of selected items | BeeRef, PureRef | P1 | M3 |
| Open `.bee` / `.pur`, offer conversion and save only as `.moth` | MothRef | P1 | M3 |
| `.bee` import | MothRef | P0 | M2 |
| `.pur` import, verified 1.x and 2.x samples | MothRef | P1 | M2 |
| Always-on-top, frameless, move window by key | BeeRef, PureRef | P0 | M3 |
| Board tabs + tear-off windows | MothRef | P1 | M3 |
| Transparent overlay, click-through (#100) | MothRef, PureRef | P1 | M3 |
| Attach to another app's window | PureRef 2.0 | P2 | M3 |
| Browser extension "Send to MothRef" | MothRef | P1 | M3 |
| Groups and hierarchy tree | PureRef 2.0 | P1 | M4 |
| Freehand drawing | PureRef 2.0 | P1 | M4 |
| Shapes | PureRef 2.1 | P1 | M4 |
| Rich text: links, checklists, formatting | PureRef 2.0 | P1 | M4 |
| Configurable heading font size (#130) | MothRef | P1 | M4 |
| GIFs with frame stepping | PureRef 2.0 | P1 | M4 |
| Grid with snapping | PureRef 2.1 | P1 | M4 |
| Alignment guides and snapping (#129) | MothRef | P1 | M4 |
| Searchable in-app cheat sheet of all current hotkeys | MothRef, Milanote | P1 | M3 |
| Command Palette / Omnibar: search and run actions, show shortcuts | MothRef, PureRef 2.0 | P1 | M3 |
| Slideshow incl. random order | PureRef 2.0 | P2 | M4 |
| ICC color profiles | PureRef 2.0 | P1 | M4 |
| Trackpad gestures | PureRef 2.0 | P1 | M3 |
| Start screen with board previews | MothRef, BeeRef | P0 | M5 |
| Stacks (folders) and tags, smart collections | MothRef | P1 | M5 |
| File manager thumbnails (#152) | MothRef | P1 | M5 |
| Milanote-style elements (see the Milanote checklist) | Milanote | P1 | M4–M6 |
| Web build | MothRef | P2 | M8 |
| Real-time collaboration | MothRef | P2 | M9 |
| Status bar, empty-board state, zoom field, selection badge | MothRef | P1 | M2 |
| Watch folder import | MothRef | P1 | M3 |
| Overlay HUD and grab handle | MothRef | P1 | M3 |
| Timed drawing sessions | MothRef | P1 | M4 |
| Minimap / navigator | MothRef | P1 | M4 |
| Board preview chunk | MothRef | P0 | M5 |
| Library sorting, views, board stats, library health | MothRef | P1 | M5 |
| Palette export/import (`.gpl`, `.ase`) | MothRef | P1 | M6 |
| SpaceMouse | PureRef 2.0 | P2 | after 1.0 |
| Composition overlays, ruler with calibration (target 1.5) | MothRef, This Ruler Rules | P2 | M10 |
| Mirror view | MothRef | P2 | M10 |
| Value check, perspective helper (target 2.0) | MothRef | P2 | M10 |
| Video references | FrameRef | P2 | M10 |
| Open in external editor | MothRef | P2 | M10 |
| Redlines, review statuses, image comparison | MothRef | P2 | M11 |
| Client presentation and pitch sheet export | MothRef | P2 | M11 |
| JSON Canvas export/import | Obsidian | P2 | M12 |
| Deep links, wiki-links, linked `.md` notes, preview sidecar | MothRef | P2 | M12 |
| CLI / headless automation (`clap` subcommands, `--json`) | MothRef | P2 | M13 |
| Fetch images from HTTP(S) URLs into a board | MothRef | P2 | M13 |
| Local drop API, ShareX / ComfyUI / file-manager integrations (target 1.5) | RefDrop, PureDrop, MothRef | P2 | M14 |
| PDF import with page picker (release to be confirmed) | MothRef | P2 | M15 |
| Excalidraw, draw.io and UML/XML import (target 2.0) | MothRef | P2 | M16 |
| Blender plugin (target 2.0) | MothRef | P2 | M16 |
| Video-frame capture from the browser (target 2.0) | RefDrop, MothRef | P2 | M16 |

`#NNN` refers to issues in the [BeeRef repository](https://github.com/rbreu/beeref/issues).

### Parity checklist (PureRef 2.1 + BeeRef 0.3.x)

The matrix above lists features at headline level. This checklist is the full command-level inventory from the [PureRef handbook](https://www.pureref.com/handbook/) and BeeRef's [actions](https://github.com/rbreu/beeref/blob/main/beeref/actions/actions.py) and [CHANGELOG](https://github.com/rbreu/beeref/blob/main/CHANGELOG.rst). A milestone is not done until every item tagged with it is ticked. Source: **PR** = PureRef, **BR** = BeeRef.

As in PureRef: when nothing is selected, arrange/align/normalize actions apply to all items; otherwise only to the selection. Most of them also work on notes.

#### Normalize (M2)
- [x] Normalize height (PR, BR)
- [x] Normalize width (PR, BR)
- [x] Normalize size (PR, BR)
- [x] Normalize area (PR)
- [x] Normalize scale (PR)

#### Arrange (M2 unless noted)
- [x] Optimal — tightly pack all selected displayed bounds into a compact square, with no gap or resizing; independent of window size and zoom (maintainer clarification)
- [x] By name / by filename (PR, BR)
- [x] By time added (PR)
- [ ] By file path (PR)
- [x] Randomly (PR 2.1)
- [ ] By hierarchy order (PR) — M4, needs the hierarchy
- [x] Horizontal, vertical and square layouts by filename (BR)
- [x] Running the same arrange command again reverses the order (PR)
- [x] Configurable gap between items for horizontal, vertical and grid layouts; Optimal always uses zero gap
- [x] Auto-arrange newly added images, with a chosen default method (PR "select and pack new images", BR "default arrange method")
  - [x] Ask how to arrange when one drop/paste adds more than `arrange_prompt_threshold` (default 5) images; result lands at the cursor in one undo step
  - [x] Make the threshold and the default method real settings instead of a constant
- [x] Pack all + optimize canvas in one command (PR)

#### Align and distribute (M2)
- [x] Align left / right / top / bottom (PR)
- [x] Distribute horizontally / vertically (PR)
- [x] Stack — pile selected images on top of each other in layer order (PR)

#### Transform (M2 unless noted)
- [x] Flip horizontally / vertically (PR, BR)
- [x] Reset scale, rotation, flip, crop, or all transforms (PR, BR)
- [x] Raise to top / lower to bottom (BR)
- [ ] Setting "bring selected images to front" (PR)
- [ ] Per-image sampling: bilinear vs nearest neighbour, plus a default for new images (PR); automatic nearest-neighbour at large zoom for pixel art (BR)
- [ ] Lock item so it cannot be selected on the canvas, e.g. as a backdrop (PR) — M4

#### Canvas and view (M0–M2)
- [x] Fit scene / focus canvas (PR, BR) — M0
- [x] Fit selection / focus image; double-click a selected image to fit the entire group's width and height, again on the same image to go back; an unselected image focuses alone (PR, BR) — M0
- [x] Reset camera; reset zoom to 100% (PR) — M0
- [x] Focus next / previous image (PR; BeeRef #116)
- [x] Optimize canvas — fit the canvas snugly around all items (PR)
- [x] Lock canvas — nothing on it can be changed (PR)
- [x] Grayscale for the whole canvas (PR) and per image (PR, BR)
- [x] Configurable scroll wheel: pan, zoom or nothing; Shift / Shift+Ctrl + wheel to pan (PR, BR)
- [ ] Optional scrollbars and menu bar (BR)
- [x] Scale the scene with the window on resize, as a setting (PR)

#### Window modes (M3)
- [ ] Always on top (PR, BR)
- [ ] Always on top of a chosen application (PR) — P2 outside Windows/X11
- [ ] Always on bottom — use the board as an interactive wallpaper (PR)
- [ ] Lock window position; option to stay locked while maximized (PR)
- [ ] Transparent to mouse (click-through), with a system-global shortcut and "disable on activate" (PR; BeeRef #100)
- [ ] Overlay selection — every selected image gets its own borderless floating window (PR)
- [ ] Master window opacity up / down (PR)
- [ ] Resize window to selection or to all items (PR)
- [ ] Toggle title bar, window border, fullscreen (PR, BR); move window by hotkey or modifier+drag with its own cursor (PR, BR)
- [ ] Appearance: background, canvas and selection colors; "glass" theme with a transparent canvas (PR)

#### Pickers and info (M2)
- [ ] Color picker: RGB / HSV / HEX, copy value, keep picking outside the window while the button is held, ignores image opacity (PR, BR)
- [ ] Coordinate picker: pixel X/Y, image name, source, size, size of the selection (PR)
- [ ] Show color gamut of an image (BR)
- [ ] Open image source — the URL or path the image came from (PR); filled automatically by the browser extension

#### Images, files and export
- [ ] Embed images or link to files on disk (PR) — M1
- [ ] Relink missing linked images (PR) — M2
- [ ] Optional auto-downscale of very large images on import (PR) — M2
- [x] Export selected images, all images, scene (PR, BR) — M2
- [ ] Load most recent scene; option to reopen the last scene on start (PR) — M1
- [ ] Open image files from the command line; first argument `.moth`/`.bee`/`.pur` opens a board (BR) — M1
- [ ] Notify when pasting from an empty or unusable clipboard (BR) — M0
- [ ] Comment on an image, shown as an icon and tooltip (PR) — M6, with comment pins

#### Notes (M2 plain, M4 rich)
- [ ] Notes auto-size to text; manual size wraps text (PR)
- [ ] Creating a note with an image or group selected parents it to that item (PR) — M4
- [ ] Copy / paste text formatting (PR) — M4
- [ ] Links to local files; clicking a link to a `.moth` / `.pur` / `.bee` file opens that board (PR) — M4

#### GIFs (M4)
- [ ] Play / pause, timeline scrubbing, playback speed, frame step (PR)
- [ ] Copy a single frame, or drag it out as a new image item (PR)

#### Drawing (M4)
- [ ] Line color, thickness and style for all tools (PR)
- [ ] Shift snaps lines to 0/45/90°, makes perfect circles and squares (PR)
- [ ] Double-click a drawing (or its hierarchy entry) to edit it again (PR)

#### Groups and hierarchy (M4)
- [ ] Groups with a customizable background, auto-resize to content, empty groups removed (PR)
- [ ] Locked groups: click selects the group, double-click enters it (PR)
- [ ] Slideshow and pack operate on a group's content when only the group is selected (PR)
- [ ] Hierarchy: drag to reorder and re-parent, rename with F2, status icons (linked, locked, comment), tooltip with source and size (PR)
- [ ] Parent selected items to the first selected item (PR)

#### Slideshow (M4)
- [ ] Interval setting; order: ordered / shuffle (each once) / random (PR)
- [ ] Toolbar play / pause / step; manual navigation pauses it; UI fade options (PR)

#### Input, shortcuts, localization
- [ ] Every action rebindable; system-global shortcuts for chosen actions (PR, BR) — M3
- [x] Settings show which values differ from defaults (BR) — M2
- [x] Settings store and panel: persist preferences; first values `arrange_prompt_threshold` and the arrange gap
- [x] Undo history size slider: how many steps to keep in memory (default 256), shown with an estimate of the memory the history consumes (bytes per step × kept steps), so a user can trade undo depth for RAM
- [ ] Touch input and SpaceMouse (PR) — touch via the input abstraction (M0), SpaceMouse after 1.0
- [x] Interface translations; at least English and Russian at 0.3, more later (PR 2.1 ships 7 languages) — M2
- [x] Extensible translation catalogs with automatic language registration and English fallback
- [ ] Debug log viewer in the app (BR) — M1

### Milanote checklist

Inventory of [Milanote](https://milanote.com) features from its [help center](https://help.milanote.com/llms.txt), [keyboard shortcuts](https://help.milanote.com/getting-started/shortcuts) and [release notes](https://milanote.com/releases), with a decision for each. MothRef takes what helps collect and arrange references offline; it skips what only makes sense for a cloud SaaS. Milestone in brackets; **skip** = not planned, **after 1.0** = P2.

#### Cards and elements
- [ ] **Note card**: rich text with large/normal heading, bold, italic, underline, strikethrough, highlight, bullet/numbered/check lists, quote, code block, indent, center alignment (M4)
- [ ] Markdown shortcuts at line start: `#`, `.`/`-`, `1.`, `[ ]`, `>` (M4)
- [ ] Optional **source line** at the bottom of a note; Ctrl/⌘+Enter copies it to the next note (M6)
- [ ] **Card colors**: one of 9 colors for notes, columns and boards; a color strip on image cards (M6)
- [ ] **Heading** card — big text without a card background (M4)
- [ ] **To-do card** as its own element, besides checklists inside notes (M6)
- [ ] **Image caption** — select an image and start typing; toggle caption visibility (M6)
- [ ] Image actions: download original, open in the system viewer (M2)
- [ ] **Link card** with three toggleable parts: preview image, URL, description; preview fetched only on user action (M6)
- [ ] Custom link thumbnail — drop an image onto the link card's preview (M6)
- [ ] Pasting a board link (`.moth` path or `mothref://` link) creates a **board shortcut card** (M6)
- [ ] **Column**: title, drag to reorder with an insertion line, resize, collapse/expand, item count, "Group into column" from a selection; columns can hold boards but not other columns (M6)
- [ ] **Nested board**: card with preview, double-click to enter, drop a board on a board to nest it, drag onto a breadcrumb to move it up (M6)
- [ ] **Lines**: drag from a card's corner handle, connect to another card or anchor to a point, bend by the middle handle (double-click to straighten), Shift for straight, color and weight, label by typing, pasted text on a line becomes its label (M6)
- [ ] **Color swatch**: new swatches auto-pick a color that matches the board's images; HEX / RGB / HSL display or no label; paste a HEX; eyedropper from board images; resizable (M6)
- [ ] Palette extraction from a selected image (k-means, 5–8 swatches) — MothRef addition (M6)
- [ ] **Sketch card** with a white background (M4)
- [ ] Drawing shortcuts: D pen, V select, E eraser, X previous color, 1–4 pen size, Esc to finish; pressure "ink effect" with an off switch; export a drawing as SVG (M4)
- [ ] **Comment pins** on items, shown on hover; hidden in presentation mode (M6)
- [ ] Table card with formulas (after 1.0)
- [ ] Document card — long-form text opened full-screen (after 1.0)
- [ ] PDF documents (M15); other file cards and audio cards (after 1.0)
- [ ] Video card and video link embeds (YouTube, Vimeo) (after 1.0, together with video on the canvas)
- Map card — **skip** (needs online map tiles)
- Built-in stock photo search (Pexels) — **skip** (network service; the browser extension covers collecting)

#### Creating and editing
- [ ] Double-click empty canvas creates a note (setting, off by default to keep PureRef behaviour) (M4)
- [ ] Ctrl/⌘+Enter creates the next note/board/column below the selected one, matching its width and pushing items below down (M6)
- [x] Duplicate with Alt/⌥+drag (M2)
- [x] Nudge with arrow keys, farther with Shift+arrows (M2)
- [x] Send backward / bring forward one step, besides to-top / to-bottom (M2)
- [ ] Lock item position from the context menu (M4, same as PureRef "lock item")
- [ ] Cards snap to the grid; optional "crop images to fit the grid" (M4)
- [ ] Remove image background — local model, no cloud (after 1.0)

#### Organizing
- [ ] **Unsorted tray** per board: imported and clipped items land there first, then get dragged onto the canvas; the browser extension can send to it (M6)
- [ ] **Item labels**: colored labels on any item, rename updates everywhere, usage count, find all items with a label. Same tag system as board tags in the library (M6)
- [ ] **Synced notes**: duplicate a note as a linked copy that stays in sync, within one board file (M6); across board files (after 1.0)
- [ ] Item **trash**: deleted items stay restorable per board until emptied, on top of undo (M5)
- [ ] **Templates**: "convert board to template", start a new board from a template (M5)
- [ ] Search across boards and inside them: note text, captions, labels, link titles; jump to the item (M5)
- [ ] Navigation: back / forward history, go to parent board, recent boards (M6)

#### Viewing and presenting
- [ ] **Presentation mode**: full-screen, editing locked, comments hidden, click an item to focus/zoom it, settings remembered (M4, alongside slideshow)
- [x] Dark / light theme toggle (M2)
- [x] Interface scale smaller / larger (M2)
- [x] Searchable in-app shortcut cheat sheet on `/` and in Help: all actions grouped by category, showing current custom bindings (M3, next to the command palette)

#### Export
- [x] PDF export of a board or a selection (M2)
- [ ] Markdown and plain-text export of notes, headings, to-dos and link cards (M6)
- [x] ZIP of all original images and files (M2, with "export all images")
- Word export — **skip** (Markdown covers it)

#### Web clipper (M3, in the MothRef browser extension)
- [ ] "Save" button on hover over any image
- [ ] Save selected text as a note: right-click menu or a button next to the selection (setting)
- [ ] Choose the target: active board, one of the 10 most recent boards, or the board's unsorted tray
- [ ] Clipped items keep the page URL as their source

#### Collaboration and cloud (M9 or skip)
- [ ] Real-time co-editing, read-only share links, per-board editor list (M9)
- [ ] Reactions and @mentions on comments (M9)
- Password-protected boards — **skip** as a server feature; optional encrypted `.moth` files (after 1.0)
- Accounts, plans, billing, team admin, notifications by email — **skip**

#### Mobile and tablet (future Android port)
- [ ] Share-sheet target ("Share to MothRef") instead of the browser extension
- [ ] Touch gestures: double-tap canvas for a note, long-press to drag, pinch to zoom, three-finger swipe undo/redo, long-press + drag to multi-select

---

## Deferred qualification — 1.2 or later

- [ ] Full eight-hour combined render/persistence soak (CI / maintainer only).
- [ ] Native Windows/macOS manual save, recovery, clipboard, close and OS-shutdown checks.

Deferred by the maintainer on 2026-10-18; these checks do not block earlier releases.
Automated cross-platform CI and short change-specific safety checks remain required.

---

## Field observations — 2026-10-20

The M3 navigation, packing, feedback, window and memory follow-ups above come from
maintainer use of PureRef; they describe desired MothRef behavior, not verified
claims about PureRef internals. The unwanted grouping after opening a file is a
reported pain point; its cause and source-file hierarchy remain unknown.

- [ ] **2.0.0 candidate only:** 3D model reference items; scope, formats, interaction and resource budgets remain open. Do not start before 1.0 or treat this as an M3 import requirement.

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
| Extension | WebExtension (MV3) | addons.mozilla.org, Chrome Web Store | M3 |

Known issues to plan for: macOS notarization needs a paid Apple Developer account; Windows signing (apply to a free OSS program such as SignPath Foundation early); Flatpak sandbox limits drag-and-drop sources, native messaging and overlays; the in-app update check is disabled in package-manager builds.

---

## Decision log

| Date | Decision |
| --- | --- |
| 2026-10-24 | [UI surfaces from the action registry](docs/decisions/0018-ui-surfaces-from-action-registry.md): buttons run registered actions, surfaces are action lists, settings are a schema-driven model, visual values come from light/dark theme tokens; Board tools window removed in M2.6 |
| 2026-10-22 | [Full-resolution mode](docs/decisions/0017-full-resolution-mode.md): default mode preloads every image at full resolution into video memory behind a progress bar and never loads while zooming; video-memory limit detected from the adapter and shown to the user; AGENTS.md exception |
| 2026-10-21 | [Baked tile packs](docs/decisions/0016-baked-tile-packs.md): one background decode per large image writes every detail tile; parent tiles stand in while loading; "Prepare zoom tiles" setting (on screen / whole board / off); RAM cache 12.5% ≤ 1 GiB written through to an 8 GiB disk cache |
| 2026-10-21 | [Viewport tiles and scheduling](docs/decisions/0015-viewport-tiles-and-scheduling.md): bounded replaceable viewport queue; fixed-size visible detail tiles; native JPEG scanlines and Android NDK toolchain |
| 2026-10-21 | Post-1.0 release targets: 1.5 = composition overlays and ruler (M10), Obsidian interop (M12), capture integrations (M14); 2.0 = perspective and value tools (M10), Excalidraw / draw.io / UML-XML interop, Blender plugin, video-frame capture (M16); PDF import with a page picker (M15), release to be confirmed |
| 2026-10-20 | [Performance modes and LOD cache](docs/decisions/0014-performance-modes-lod-cache.md): PureRef-style In memory (default) / low zoom quality / Cache on disk / No cache; decode once per zoom range from a budgeted RAM or disk cache; BC7 only where it can be encoded |
| 2026-10-19 | [RAM-aware texture budget](docs/decisions/0013-ram-aware-texture-budget.md): replace per-image growth with actual texture bytes and available RAM; configurable RAM allowance/cache limits, adapter defaults and LRU eviction |
| 2026-10-19 | [Progressive durable loading](docs/decisions/0012-progressive-board-loading.md): journaled loading batches overlap previews with storage; bounded footer-chain indexing accelerates healthy `.moth` reads while retaining scanner recovery |
| 2026-10-19 | [Journal original ranges](docs/decisions/0011-journal-original-ranges.md): read originals through bounded read-only storage handles into the session journal; remove duplicate `.source` files without changing durable completion boundaries |
| 2026-10-16 | Name: MothRef, file extension `.moth`, license GPL-3.0-or-later |
| 2026-10-16 | Stack: Rust, `wgpu`, `winit`, egui with a fully custom theme and animations |
| 2026-10-16 | Multiple boards: tabs in one window, tabs can be torn off into windows |
| 2026-10-16 | Stacks are folders on disk; smart collections are saved tag filters |
| 2026-10-16 | LOD tiles: per-board switch between "fast open" (in file) and "compact file" (external cache); compact by default |
| 2026-10-16 | No-GPU support: wgpu software adapter in M0; dedicated CPU renderer only if benchmarks demand it |
| 2026-10-16 | PureRef compatibility through `purcore` from LazyRef (MIT), pinned; fixes go upstream; clean-room only |
| 2026-10-16 | Collaboration via CRDT; the sync server is recorded but has no timeline, its license is decided later |
| 2026-10-16 | Distribute through every channel listed above |
| 2026-10-16 | Milanote: take the offline board features (cards, lines, columns, nested boards, labels, unsorted tray, presentation mode, web clipper); skip cloud-only ones (stock photos, maps, accounts, password boards) |
| 2026-10-16 | Android is a future target (no milestone yet). Portability rules are built in from M0: storage trait, input abstraction, surface-loss handling (M0); runtime texture format (M1); responsive layout (M2) |
| 2026-10-16 | [M0 preview budgets and durable clipboard staging](docs/decisions/0001-m0-canvas-budgets.md): 96 MiB GPU cache, bounded decode work, checksummed RGBA scratch records; keep the 50 ms paste criterion open rather than bypass durability |
| 2026-10-16 | [M1 version-one records and recovery](docs/decisions/0002-m1-durable-records.md): footer commits, journal-before-acknowledgment, atomic snapshots and compaction, bounded session history |
| 2026-10-16 | Maintainer accepts the measured 11–12 ms frame time (12 ms benchmark ceiling); revise the immediate 4K paste target to 250 ms. Historical measurements retain their original targets. The paste harness excludes OS clipboard transfer; native end-to-end qualification remains separate. |
| 2026-10-16 | Maintainer tightens the immediate 4K paste target to 150 ms; qualification remains open. |
| 2026-10-16 | Maintainer accepts approximately 200 ms durable 4K paste and closes TD-03; the 150 ms goal is withdrawn. Timing outliers and native qualification remain documented. |
| 2026-10-16 | [Bounded WebP previews](docs/decisions/0003-bounded-webp-previews.md): maintainer permits full-frame static WebP with 8 MiB encoded/frame admission limits; internal codec allocations are separately qualified. GIF/BMP use row-based previews. |
| 2026-10-18 | [Runtime texture format](docs/decisions/0006-runtime-texture-format.md): BC7 on desktop (target-gated `intel_tex_2`), RGBA8 fallback, ASTC deferred to M8; `.moth` stays GPU-format independent |

| 2026-10-17 | [Reference workspace](docs/decisions/0004-reference-workspace.md): requested early UI slice uses four isolated sessions, a bounded catalog and a separate overlay controller; full M3/M5 remain open. |
| 2026-10-17 | [M2 content transforms and schema 2](docs/decisions/0005-m2-content-transform.md): five-field per-item transform (rotation, flip, content offset/scale/rotation), frame-only indexed footprint, schema 2 records that keep reading schema 1 |
| 2026-10-16 | Post-1.0 milestones M10 (artist tools), M11 (review and presentation), M12 (note-app interop through Markdown, JSON Canvas and URIs, not a single app's API); independent of M8/M9 |
| 2026-10-17 | Post-1.0 milestone M13 (CLI and automation): headless subcommands built into the `mothref` binary with `clap`, `--json` output; reuses the single storage-worker write path; images may be fetched from URLs on explicit invocation |
| 2026-10-18 | [M2 editing and export](docs/decisions/0008-m2-editing-export.md): schema 4 appearance/source names, journaled inspector, bounded PNG/SVG export and unchanged original export |
| 2026-10-18 | [Local purcore import fork](docs/decisions/0007-local-purcore-import.md): maintainer authorizes vendoring and local fixes; bounded path-free import, 1.10/2.1 baseline corpus; coverage limited to verified files, with additional qualification driven by available samples |

| 2026-10-19 | [Adaptive GPU cache budget](docs/decisions/0009-adaptive-gpu-budget.md): maintainer requests count-based growth/shrink; 32–512 MiB discrete, 32–96 MiB shared/software, immediate eviction on shrink and zero allocation for an empty board |
| 2026-10-19 | [Parallel app responsibility split](docs/decisions/0010-app-responsibility-split.md): retain upstream history and historical design notes while integrating the later tested intent/Workspace/Shell refactor and motion implementation |
