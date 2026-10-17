# MothRef Roadmap

MothRef is a fast, crash-proof reference board for artists, written in Rust and licensed under GPL-3.0-or-later. It aims to carry on where [BeeRef](https://github.com/rbreu/beeref) stopped: everything BeeRef and PureRef 2 can do, plus a board library, Milanote-style elements, a browser extension, a web build and (eventually) real-time collaboration — while being several times faster and never losing the user's work.

Board files use the `.moth` extension.

---

## Guiding principles

Every milestone and every pull request is judged against these, in this order:

1. **Never lose the user's work.** Every action reaches disk within 1 second. A crash costs at most the last second. This beats any feature.
2. **Memory follows a budget, not the board size.** RAM and VRAM stay flat as boards grow; only what is visible is resident, at the resolution it is shown.
3. **The UI thread never waits.** Decoding, compression, disk and network run off the main thread. Frame time target: ≤ 4 ms of main-thread work.
4. **Small files.** Original image bytes are stored as-is (no re-encoding to PNG), deduplicated by content hash. Caches can be dropped and rebuilt.
5. **Open.** GPL-3.0-or-later, a documented file format, import of `.bee` and `.pur`.

---

## Releases at a glance

| Release | After milestone | Promise |
| --- | --- | --- |
| 0.1 pre-alpha | M1 | Does not lose data |
| 0.3 public alpha | M2 | Replaces BeeRef, opens `.bee` and `.pur` |
| 0.7 beta | M4 | Feature parity with PureRef 2 |
| 1.0 | M7 | All P0 and P1 features done, all performance targets met |
| 2.0 | M9 | Web build and real-time collaboration |

Starting with **M1 / release 0.1**, every tagged release includes downloadable Windows, macOS and Linux binaries on **GitHub Releases**.

Milestones run strictly in order. M8 and M9 come after 1.0, but the architecture decisions they depend on (stable UUIDs, fractional z-order, command-based mutations, content-addressed blobs) are made in M0–M1.

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
- [ ] Portability: input abstraction — pointer events carry kind (mouse / touch / pen) and pressure; gestures are a separate layer
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
- [ ] Tests: truncated-file recovery, 1,000× `kill -9`, reader fuzzing, 8-hour soak test
- [x] Recent files
- [ ] Portability: compressed texture format chosen at runtime (BC7 / ASTC / ETC2 / uncompressed RGBA8); LOD caches in `.moth` stay rebuildable for any format

**Status:** persistence, recovery and release packaging implemented; compressed-texture support and full soak qualification remain open. See [M1 validation notes](docs/M1-VALIDATION.md).

**Exit criteria:** 1,000 random `kill -9` runs lose no more than 1 second of work; the soak test shows a flat RSS. → **Release 0.1**

### M2 — BeeRef parity (+ PureRef import)

- [ ] Scale, rotate, flip, crop; transform image content inside its crop frame (BeeRef #140)
- [ ] Mass scaling and arrangement: vertical, horizontal, optimal packing, by filename, configurable gap
- [ ] Text notes, opacity, grayscale, color sampler (hex to clipboard)
- [ ] Export: whole scene to PNG/SVG, a selected region (BeeRef #143), a single image to file or clipboard (BeeRef #132), all images to a folder
- [ ] Hotkey to jump between images (BeeRef #116); configurable keyboard and mouse bindings
- [ ] Portability: responsive UI layout — no fixed desktop sizes, panels adapt to small screens
- [ ] `.bee` import
- [ ] `.pur` import for all PureRef versions via `purcore` from [LazyRef](https://github.com/achrllrogia45/LazyRef) (pinned revision), plus our own test corpus
- [ ] All parity- and Milanote-checklist items tagged M2 (duplicate by Alt+drag, nudge, PDF export, dark theme, UI scale, normalize, arrange, align/distribute/stack, resets, pickers, canvas locks, export variants)

**Exit criteria:** every BeeRef board opens losslessly; every feature in BeeRef's README and CHANGELOG is present; all M2 parity items are ticked. → **Release 0.3 (public alpha)**

### M3 — Window, overlay, browser

- [ ] Always-on-top, frameless window, move window by hotkey
- [ ] Transparent overlay with adjustable opacity and click-through (BeeRef #100)
  - [x] Native overlay controls and composited master opacity; X11 qualification (Windows/macOS pending)
- [ ] Board tabs in one window; drag a tab out into its own window and back
  - [x] Four independent journaled tabs, with dirty-close prompts and stable ordering
- [ ] Browser extension "Send to MothRef" + `moth-bridge` native host + "Inbox" board
- [ ] Attach to another application's window: Windows and X11 (macOS and Wayland are P2)
- [x] Platform capability flags surfaced in the UI
- [ ] All parity- and Milanote-checklist items tagged M3 (web clipper: hover Save, text clips, board picker; always on bottom, lock window, overlay selection, master opacity, resize to selection, appearance themes)

**Exit criteria:** the overlay works on top of Krita, Photoshop and Blender on Windows, macOS and X11.

### M4 — PureRef 2 parity

- [ ] Groups and a hierarchy tree view
- [ ] Freehand drawing (3 widths, eraser, color) and shapes (ellipse, rectangle, line)
- [ ] Rich text notes with links and checklists; configurable heading font size (BeeRef #130)
- [ ] Animated GIFs with frame stepping
- [ ] Line/dot grid with snapping; automatic alignment guides and edge/center snapping (BeeRef #129)
- [ ] In-app shortcut cheat sheet: all actions and their current bindings (including custom bindings), grouped by category and searchable; open with `/` or from Help
- [ ] Command Palette (Omnibar / Quick Search): search and run application actions, show their current shortcuts; open with Ctrl/⌘+K or a visible menu button, navigate with arrows, Enter to run, Esc to close
- [ ] Slideshow (incl. random order), embedded ICC profiles, trackpad gestures
- [ ] All parity- and Milanote-checklist items tagged M4 (presentation mode, note formatting and markdown shortcuts, sketch card, drawing shortcuts, GIF frame tools, drawing details, locked groups, hierarchy tools, note parenting and links)

**Exit criteria:** every P0/P1 item sourced from PureRef in the feature matrix is done. → **Release 0.7 (beta)**

### M5 — Library

- [ ] Board index (`redb`) with file watcher (`notify`)
  - [x] Bounded recent/pinned catalog with background previews, search, folder filters and persistent tags/pins (requested reference UI slice)
- [ ] Start screen: card grid with board previews; stacks shown as a fan of previews or as a folder with icon and color; tags; search; pinning
- [ ] Stacks are real folders on disk (Procreate-style: drop a board on a board to create a stack); "smart collections" are saved tag filters
- [ ] File manager thumbnails for `.moth` and `.bee` (BeeRef #152): Windows `IThumbnailProvider`, freedesktop thumbnailer, macOS Quick Look extension
- [ ] All Milanote-checklist items tagged M5 (templates, search inside boards, item trash)

**Exit criteria:** a library of 1,000 boards is on screen < 200 ms after launch.

### M6 — Milanote-style elements

- [ ] Arrow connectors that stick to item edges
- [ ] Columns (auto-stacking containers), nested boards with breadcrumbs
- [ ] Color swatch cards and palette extraction from an image; link cards; comment pins
- [ ] Collapsible left toolbar (hidden by default)
- [ ] All Milanote-checklist items tagged M6 (unsorted tray, item labels, synced notes, card colors, captions, to-do cards, board shortcuts, back/forward navigation, Markdown export)

### M7 — PureRef export and full compatibility

- [ ] `.pur` export through `purcore`; verify files open in PureRef
- [ ] Corpus covers every released PureRef 1.x and 2.x version; round-trip tests in CI
- [ ] Format discrepancies are fixed upstream in LazyRef, not in a private fork

**Exit criteria:** all P0 and P1 features done, all performance targets met. → **Release 1.0**

### M8 — Web build (after 1.0)

- [ ] `wasm32` build; WebGPU renderer with WebGL2 fallback
- [ ] Web backend for `moth-io` using `createImageBitmap` and Web Workers
- [ ] Storage in OPFS; open/save via File System Access API (download/upload fallback); installable offline PWA
- [ ] Receive images from the browser extension directly into the tab

**Exit criteria:** a 1,000-image board opens in the browser and saves to a `.moth` file that the desktop app opens.

### M9 — Collaboration (after 1.0, no fixed timeline)

- [ ] Prototype on two CRDT libraries (candidates: Loro, Yrs, Automerge); pick by speed and history size
- [ ] `moth-sync`: commands → CRDT operations; blob sync by `blake3` hash
- [ ] `moth-server`: WebSocket relay + blob store, Docker image (license to be decided when we get there)
- [ ] Participant cursors and selections; share links with view/edit rights

**Exit criteria:** three people edit one board from different OSes and the browser; after a network drop everything merges without loss. → **Release 2.0**

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
| Hotkey to jump between images (#116) | MothRef | P1 | M2 |
| Configurable hotkeys and mouse | BeeRef, PureRef | P1 | M2 |
| `.bee` import | MothRef | P0 | M2 |
| `.pur` import, all 1.x and 2.x versions | MothRef | P1 | M2 |
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
| Searchable in-app cheat sheet of all current hotkeys | MothRef, Milanote | P1 | M4 |
| Command Palette / Omnibar: search and run actions, show shortcuts | MothRef, PureRef 2.0 | P1 | M4 |
| Slideshow incl. random order | PureRef 2.0 | P2 | M4 |
| ICC color profiles | PureRef 2.0 | P1 | M4 |
| Trackpad gestures | PureRef 2.0 | P1 | M4 |
| Start screen with board previews | MothRef, BeeRef | P0 | M5 |
| Stacks (folders) and tags, smart collections | MothRef | P1 | M5 |
| File manager thumbnails (#152) | MothRef | P1 | M5 |
| Milanote-style elements (see the Milanote checklist) | Milanote | P1 | M4–M6 |
| `.pur` export | MothRef | P2 | M7 |
| Web build | MothRef | P2 | M8 |
| Real-time collaboration | MothRef | P2 | M9 |
| SpaceMouse | PureRef 2.0 | P2 | after 1.0 |
| Video on the canvas | FrameRef | P2 | after 1.0 |

`#NNN` refers to issues in the [BeeRef repository](https://github.com/rbreu/beeref/issues).

### Parity checklist (PureRef 2.1 + BeeRef 0.3.x)

The matrix above lists features at headline level. This checklist is the full command-level inventory from the [PureRef handbook](https://www.pureref.com/handbook/) and BeeRef's [actions](https://github.com/rbreu/beeref/blob/main/beeref/actions/actions.py) and [CHANGELOG](https://github.com/rbreu/beeref/blob/main/CHANGELOG.rst). A milestone is not done until every item tagged with it is ticked. Source: **PR** = PureRef, **BR** = BeeRef.

As in PureRef: when nothing is selected, arrange/align/normalize actions apply to all items; otherwise only to the selection. Most of them also work on notes.

#### Normalize (M2)
- [ ] Normalize height (PR, BR)
- [ ] Normalize width (PR, BR)
- [ ] Normalize size (PR, BR)
- [ ] Normalize area (PR)
- [ ] Normalize scale (PR)

#### Arrange (M2 unless noted)
- [ ] Optimal — pack to fit the current window (PR, BR)
- [ ] By name / by filename (PR, BR)
- [ ] By time added (PR)
- [ ] By file path (PR)
- [ ] Randomly (PR 2.1)
- [ ] By hierarchy order (PR) — M4, needs the hierarchy
- [ ] Horizontal, vertical and square layouts by filename (BR)
- [ ] Running the same arrange command again reverses the order (PR)
- [ ] Configurable gap between items (PR, BR)
- [ ] Auto-arrange newly added images, with a chosen default method (PR "select and pack new images", BR "default arrange method")
- [ ] Pack all + optimize canvas in one command (PR)

#### Align and distribute (M2)
- [ ] Align left / right / top / bottom (PR)
- [ ] Distribute horizontally / vertically (PR)
- [ ] Stack — pile selected images on top of each other in layer order (PR)

#### Transform (M2 unless noted)
- [ ] Flip horizontally / vertically (PR, BR)
- [ ] Reset scale, rotation, flip, crop, or all transforms (PR, BR)
- [ ] Raise to top / lower to bottom (BR); setting "bring selected images to front" (PR)
- [ ] Per-image sampling: bilinear vs nearest neighbour, plus a default for new images (PR); automatic nearest-neighbour at large zoom for pixel art (BR)
- [ ] Lock item so it cannot be selected on the canvas, e.g. as a backdrop (PR) — M4

#### Canvas and view (M0–M2)
- [ ] Fit scene / focus canvas (PR, BR) — M0
- [ ] Fit selection / focus image; double-click an image to zoom in, again to go back (PR, BR) — M0
- [ ] Reset camera; reset zoom to 100% (PR) — M0
- [ ] Focus next / previous image (PR; BeeRef #116)
- [ ] Optimize canvas — fit the canvas snugly around all items (PR)
- [ ] Lock canvas — nothing on it can be changed (PR)
- [ ] Grayscale for the whole canvas (PR) and per image (PR, BR)
- [ ] Configurable scroll wheel: pan, zoom or nothing; Shift / Shift+Ctrl + wheel to pan (PR, BR)
- [ ] Optional scrollbars and menu bar (BR)
- [ ] Scale the scene with the window on resize, as a setting (PR)

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
- [ ] Export selected images, all images, scene (PR, BR) — M2
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
- [ ] Every action rebindable; system-global shortcuts for chosen actions (PR, BR) — M2 (global: M3)
- [ ] Settings show which values differ from defaults (BR) — M2
- [ ] Touch input and SpaceMouse (PR) — touch via the input abstraction (M0), SpaceMouse after 1.0
- [ ] Interface translations; at least English and Russian at 0.3, more later (PR 2.1 ships 7 languages) — M2
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
- [ ] File cards (PDF and other uploads) and audio cards (after 1.0)
- [ ] Video card and video link embeds (YouTube, Vimeo) (after 1.0, together with video on the canvas)
- Map card — **skip** (needs online map tiles)
- Built-in stock photo search (Pexels) — **skip** (network service; the browser extension covers collecting)

#### Creating and editing
- [ ] Double-click empty canvas creates a note (setting, off by default to keep PureRef behaviour) (M4)
- [ ] Ctrl/⌘+Enter creates the next note/board/column below the selected one, matching its width and pushing items below down (M6)
- [ ] Duplicate with Alt/⌥+drag (M2)
- [ ] Nudge with arrow keys, farther with Shift+arrows (M2)
- [ ] Send backward / bring forward one step, besides to-top / to-bottom (M2)
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
- [ ] Dark / light theme toggle (M2)
- [ ] Interface scale smaller / larger (M2)
- [ ] Searchable in-app shortcut cheat sheet on `/` and in Help: all actions grouped by category, showing current custom bindings (M4, next to the command palette)

#### Export
- [ ] PDF export of a board or a selection (M2)
- [ ] Markdown and plain-text export of notes, headings, to-dos and link cards (M6)
- [ ] ZIP of all original images and files (M2, with "export all images")
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

| 2026-10-17 | [Reference workspace](docs/decisions/0004-reference-workspace.md): requested early UI slice uses four isolated sessions, a bounded catalog and a separate overlay controller; full M3/M5 remain open. |
