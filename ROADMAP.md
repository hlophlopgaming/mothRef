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
- [ ] Pan, zoom, rubber-band selection, file drag-and-drop, paste from clipboard
- [x] Benchmark boards and measurement harness
- [x] UI skeleton on egui: custom theme, color tokens, transition animations
- [x] Portability: storage behind a byte-stream trait (no `std::fs::Path` in core crates)
- [ ] Portability: input abstraction — pointer events carry kind (mouse / touch / pen) and pressure; gestures are a separate layer
- [x] Portability: surface loss handled like device loss (recreate the renderer, keep the scene)
- [x] CI: core crates (`moth-scene`, `moth-format`, `moth-io`) build for `aarch64-linux-android`

**Status:** runnable preview; native Wayland drops, pen-device validation and the 50 ms end-to-end paste target remain open. See [validation notes](docs/M0-VALIDATION.md).

**Exit criteria:** a 10,000-image board pans with p99 frame time < 8 ms; a pasted 4K image is visible in < 50 ms.

### M0 review follow-up

- [x] Clipboard staging: application data, lossless compression, per-board ownership
  and cleanup on explicit discard (originals survive a crash; layout recovery is M1)
- [ ] Allow large JPEG sources when DCT-reduced allocation fits the decode budget
- [ ] WebP, static GIF and BMP import; EXIF orientation and CMYK JPEG conversion
- [ ] Area-averaged PNG and clipboard previews, including Adam7 input
- [ ] Avoid linear `Scene::top()` scans on repeated insertion
- [ ] Replace the 10,000-item board/render ceilings with bounded render batches by M1–M2
- [ ] Clipboard backpressure feedback and file/URL/HTML clipboard handling
- [ ] Add storage temporary objects and atomic replacement before M1 compaction

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

**Exit criteria:** every BeeRef board opens losslessly; every feature in BeeRef's README and CHANGELOG is present. → **Release 0.3 (public alpha)**

### M3 — Window, overlay, browser

- [ ] Always-on-top, frameless window, move window by hotkey
- [ ] Transparent overlay with adjustable opacity and click-through (BeeRef #100)
- [ ] Board tabs in one window; drag a tab out into its own window and back
- [ ] Browser extension "Send to MothRef" + `moth-bridge` native host + "Inbox" board
- [ ] Attach to another application's window: Windows and X11 (macOS and Wayland are P2)
- [ ] Platform capability flags surfaced in the UI

**Exit criteria:** the overlay works on top of Krita, Photoshop and Blender on Windows, macOS and X11.

### M4 — PureRef 2 parity

- [ ] Groups and a hierarchy tree view
- [ ] Freehand drawing (3 widths, eraser, color) and shapes (ellipse, rectangle, line)
- [ ] Rich text notes with links and checklists; configurable heading font size (BeeRef #130)
- [ ] Animated GIFs with frame stepping
- [ ] Line/dot grid with snapping; automatic alignment guides and edge/center snapping (BeeRef #129)
- [ ] Command palette, slideshow (incl. random order), embedded ICC profiles, trackpad gestures

**Exit criteria:** every P0/P1 item sourced from PureRef in the feature matrix is done. → **Release 0.7 (beta)**

### M5 — Library

- [ ] Board index (`redb`) with file watcher (`notify`)
- [ ] Start screen: card grid with board previews; stacks shown as a fan of previews or as a folder with icon and color; tags; search; pinning
- [ ] Stacks are real folders on disk (Procreate-style: drop a board on a board to create a stack); "smart collections" are saved tag filters
- [ ] File manager thumbnails for `.moth` and `.bee` (BeeRef #152): Windows `IThumbnailProvider`, freedesktop thumbnailer, macOS Quick Look extension

**Exit criteria:** a library of 1,000 boards is on screen < 200 ms after launch.

### M6 — Milanote-style elements

- [ ] Arrow connectors that stick to item edges
- [ ] Columns (auto-stacking containers), nested boards with breadcrumbs
- [ ] Color swatch cards and palette extraction from an image; link cards; comment pins
- [ ] Collapsible left toolbar (hidden by default)

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
| Command palette | PureRef 2.0 | P1 | M4 |
| Slideshow incl. random order | PureRef 2.0 | P2 | M4 |
| ICC color profiles | PureRef 2.0 | P1 | M4 |
| Trackpad gestures | PureRef 2.0 | P1 | M4 |
| Start screen with board previews | MothRef, BeeRef | P0 | M5 |
| Stacks (folders) and tags, smart collections | MothRef | P1 | M5 |
| File manager thumbnails (#152) | MothRef | P1 | M5 |
| Milanote-style elements | MothRef | P1 | M6 |
| `.pur` export | MothRef | P2 | M7 |
| Web build | MothRef | P2 | M8 |
| Real-time collaboration | MothRef | P2 | M9 |
| SpaceMouse | PureRef 2.0 | P2 | after 1.0 |
| Video on the canvas | FrameRef | P2 | after 1.0 |

`#NNN` refers to issues in the [BeeRef repository](https://github.com/rbreu/beeref/issues).

### Milanote-style elements (M6 unless noted)

| Element | In MothRef |
| --- | --- |
| Note, Heading, To-do | Rich text note card, large heading, checklist (M4) |
| Draw / Sketch | Drawing on the board and a white sketch card (M4) |
| Line | Arrow connectors attached to items |
| Column | Vertical auto-stacking container |
| Board | Nested board with breadcrumbs |
| Color | Swatch card; palette extraction (k-means, 5–8 colors) |
| Link | URL card with Open Graph preview; network only on click |
| Comment | Pin with text, shown on hover |
| Table, Document, Audio, Video | After 1.0 |
| Map | Not planned (needs online map tiles) |

---

## Performance targets

Reference machine: 8 cores, SSD, GTX 1660 / Apple M1 class GPU. Calibrate after the first measurements of PureRef 2 and BeeRef on the same boards.

| Metric | Target |
| --- | --- |
| Cold start to library on screen | < 200 ms |
| Open a 1,000-image board: first frame | < 300 ms |
| Same board: everything visible at full detail | < 2 s |
| Pan/zoom a 10,000-image board, p99 frame | < 8 ms (discrete GPU), < 16 ms (integrated) |
| Paste a 4K image until visible | < 50 ms |
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
| 2026-10-16 | Android is a future target (no milestone yet). Portability rules are built in from M0: storage trait, input abstraction, surface-loss handling (M0); runtime texture format (M1); responsive layout (M2) |
| 2026-10-16 | [M0 preview budgets and durable clipboard staging](docs/decisions/0001-m0-canvas-budgets.md): 96 MiB GPU cache, bounded decode work, checksummed RGBA scratch records; keep the 50 ms paste criterion open rather than bypass durability |
| 2026-10-16 | [M1 version-one records and recovery](docs/decisions/0002-m1-durable-records.md): footer commits, journal-before-acknowledgment, atomic snapshots and compaction, bounded session history |
