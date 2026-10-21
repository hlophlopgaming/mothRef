# AGENTS.md

Instructions for AI coding agents (and humans) working on MothRef. Read this file fully before making changes. When it conflicts with a habit or a default, this file wins. When it conflicts with an explicit instruction from the maintainer, the maintainer wins.

See `ROADMAP.md` for what to build and in which order. This file is about **how** to build it.

---

## 1. What MothRef is

A native reference board for artists: drop images onto an infinite canvas, arrange them, keep them visible while you work. Successor in spirit to BeeRef; feature target is BeeRef + PureRef 2 + a board library + Milanote-style elements.

Two properties matter more than any feature:

1. **The user never loses work.** Users spend hours collecting references. A crash or a bad save that destroys a board is the worst bug this project can have.
2. **It stays fast and lean on huge boards.** Memory is bounded by budgets, not by the number of images. The one deliberate exception is the default **full-resolution mode** (decision 0017): the user chose to trade video memory for zero loading, so every image of the board is resident at full resolution, bounded by the video-memory limit detected from the graphics card instead of by the viewport. When a board does not fit, the app says so and the rest loads on demand.

If a change trades either of these for a feature, stop and ask.

---

## 2. Repository layout

```
crates/
  mothref/          binary: event loop, windows and tabs, wires everything together
  moth-scene/       scene graph, items, Command + undo/redo, R-tree        (no GPU, no I/O)
  moth-format/      .moth reader/writer, journal, recovery, migrations      (no GPU)
  moth-io/          image decoding, LOD pyramid, tiles, BC7, caches
  moth-render/      wgpu renderer: instancing, texture residency, strokes, text
  moth-render-cpu/  software renderer (P2, only if benchmarks require it)
  moth-ui/          egui-based UI: theme, widgets, animations, command palette, hotkeys
  moth-platform/    OS-specific code: overlay, click-through, window attach, clipboard, DnD
  moth-library/     board index: stacks (folders), tags, smart collections, previews
  moth-import/      .bee and .pur → scene (uses purcore from LazyRef)
  moth-bridge/      native messaging host for the browser extension
  moth-sync/        CRDT sync (M9, do not start before M8 is done)
  moth-web/         wasm32 entry point (M8)
  moth-thumb/       binary: thumbnails for file managers
third_party/purcore/ local, attributed purcore safety fork (maintainer-authorized; decision 0007)
extension/          browser extension (TypeScript, Manifest V3, Firefox + Chromium)
xtask/              build helpers: bench, corpus, packaging
corpus/             test files: .moth, .bee, .pur (available verified samples), images
docs/               FORMAT.md (the .moth spec), ARCHITECTURE.md, decisions
```

Dependency direction is strict: `moth-scene` and `moth-format` depend on nothing GPU-, window- or OS-specific. Never add `wgpu`, `winit`, `egui` or platform crates to them. OS-specific code lives only in `moth-platform` behind a capability trait.

---

## 3. Commands

Checks come in three tiers. Agents run tiers 1 and 2; tier 3 is the release gate and runs in CI or by the maintainer, never inside an agent session. A tier-3 run takes 10 minutes to hours, outlives tool timeouts and competes with builds for CPU.

### Tier 1: fast loop (every change, must finish in a few minutes)

```sh
cargo fmt --all --check
cargo clippy -p <changed crates> --all-targets -- -D warnings
cargo test -p <changed crates>
cargo xtask corpus                    # when touching format or import: round-trip every corpus file
cargo deny check                      # when adding or bumping a dependency
```

Before a PR is final, run the workspace-wide versions once: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.

### Tier 2: quick stress (only for the area you touched)

Run these **one at a time, in the foreground, with no build, fuzzer or other benchmark running**. Timing results under load are meaningless: a failed `bench` or `paste` guard while anything else runs is not a regression, rerun it on an idle machine before acting on it.

```sh
cargo xtask killtest 50               # journal / autosave
cargo xtask soak 300 5                # long-lived data: 5-minute smoke, RSS sampled every 5 s
cargo +nightly fuzz run moth_reader  -- -max_total_time=90   # .moth reader changes
cargo +nightly fuzz run image_reader -- -max_total_time=90   # decoder changes
cargo +nightly fuzz run bee_reader   -- -max_total_time=90   # BeeRef import changes
cargo +nightly fuzz run pur_reader   -- -max_total_time=90   # PureRef import changes
cargo xtask bench --quick             # render, io, scene
```

For agent sessions, one 90-second fuzz run per affected target is sufficient; longer runs belong to the tier-3 release gate.

`bee_reader` and `pur_reader` cover the M2 importers; seed them from the corresponding corpus directories.

### Tier 3: release gate (CI / maintainer only)

```sh
cargo xtask killtest 1000
cargo xtask soak 1800                 # and the 8-hour soak before a release (Reliability workflow)
cargo +nightly fuzz run <target> -- -max_total_time=300      # every target
cargo xtask bench --quick --enforce
cargo xtask paste --enforce
```

If a change needs tier-3 evidence (see section 7), say so in the PR description instead of running it in the session.

Run the app: `cargo run -p mothref --release` (debug builds are much slower because decoders rely on SIMD; `[profile.dev.package."*"] opt-level = 3` is set in the workspace for that reason).

---

## 4. Architecture invariants

Breaking any of these needs an explicit decision in `docs/decisions/`, not a code review comment.

### Scene and mutations
- **Every change to a board is a `Command`** with an inverse. No code mutates scene items directly. This single path gives undo/redo, the autosave journal and (later) CRDT sync.
- **Item ids are `UUIDv7`.** `slotmap` keys are an in-process cache only; never write them to disk or send them anywhere.
- **Z-order uses fractional indices**, not integers.
- **The scene holds no pixels.** Items reference image blobs by `blake3` hash.

### Threads
- **The main thread never does I/O or decoding.** No `std::fs`, no network, no decode, no compression on it. Send work to the pools through bounded `crossbeam` channels and receive results as messages.
- Pools: decode (`rayon`, cores − 1), compression (low priority), storage (journal, autosave, file writes), library indexing.
- A frame must not block on a result. If data is not ready, draw the lower mip or a placeholder.

### Rendering
- Images are instanced quads; textures live in size-class texture arrays (or binding arrays). Pick the mip by on-screen size.
- Redraw only on events or running animations. An idle app uses 0% CPU and GPU.
- Handle `wgpu` device loss by recreating the renderer; the scene is untouched.
- The software adapter (`force_fallback_adapter`) must keep working. Do not use features that only exist on discrete GPUs without a fallback path.

### Memory
- **Every cache has an explicit budget and LRU eviction.** A `HashMap` or `Vec` that grows with board size and is never trimmed is a bug.
- **Video memory follows the hardware.** Texture limits come from the detected adapter (`moth-platform::gpu`): most of dedicated VRAM on discrete GPUs, half of RAM on shared-memory GPUs, and they are shown to the user. Do not reintroduce small fixed caps; the user can override the limit in Settings.
- **Full-resolution mode** (default, decision 0017) keeps everything it preloaded resident (`Registry::keep_all`): no tile release on pan, no texture shrinking on zoom-out, no viewport requests while the board loads. It must stop preloading at the limit (`Registry::fits`) and warn instead of evicting what it loaded. RAM caches stay bounded in this mode; only GPU residency follows the board.
- Back-references use `slotmap` keys or `Weak`. `Arc` cycles are not allowed.
- GPU resources are created through the resource registry so counts can be checked. In debug builds, closing a board must bring the counts back to zero.
- Free egui textures explicitly when panels close.
- Check image dimensions from the header before allocating (decompression bombs).

### Portability (Android and web)
Android and the browser are future targets. These rules keep the port cheap; they apply from M0 even though no mobile build exists yet.
- **No filesystem paths in core crates.** `moth-scene`, `moth-format`, `moth-io`, `moth-import` and `moth-library` read and write through the storage trait (byte streams and opaque handles). Paths are resolved only in the desktop backend. Android hands out `content://` URIs and the web uses OPFS; neither has real paths.
- **Input goes through the pointer abstraction.** Every pointer event carries its kind (mouse, touch, pen) and pressure. Gestures (pinch, two-finger pan, long press) live in their own layer. Never read raw `winit` mouse events outside the input layer, and never make a feature reachable only by hover, right-click or a hotkey without a touch path planned.
- **Surface loss is a normal event.** The window surface can disappear at any time (backgrounded app, minimized window). Recreate the renderer and keep the scene, exactly like device loss.
- **Texture format is chosen at runtime.** BC7, ASTC, ETC2 or uncompressed RGBA8, depending on what the adapter supports. Never hard-code BC7, and never make a `.moth` file depend on one GPU format: LOD caches must stay rebuildable.
- **UI layout is responsive.** No fixed desktop pixel sizes; panels must adapt to small screens and touch-sized targets.
- CI builds the core crates for `aarch64-linux-android` (and `wasm32-unknown-unknown` from M8). Do not add a dependency that breaks those builds.

---

## 5. Data safety rules

These rules protect user boards. Treat them as non-negotiable.

- **Never overwrite a board file in place.** Checkpoints append chunks and a new footer. Compaction writes a temp file in the same directory, `fsync`s it, then atomically renames it (`ReplaceFileW` on Windows).
- **Journal first.** A command is appended to the journal before the UI treats it as done. New image bytes go into the journal at paste time.
- **Every chunk carries a checksum.** Readers skip a torn tail and fall back to the last valid footer. A reader must never panic on a corrupt file; it returns an error or a partial board.
- **Decoders run inside `catch_unwind`.** A broken image becomes a placeholder with an error, never a crash.
- **Format changes are versioned.** Bump the schema version, write a migration `vN → vN+1`, add old-version files to `corpus/`, and update `docs/FORMAT.md` in the same PR. Never remove the ability to read an older version.
- **Never delete user files** outside the app's own cache and recovery folders. Library deletes go to the OS trash (`trash` crate).
- On OS shutdown signals (`WM_QUERYENDSESSION`, `applicationShouldTerminate`, SIGTERM) flush the journal without prompting.

If you are unsure whether a change could lose data, it needs a test that kills the process mid-operation.

---

## 6. Code style

- Rust stable, edition 2024. `#![forbid(unsafe_code)]` in every crate except `moth-render`, `moth-platform`, `moth-bridge` and `moth-thumb`. In those, every `unsafe` block has a `// SAFETY:` comment.
- Errors: `thiserror` in library crates, `anyhow` only in binaries and `xtask`. No `unwrap()`/`expect()` on anything that comes from files, the network, the clipboard or the OS. `expect()` on true invariants must say why it cannot fail.
- No panics across FFI or thread boundaries.
- Logging with `tracing`. No `println!` outside `xtask`. Hot paths get `tracing` spans so Tracy can profile them.
- Public items in library crates have doc comments. Explain *why*, not *what*.
- Keep functions small and crates focused. Do not add a new crate without updating section 2.
- UI text goes through the localization layer, never hard-coded in widgets.
- Platform code: one trait in `moth-platform`, one implementation per OS, capability flags for what an OS cannot do (for example, Wayland cannot keep a window always on top). Never `#[cfg(target_os)]` scattered through other crates.

---

## 7. Testing requirements by change type

Agents may use images from the repository's `images/` directory for tests and manual validation.
Per the maintainer, these images are licensed under Creative Commons; artwork in `images/artBydavidrevoy/` is under Creative Commons 4.0. Preserve attribution and check the specific license variant and its terms before redistributing images.

| You changed | You must add or run |
| --- | --- |
| A `Command` or scene logic | `proptest`: apply + undo = identity; save + load = same scene |
| `.moth` reader/writer | Corpus round-trip, truncation test (cut at every byte of the last checkpoint), tier-2 fuzz run |
| Journal / autosave | Kill-process test: `cargo xtask killtest 50` (the 1,000-kill run is tier 3) |
| `.bee` / `.pur` import or `.pur` export | New corpus files for the case, round-trip test, fuzz run |
| Decoding / LOD / caches | Tier-2 benchmark run; `dhat` allocation check on the hot path |
| Rendering | Headless golden-image test (with tolerance); tier-2 benchmark run |
| Anything holding long-lived data | `cargo xtask soak 300 5` smoke run; the 30-minute plateau check is tier 3 and runs in the Reliability workflow |
| Platform code | Manual test notes in the PR for each OS you could not run in CI |

Bug fixes start with a failing test that reproduces the bug.

---

## 8. Performance budgets

Targets are in `ROADMAP.md`. Rules for agents:

- Do not regress the benchmark suite by more than 10%. CI blocks it.
- Never decode an image at full resolution unless it is shown at ≥ 1:1 zoom, and then only the visible tiles. Exception: the full-resolution mode decodes each original once when the board opens (decision 0017), streams it into tiles without holding the full frame, and keeps the GPU-ready result on disk so later openings do not decode at all.
- Never hold a lock across I/O, decoding or a frame.
- Prefer zero-copy (`rkyv`, `mmap`, `bytes::Bytes`) over cloning buffers.
- If you add a dependency to a hot path, include benchmark numbers before and after in the PR.

---

## 9. Licensing and third-party code

- MothRef is **GPL-3.0-or-later**. Every new dependency must be GPLv3-compatible (MIT, Apache-2.0, BSD, Zlib, ISC, MPL-2.0, LGPL, GPLv3). `cargo deny check` enforces this. Never add a dependency that is proprietary, GPLv2-only, or has no license.
- New source files start with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- **Porting code** from another project (for example BeeRef, GPLv3; PureRef-format, MIT; LazyRef, MIT): keep the original copyright notice, note the source and commit in the file header, and add the project to `THIRD_PARTY.md`.
- **`purcore` (LazyRef):** the maintainer authorized a local safety fork for PureRef import on 2026-10-18. Keep its source revision and attribution in `third_party/purcore/README.md`; see `docs/decisions/0007-local-purcore-import.md`. Other vendoring/forks still need explicit authorization.
- **Clean-room rule for PureRef:** learn the `.pur` format only from files, `purcore` and PureRef-format. Never disassemble, decompile or inspect PureRef binaries, and never paste code or text from them. Generate test files with PureRef's normal UI only.
- "PureRef", "BeeRef" and "Milanote" are mentioned only to describe compatibility. Do not use their logos, icons or UI assets.

---

## 10. Working conventions

- Work in the order of `ROADMAP.md`. Do not start a later milestone's feature unless asked; M8/M9 work is off-limits before 1.0 except for keeping the invariants in section 4.
- Small PRs, one concern each. Title format: `area: imperative summary` (for example `format: append checkpoints instead of rewriting`).
- Commits follow Conventional Commits (`feat:`, `fix:`, `perf:`, `refactor:`, `test:`, `docs:`, `chore:`).
- Update `CHANGELOG.md` under "Unreleased" for every user-visible change.
- Architectural decisions go into `docs/decisions/NNNN-title.md` (context, decision, consequences) and a line in the decision log of `ROADMAP.md`.
- Tick the matching checkbox in `ROADMAP.md` when a milestone task is done.

---

## 11. Things an agent must not do

- Mutate the scene outside a `Command`.
- Add I/O, decoding or sleeping to the main thread.
- Write a board file in place, or change the `.moth` format without a version bump, migration, corpus files and `docs/FORMAT.md` update.
- Add an unbounded cache, an `Arc` cycle, or a GPU resource outside the registry.
- Silence a test, a clippy lint or a `cargo deny` finding to make CI pass. Fix the cause or ask.
- Add a dependency with an incompatible license, or vendor/fork `purcore` outside the authorized safety fork in section 9.
- Use `std::fs` paths in core crates, read raw mouse events outside the input layer, or hard-code a GPU texture format (see "Portability").
- Touch PureRef binaries in any way (see the clean-room rule).
- Make network requests from the desktop app without an explicit user action (link previews load only on click; the update check can be disabled).
- Claim a task is done without running the tier-1 commands (and the tier-2 commands for the area you touched) in section 3.
- Run tier-3 checks inside a session, or run benchmarks while a build or fuzzer is running.

When something is ambiguous, write down the question and the options in the PR description instead of guessing.
