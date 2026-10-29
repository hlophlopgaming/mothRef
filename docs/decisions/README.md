# Decision log

Every architectural or product decision gets a line here, newest first. Decisions with a full record link to it; the record (context, decision, consequences) lives next to this file as `NNNN-title.md`.

Moved from `ROADMAP.md` on 2026-10-26. Add new lines at the top of the table.

| Date | Decision |
| --- | --- |
| 2026-10-29 | Over grab handle dropped: the Over controller stays the way out of click-through |
| 2026-10-29 | Watch folder import dropped. The browser extension and web clipper leave M3 for M14 as a separate project after 1.0, built on the `moth-bridge` local drop API |
| 2026-10-28 | [Window stacking on Wayland](0019-wayland-window-stacking.md): KWin scripts and a bundled GNOME Shell extension keep MothRef above other windows on Plasma and GNOME; D-Bus through the `dbus` crate and system libdbus |
| 2026-10-26 | Timed drawing sessions (gesture mode) stay in M4 and ship with 0.7 beta as a key feature, not later in 1.1 |
| 2026-10-26 | Embedding vs linking image files and relinking missing files move from M2 to M5, next to Library health: it is a format change and does not block 0.3 |
| 2026-10-26 | Roadmap restructured: `ROADMAP.md` lists open work by release, finished work collapses per milestone; the feature matrix and parity/Milanote checklists move to [`docs/FEATURES.md`](../FEATURES.md); this decision log moves here. Three M3 follow-ups (editable imports, zoom memory measurements, `.moth` size examples) move to [`docs/DEFERRED.md`](../DEFERRED.md) without a timeline |
| 2026-10-24 | [UI surfaces from the action registry](0018-ui-surfaces-from-action-registry.md): buttons run registered actions, surfaces are action lists, settings are a schema-driven model, visual values come from light/dark theme tokens; Board tools window removed in M2.6 |
| 2026-10-22 | [Full-resolution mode](0017-full-resolution-mode.md): default mode preloads every image at full resolution into video memory behind a progress bar and never loads while zooming; video-memory limit detected from the adapter and shown to the user; AGENTS.md exception |
| 2026-10-21 | [Baked tile packs](0016-baked-tile-packs.md): one background decode per large image writes every detail tile; parent tiles stand in while loading; "Prepare zoom tiles" setting (on screen / whole board / off); RAM cache 12.5% ≤ 1 GiB written through to an 8 GiB disk cache |
| 2026-10-21 | [Viewport tiles and scheduling](0015-viewport-tiles-and-scheduling.md): bounded replaceable viewport queue; fixed-size visible detail tiles; native JPEG scanlines and Android NDK toolchain |
| 2026-10-21 | Post-1.0 release targets: 1.5 = composition overlays and ruler (M10), Obsidian interop (M12), capture integrations (M14); 2.0 = perspective and value tools (M10), Excalidraw / draw.io / UML-XML interop, Blender plugin, video-frame capture (M16); PDF import with a page picker (M15), release to be confirmed |
| 2026-10-20 | [Performance modes and LOD cache](0014-performance-modes-lod-cache.md): PureRef-style In memory (default) / low zoom quality / Cache on disk / No cache; decode once per zoom range from a budgeted RAM or disk cache; BC7 only where it can be encoded |
| 2026-10-19 | [RAM-aware texture budget](0013-ram-aware-texture-budget.md): replace per-image growth with actual texture bytes and available RAM; configurable RAM allowance/cache limits, adapter defaults and LRU eviction |
| 2026-10-19 | [Progressive durable loading](0012-progressive-board-loading.md): journaled loading batches overlap previews with storage; bounded footer-chain indexing accelerates healthy `.moth` reads while retaining scanner recovery |
| 2026-10-19 | [Journal original ranges](0011-journal-original-ranges.md): read originals through bounded read-only storage handles into the session journal; remove duplicate `.source` files without changing durable completion boundaries |
| 2026-10-19 | [Adaptive GPU cache budget](0009-adaptive-gpu-budget.md): maintainer requests count-based growth/shrink; 32–512 MiB discrete, 32–96 MiB shared/software, immediate eviction on shrink and zero allocation for an empty board |
| 2026-10-19 | [Parallel app responsibility split](0010-app-responsibility-split.md): retain upstream history and historical design notes while integrating the later tested intent/Workspace/Shell refactor and motion implementation |
| 2026-10-18 | [Runtime texture format](0006-runtime-texture-format.md): BC7 on desktop (target-gated `intel_tex_2`), RGBA8 fallback, ASTC deferred to M8; `.moth` stays GPU-format independent |
| 2026-10-18 | [M2 editing and export](0008-m2-editing-export.md): schema 4 appearance/source names, journaled inspector, bounded PNG/SVG export and unchanged original export |
| 2026-10-18 | [Local purcore import fork](0007-local-purcore-import.md): maintainer authorizes vendoring and local fixes; bounded path-free import, 1.10/2.1 baseline corpus; coverage limited to verified files, with additional qualification driven by available samples |
| 2026-10-17 | [Reference workspace](0004-reference-workspace.md): requested early UI slice uses four isolated sessions, a bounded catalog and a separate overlay controller; full M3/M5 remain open. |
| 2026-10-17 | [M2 content transforms and schema 2](0005-m2-content-transform.md): five-field per-item transform (rotation, flip, content offset/scale/rotation), frame-only indexed footprint, schema 2 records that keep reading schema 1 |
| 2026-10-17 | Post-1.0 milestone M13 (CLI and automation): headless subcommands built into the `mothref` binary with `clap`, `--json` output; reuses the single storage-worker write path; images may be fetched from URLs on explicit invocation |
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
| 2026-10-16 | [M0 preview budgets and durable clipboard staging](0001-m0-canvas-budgets.md): 96 MiB GPU cache, bounded decode work, checksummed RGBA scratch records; keep the 50 ms paste criterion open rather than bypass durability |
| 2026-10-16 | [M1 version-one records and recovery](0002-m1-durable-records.md): footer commits, journal-before-acknowledgment, atomic snapshots and compaction, bounded session history |
| 2026-10-16 | Maintainer accepts the measured 11–12 ms frame time (12 ms benchmark ceiling); revise the immediate 4K paste target to 250 ms. Historical measurements retain their original targets. The paste harness excludes OS clipboard transfer; native end-to-end qualification remains separate. |
| 2026-10-16 | Maintainer tightens the immediate 4K paste target to 150 ms; qualification remains open. |
| 2026-10-16 | Maintainer accepts approximately 200 ms durable 4K paste and closes TD-03; the 150 ms goal is withdrawn. Timing outliers and native qualification remain documented. |
| 2026-10-16 | [Bounded WebP previews](0003-bounded-webp-previews.md): maintainer permits full-frame static WebP with 8 MiB encoded/frame admission limits; internal codec allocations are separately qualified. GIF/BMP use row-based previews. |
| 2026-10-16 | Post-1.0 milestones M10 (artist tools), M11 (review and presentation), M12 (note-app interop through Markdown, JSON Canvas and URIs, not a single app's API); independent of M8/M9 |
