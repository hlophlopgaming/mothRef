# Reference workspace validation — 2026-10-17

The five supplied UI states are wired to native application actions: library,
board, empty board, overlay and dirty-close confirmation. This is the initial
image-board implementation, not completion of every feature pictured in the
reference or the M2–M5 roadmap.

## Checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including deferred fitting, isolated tab
  journals, destination leases, relative file replacement and card label layout.
- `cargo deny check`: passed (advisories, bans, licenses, sources).
- `cargo xtask bench --quick`: passed; 10,000 distinct images, 100 frames,
  p99 3.195 ms, 13.32 MiB textures, AMD RX 5700 XT / RADV.
- `cargo xtask corpus`: passed, 21 image and board fixtures.
- `cargo xtask killtest`: passed, 1,000 process kills with no acknowledged
  commands lost.
- Headless golden checks cover software rendering, dot-grid layering and master
  opacity over overlapping images; registry clear releases tracked resources.
- A failing UI regression reproduced the card-label layout bug before the fix.

**At the maintainer's explicit request, the memory soak and both fuzz processes
were stopped. They are incomplete and are not qualification results.** Do not
restart them as part of this task. The importer-specific `bee_reader` and
`pur_reader` fuzz targets do not exist yet.

## Native inspection

Linux X11, RX 5700 XT, 1440×900: launched the release application with a separate
`XDG_DATA_HOME`, generated synthetic gradient fixtures and opened an actual
`.moth` board. Inspected library previews, the empty import prompt, image board,
fit, tool rail and minimap. Screenshots and command logs are in ignored
`target/ui-validation/`; no repository artwork was redistributed. The library
inspection found sideways/clipped card labels, now fixed with a regression test.

Earlier native overlay inspection covered the separate interactive controller.
Windows/macOS, actual touch devices and full cross-platform overlay behavior
remain unqualified. Wayland on-top is disabled through capability flags.

## Scope and limits

Four sessions; twenty recent plus twenty pinned catalog entries; four 128-pixel
previews per entry. The minimap samples at most 128 items. Arrangement commands
cover the whole selection and group into a single undo step. Folder
filters reflect parent folder names. Full folder management, tab tear-off,
index watchers, notes, hierarchy, crop and other later-milestone tools remain
roadmap work. The UI does not expose decorative controls for those features.

## Motion

Manual checks pending on Linux, Windows and macOS (mouse, pen and touch):

- [ ] Drag a rotated image and multiple selected images; verify 1:1 translation and unchanged saved rotations.
- [ ] Alt-drag leaves originals visible; release has no position jump.
- [ ] Drag quickly then stop: sway settles; grabbing a corner uses that corner as pivot.
- [ ] Cancel with Esc, focus loss and pointer cancellation: return without commands.
- [ ] Enable Reduce motion, restart, and verify the preference persists and motion is instant.
- [ ] After zoom, fit, drag and import, wait one second: frame tracing stops and CPU returns to idle.
- [ ] Repeat using --software and a small window.

- [ ] Wheel zoom keeps a marked pixel under the pointer, including repeated ticks during animation.
- [ ] Pinch/trackpad zoom and mouse/pen/touch pans follow input directly.
- [ ] Touch/trackpad pan glides after release, but holding still before release and mouse-middle pan do not glide.
- [ ] Fit, fit selection and focus return land at the same framing as before.
- [ ] Paste/import/undo/redo/arrange animate; more than 300 changes skip transitions.
- [ ] Mouse/pen hover fades; touch does not introduce hover actions. Toolbar, tabs, Tools and library feedback settle.

No interactive frame resize handles exist in this branch; resize cursor behavior requires the future resize gesture.

Motion automation uses the existing opacity/rotation GPU golden on a forced
software adapter. This host initially had no fallback ICD. Mesa vulkan-swrast
26.2.2 was extracted under ignored `target/software-driver/` (no system or Cargo
dependency changes). Test invocations use
`VK_DRIVER_FILES="$PWD/target/software-driver/lvp.json:/usr/share/vulkan/icd.d/radeon_icd.json"`
to expose both llvmpipe and the normal RADV adapter.

Automated Motion results (2026-10-19):

- Formatting and changed-crate clippy/tests passed after each step.
- Workspace clippy and tests passed, including the existing renderer goldens.
- Final focused tests additionally cover a single frame transform for the whole
  selection, cancellation ending repaint, and release commands preserving rotation.
- Forced-software opacity/rotation golden passed with the local llvmpipe ICD.
- `cargo xtask bench --quick`: 10,000 images, 100 frames, p99 5.813 ms,
  textures 13.32 MiB, AMD Radeon RX 5700 XT / RADV. No comparable pre-change
  measurement was captured; a percentage regression claim is not established.
- `cargo xtask soak 300 5`: passed, 2,813 frames; RSS 159,540 KiB initially,
  plateau 160,024 KiB through the end. It finished before the maintainer asked
  not to wait five minutes; do not repeat a long soak for this task.
- Native device behavior and actual idle CPU remain unverified; use the checklist above.

### Drag release handoff fix

- Regression test delays acknowledgement beyond the 400 ms decorative tail and
  checks image position, logical selection rect, no waiting repaint, and no second
  move tween on acknowledgement, with reduced motion both enabled and disabled.
- Failed storage completion drops the pending preview and displays the actual scene.
- [ ] Release an image repeatedly: neither image nor outline flashes at the old position.
- No long soak repeated, per maintainer instruction.
- Validation: `cargo fmt --all`, clippy for mothref with `-D warnings`, and all
  30 mothref tests passed, including the forced-software golden. The delayed
  acknowledgement test first failed with x=10 instead of x=110, then passed.

### Animated outlines

- Selection and hover outlines use the displayed Item (position, scale, rotation
  and opacity); both UI and GPU sample the same frame timestamp.
- Automated polygon checks compare the painted outline to animated image corners.
- [ ] Drag, release, cancel and arrange rotated images: outlines stay attached.
- Appearance preferences remain deferred; no long stress run repeated.
- Validation: formatting, mothref clippy (`-D warnings`) and all 31 mothref tests
  passed, including animated outline polygons and the software opacity/rotation golden.

### Drag feedback preferences

- [ ] In Tools → Settings, disable Animate dragging: preview follows input 1:1,
  without sway/lift; opacity slider controls only the live preview (default 65%).
- [ ] Release/cancel restores the original opacity; saved item opacity is unchanged.
- [ ] Reduced motion also uses translucent dragging, without animation frames.
- [ ] Restart: drag preferences persist. Re-enable animation: sway/lift return.
- Validation: formatting, clippy for mothref/moth-ui (`-D warnings`) and all 45
  tests passed, including preference round-trip/invalid values, transient opacity
  restoration, scene immutability and the forced-software golden.

### Outline visibility preference

- [ ] Tools → Settings → Show item outlines: disable to hide selection handles
  and hover outlines, including during dragging and its release animation.
- [ ] Selection, dragging and marquee selection still work; restart preserves the setting.
- Automated tests cover preference persistence/defaults and no painted outlines/handles when disabled.
- Validation: formatting, clippy for mothref/moth-ui (`-D warnings`) and all 46 tests passed.

### Stale drag-tail cleanup

- Regression reproduces a confirmed move followed by another geometry change
  while its release tail is alive: the old preview must disappear immediately.
- [ ] Drag, then quickly undo/arrange/edit/delete: no non-interactive old image remains.
- [ ] A normal release still finishes its sway and goes idle; ordinary drag does not duplicate images.
- The regression first failed with a surviving, non-interactive release copy;
  after cleanup, formatting, mothref clippy (`-D warnings`) and all 34 tests passed.
- This covers stale tails after later commands; the maintainer's report also
  mentions single drags, so native confirmation of that exact interaction remains pending.

### Rapid zoom residency fix

- Software golden initially reproduced a blank image while upgrading the fifth
  2048-pixel preview under the 96 MiB texture budget. It compares all eight image
  centers after every upgrade, not only the upgraded image.
- Retain a 32-pixel mip from each existing pyramid inside the same GPU budget;
  evict detailed pages first and promote fallback residency instead of skipping images.
- Ignore stale lower-resolution decode results after a larger preview arrives.
- dhat checks that upgrading an already-decoded 2048-pixel image does not clone
  its full pixel pyramid; dhat is a test-only dependency already used in moth-io.
- [ ] Rapidly zoom in/out over a large board: temporary lower detail is allowed,
  blank images are not. Verify with hardware and --software.
- No long soak repeated, per maintainer instruction.
- Validation: workspace clippy (`-D warnings`), workspace tests, software golden,
  renderer dhat allocation check and `cargo deny check` passed (existing warnings).
- Quick benchmark, RX 5700 XT/RADV, 10,000 images/100 frames: pre-change executable
  p99 7.266 ms; rebuilt executable 9.978 ms, repeat without builds 9.046 ms;
  texture allocation remained 13.32 MiB. Active Resolve, MothRef and browser
  processes were observed, so these timings do not qualify an idle-machine
  before/after comparison or establish the 10% regression guard. Repeat performance
  qualification on an idle machine; no app processes were stopped for this task.

### Adaptive GPU budget

- Limit: 1 MiB per image item, minimum 32 MiB for nonempty boards; maximum
  512 MiB on discrete adapters, 96 MiB on shared/software/unknown adapters.
  No images means a zero-byte cache. Growth does not preallocate textures.
- Image count is maintained with acknowledged commands and recounted once on open.
- Software tests load beyond the former 96 MiB limit, then shrink and clear;
  dhat checks adaptation/upgrade without copying the decoded pixel pyramid.
- [ ] Add/delete/undo images and switch tabs: status shows allocated/limit MiB;
  growth is bounded, shrink releases pages, closing the last board releases all.
- [ ] Import into an empty board: a pending import has room for its preview before acknowledgement.
- No long soak repeated, per maintainer instruction.
- Validation: workspace clippy (`-D warnings`), workspace tests, software golden
  and renderer dhat allocation check passed. Quick benchmark: RX 5700 XT/RADV,
  10,000 images/100 frames, p99 5.931 ms, textures 13.32 MiB. This run does not
  replace the pending idle-machine before/after performance qualification.
